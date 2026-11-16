# memory.md 内存子系统规范

## 1 概述

本文档定义自研操作系统内存子系统完整实现规范，涵盖物理内存管理、虚拟地址管理、公平配额调度、内存压力管控、OOM兜底策略。
本系统采用**基准均分‑动态借用‑分级挤压‑缓冲降级**内存调度模型，用于避免单一进程耗尽整机内存资源。系统进程实施分级保护策略，在极端资源枯竭场景提供可预期的降级与兜底行为。

全部内存配额、运行统计、压力通知事件均通过内核对象条目对外暴露，不新增独立系统调用。

强制约束：禁止抢夺进程热内存页；仅允许对长期未访问的冷页执行换出操作；进程保底配额在任何场景下不允许被击穿。

## 2 术语定义

| 术语 | 说明 |
| --- | --- |
| HPA | 宿主机物理内存地址 |
| HVA | 内核虚拟地址，内核空间全局共享 |
| UVA | 用户进程虚拟地址，每个进程具备独立用户地址空间 |
| base_quota | 基准均分配额，由用户可用总内存除以活跃进程数量计算得出 |
| effective_quota | 进程当前生效动态配额，跟随全局内存压力动态伸缩 |
| min_quota | 进程保底配额，维持进程基础运行的最小内存额度，所有运行场景不可低于该阈值 |
| free_shared_pool | 全局富余内存池，收集各进程闲置配额，为内存紧缺进程提供临时配额借用 |
| long_term_peak | 滑动观测窗口内进程PSS内存占用峰值，用于核心进程自适应配额收缩逻辑 |
| safety_margin | 自适应缩配安全余量，防止配额收缩后无法应对进程内存短期上涨 |
| alloc_fail_grace | 内存耗尽缓冲模式，仅拦截用户态新增内存申请，不改动已经分配完成的内存页面 |

## 3 进程资源优先级分类

系统按照进程类别确定内存挤压、页面换出、OOM终止的执行先后顺序：

1. **user_background**：后台用户进程
优先级最低，最先执行配额收缩、页面换出；OOM场景优先终止。
2. **user_foreground**：前台用户进程
面向用户交互，不主动执行配额收缩，仅在压力升高后对冷页执行交换。
3. **aux_system**：附属系统服务
普通用户进程资源压榨完毕之后才执行挤压；极端条件下允许终止，由init负责重启恢复。
4. **core_system**：核心系统进程
保护等级最高，仅当其余全部类别进程资源已压榨至极限，才允许执行有限度的配额调整与冷页换出。

> 
> core_system成员示例：init进程、窗口合成器、窗口管理器。
> aux_system成员示例：网络管理服务、日志服务、通知服务、主题服务、统计采集服务。

## 4 公平配额分配机制

### 4.1 基准配额计算

触发时机：进程创建、进程销毁、内存热插拔事件。

计算公式：

```
base_quota = total_user_ram / active_proc_count
```

新进程初始化：`effective_quota = base_quota`

### 4.2 富余池动态借用

1. 进程实际内存占用低于自身`effective_quota`时，闲置配额回收进入`free_shared_pool`。
2. 内存不足的进程优先从全局富余池申请临时配额扩容。
3. 借用属于临时分配，当进程数量变化或者全局内存压力升高时，系统可以回收借出配额。

## 5 核心进程自适应配额收缩

该逻辑仅针对`core_system`类别进程，目的是回收进程初始化分配、但长期从未使用的配额资源。

### 5.1 统计机制

维护滑动观测窗口，默认窗口时长600 000毫秒（10分钟，可配置），持续统计窗口内进程PSS峰值，保存为`long_term_peak`。

> 
> 禁止使用瞬时内存占用值执行缩配操作。

### 5.2 执行条件（全部条件同时成立）

1. 系统进入重度或临界内存压力状态；
2. 用户进程、附属系统服务已经压榨至资源上限；
3. `long_term_peak`显著小于当前`effective_quota`。

### 5.3 配额计算

```
new_effective = long_term_peak + safety_margin
```

计算结果不得小于该进程`min_quota`。

### 5.4 配额回弹

当核心进程内存占用快速上涨时，允许从`free_shared_pool`申请配额，抬高`effective_quota`。

## 6 内存资源挤压执行顺序

按照固定顺序逐级施加内存压力，前一梯队资源已压榨至极限，才允许进入下一梯队。

1. **第一梯队 user_background**
收缩`effective_quota`，回收闲置配额进入富余池；对冷页执行交换。
2. **第二梯队 user_foreground**
不主动收缩配额，仅执行冷页换出、文件缓存回收。
3. **第三梯队 aux_system**
下发最高等级内存压力事件；在不突破`min_quota`前提下收缩`effective_quota`；冷页执行交换；极端OOM条件允许终止进程。
4. **第四梯队 core_system**
仅在前三梯队全部耗尽且交换分区接近占满时触发：

- 根据长期峰值自适应下调`effective_quota`；
- 对长期未访问冷页执行换出；
- 严格禁止击穿`min_quota`，禁止操作热内存页。

**窗口换出**：后台 / 图层中被遮挡、且 CPU 与内存均未被使用的窗口，其全部状态可整体换出到磁盘（swap），释放内存；用户切换回来时再动态换入加载，从而节省内存占用。

## 7 内存压力状态定义

| 状态 | 说明 |
| --- | --- |
| normal | 内存资源充足，不执行主动挤压动作 |
| mild_pressure | 轻度压力，仅回收文件缓存 |
| heavy_pressure | 重度压力，开始逐级收缩配额、页面交换 |
| critical_pressure | 临界压力，允许对核心系统进程执行自适应缩配 |
| oom_grace | OOM缓冲模式，物理内存与交换分区耗尽，进入缓冲窗口期 |

## 8 OOM处理流程

当物理内存与交换分区全部耗尽，且全部可挤压资源消耗完毕，按下面阶段执行。

### 阶段一：缓冲窗口期 oom_grace

1. 不终止任何进程，不回收已经分配完成的内存页面。
2. 拦截全部用户态新增内存分配请求；内核自身内存分配不受限制。
3. 开启超时计时器，超时时间`grace_timeout_ms`可配置。

> 
> 设计目的：等待业务进程主动释放缓存、或用户手动退出程序，尝试恢复内存水位。

### 阶段二：超时未恢复，执行OOM终止

按照优先级顺序选择进程终止：

1. 后台用户进程；
2. 闲置前台用户进程；
3. 可重启附属系统服务；> 
> 尽量规避终止`core_system`类别进程。

### 阶段三：终极兜底

执行OOM终止后内存压力仍未缓解，触发内核panic，执行安全重启。

## 9 对象条目规范

### 9.1 进程对象 `/object/proc/<pid>`

| 条目 | 访问权限 | 说明 |
| --- | --- | --- |
| mem_quota.base | 只读 | 基准均分配额 |
| mem_quota.effective | 只读 | 当前动态生效配额 |
| mem_quota.min | 系统可写 | 进程保底配额，不可击穿 |
| mem_stats.long_term_peak | 只读 | 滑动窗口内存占用峰值 |
| mem_stats.window_ms | 系统可写 | 观测窗口时长配置 |
| mem_stats.safety_margin | 系统可写 | 自适应缩配安全余量 |
| mem_pressure_event | 等待可读 | 内存压力通知事件 |

### 9.2 全局内存管理对象 `/object/mem/mgr`

| 条目 | 访问权限 | 说明 |
| --- | --- | --- |
| free_pool_size | 只读 | 全局富余内存池当前大小 |
| pressure_level | 只读 | 系统当前内存压力状态 |
| grace_timeout_ms | 系统可写 | OOM缓冲模式超时时间 |
| policy_sequence | 系统可写 | OOM策略执行序列配置 |

## 10 跨模块对接说明

1. 进程创建销毁事件，由进程管理模块触发配额重计算；
2. 开机初始化阶段完成物理内存管理器初始化，设置各进程初始`min_quota`；
3. 图形缓冲、共享内存模块需要遵从本规范的配额与映射约束；
4. 虚拟机模块的客户内存同样纳入本套配额调度体系。
5. 对象、条目内存归属由对象管理器统一承载（`objmgr.md`）。

## 11 内存 `.api` 契约文件（memory.api 全文）

内存对外接口统一由以下契约文件声明，文件存放于 `/system/api/memory.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# memory.api 内存子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = memory
env = python
exec = /system/hwsched/memory/manager
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = kernel
# 全局默认调用限流规则
global_limit = 500次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/mem mem

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
mem: mgr_status { } /internal/mem/status
mem: proc_quota { } /internal/mem/quota
mem: pressure_level { } /internal/mem/pressure

# ---- 可写控制条目：转换代码为Python，实现内存配置 ----
mem: set_grace {
    return self.set_grace(timeout_ms)
} /internal/mem/grace
mem: set_policy {
    return self.set_policy(sequence)
} /internal/mem/policy

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    mem_pressure(level: str) ｜ /internal/event/mem_pressure
    oom_killed(pid: int) ｜ /internal/event/oom_kill

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    set_grace: 10次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    set_grace: kernel
    set_policy: kernel
```

契约要点：

1. 内存配额、压力状态只读透传；OOM / 压力配置修改仅授权 `kernel`。
2. `mem_pressure` / `oom_killed` 事件供系统监控与审计（`log.md`）监听。