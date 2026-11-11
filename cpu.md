# cpu.md CPU 调度与算力分配子系统规范（扩展完整版）

## 1 概述

本文档定义混合内核下 x86‑64 平台 CPU 调度、异构算力分配规范，适配**性能大核 (P‑Core)‑中核 (M‑Core)‑能效小核 (E‑Core)**异构 CPU 架构，同时兼容传统 SMP 同构处理器。
调度器运行于 Ring0 内核基座；调度配置、统计、控制全部挂载在内核对象条目，**不新增独立系统调用**。调度实体包含用户态进程 / 线程、内核线程、虚拟机 vCPU。
本模块与内存子系统复用同一套进程分类标签，实现 CPU 算力、内存挤压、OOM 策略行为协同一致；同时包含**中断亲和性、内核线程调度、功耗协同、节流机制、负载统计**完整逻辑。

### 文档对接

- **objmgr.md**：调度配置、统计、控制挂载的内核对象由对象管理器统一创建、登记、回收。
- **process.md**：进程/线程调度实体对应进程对象。
- **memory.md**：复用同一套进程分类标签，算力与内存策略协同。
- **power.md**：全局 powersave 电源策略联动调度策略。

> 
> 前提约束
> 
> 
> 1. Ring1/Ring2 闲置；仅使用 Ring0、Ring3；
> 2. 支持 SMP 同构、P/M/E 异构多核；
> 3. 用户态只能设置偏好，核心调度参数受 ACL 权限管控；
> 4. 区分概念：调度权重、CPU 时间硬配额、核簇偏好、CPU 亲和掩码、中断亲和掩码。

## 2 术语定义

表格

| 术语 | 说明 |
| --- | --- |
| P‑Core（性能大核） | 高主频、高 IPC，功耗高；优先交付交互、低延迟任务 |
| M‑Core（中核） | 性能功耗折中，作为任务缓冲中转簇；承接溢出前台、轻度后台负载 |
| E‑Core（能效小核） | 低主频、低功耗；适合批处理、索引、解压等非交互后台任务 |
| sched_weight | 调度权重；就绪状态下，决定分得 CPU 时间片占比 |
| cpu_time_quota | 单统计周期 CPU 时间硬配额；超限执行节流降权，不终止进程 |
| core_preference | 核簇偏好标记：`auto / performance / balance / efficiency` |
| sched_class | 调度分类，与`memory.md`的`proc_class`完全同源：`core_system` / `aux_system` / `user_foreground` / `user_background` |
| tick_period | 调度统计周期，单位毫秒，全局可配置 |
| isr_affinity_mask | 中断亲和掩码，指定硬件中断投递至哪些逻辑 CPU |
| kthread_class | 内核线程调度类别；区分内核关键线程、普通内核工作线程 |
| throttle_state | 节流状态；进程 / 线程超出 CPU 时间配额后进入节流状态 |
| vcpu_entity | 虚拟机 vCPU 调度实体，复用本调度器全部权重、偏好、配额逻辑 |

## 3 调度实体分类与默认策略

### 3.1 用户态进程 / 线程

表格

| 调度类别 | 默认 core_preference | 默认 sched_weight | 说明 |
| --- | --- | --- | --- |
| core_system核心系统进程 | performance（优先大核） | 最高权重 | init、窗口合成器、窗口管理器；保障交互响应；进程休眠自动回落中核 |
| aux_system附属系统服务 | balance（折中） | 次高权重 | 网络、通知、日志服务；混合分配大 / 中核，避免抢占前台应用 |
| user_foreground前台用户进程 | auto（自动） | 中等偏高权重 | 用户正在交互应用；窗口激活倾向大核；切后台自动迁移向中 / 小核 |
| user_background后台用户进程 | efficiency（优先小核） | 低权重 | 后台缓存、批处理、索引任务；尽量运行小核，释放性能核给交互任务 |

`auto`自动模式规则：

- 窗口管理器上报进程处于**激活交互状态** → 优先 P‑Core；
- 窗口最小化、失去焦点、长时间无用户输入 → 逐步迁移任务至 M‑Core / E‑Core。

> 
> 单线程可覆写所属进程的调度参数；同一进程内部：交互线程跑大核，工作线程跑小核。

### 3.2 内核线程 kthread_class

内核线程划分为两类，内核内部标记，不暴露用户态：

1. **kthread_critical 内核关键线程**
中断下半部处理、内存回收关键路径、IPC 事件调度线程。

- 策略：优先 P‑Core；不可被用户态任务抢占；拥有最高调度优先级；不受普通用户进程时间配额限制。

2. **kthread_worker 普通内核工作线程**
块设备 IO 工作队列、文件预读、垃圾回收、后台刷新。

- 策略：balance 折中；允许被前台用户交互任务抢占；可以被调度到 M‑Core，负载高时溢流 E‑Core。

> 
> 内核线程不受用户态 ACL 约束；调度参数在内核初始化写死，部分参数可通过全局`/object/cpu/mgr`条目调整。

### 3.3 虚拟机 vCPU 调度实体

vCPU 作为独立调度实体，继承对应虚拟机对象的调度配置：支持`sched_weight`、`core_preference`、`cpu_time_quota`；
虚拟机管理进程（用户态）修改调度配置需要对应虚拟机对象的 ACL 写入权限。

## 4 调度器核心工作机制

### 4.1 权重时间分片

在一个`tick_period`统计周期，就绪队列内任务 CPU 时间按照`sched_weight`权重比例分配时间片。

### 4.2 CPU 硬时间配额与节流机制 `cpu_time_quota`

1. 每个调度实体拥有周期 CPU 时间硬上限`cpu_time_quota`；0 代表不启用硬配额。
2. 周期内实际 CPU 消耗达到配额上限，进入`throttle_state`节流状态。
3. 节流行为：临时下调该实体有效调度权重，拉长调度间隔；**不杀死、不销毁任务**。
4. 新`tick_period`周期到来，自动恢复原始调度权重，解除节流。

> 
> 目的：防止单个后台任务耗尽全部 CPU 算力，阻塞系统交互。

### 4.3 异构核簇任务迁移负载均衡

迁移约束执行顺序（严格）：

1. 前台交互就绪任务优先放置 P‑Core 性能核，保证交互延迟；
2. 后台批处理任务优先调度 E‑Core 能效小核；
3. M‑Core 中核作为缓冲池：承接大核溢出前台任务、小核溢出后台任务；
4. P‑Core 全部满载，前台任务允许溢流到 M‑Core；
5. E‑Core 全部满载，后台任务允许溢流 M‑Core；

> 
> 硬性规则：**只要存在就绪前台交互任务，后台任务禁止抢占空闲 P‑Core 大核**。

### 4.4 休眠‑唤醒迁移策略

1. 前台任务长时间休眠后唤醒：优先投放 P‑Core，降低交互响应延迟。
2. 后台任务休眠唤醒：直接投放 E‑Core。
3. 内核关键线程唤醒：优先 P‑Core。

### 4.5 中断亲和调度（IRQ Affinity）

硬件中断不与普通任务争抢调度队列；通过`isr_affinity_mask`控制中断投递到指定逻辑 CPU。

1. 高实时交互设备（显卡、输入设备键盘鼠标）中断，定向投递 P‑Core，降低输入输出延迟。
2. 存储、网卡大吞吐量中断，可以分散至 M‑Core / E‑Core，释放性能核。
3. 全局 CPU 管理对象提供条目读取 / 修改中断亲和掩码；修改中断掩码需要管理员 ACL 权限。

> 
> 注意：中断亲和只是设置投递目标 CPU，不改变线程调度；中断下半部（软中断）遵循内核线程调度规则。

## 5 对象条目规范

### 5.1 进程对象 `/object/proc/<pid>`

> 
> 线程子条目 `/object/proc/<pid>/thread/<tid>` 可覆写全部调度条目；不覆写则继承进程配置。

表格

| 条目 | 权限 | 说明 |
| --- | --- | --- |
| sched.sched_class | 只读 | 调度分类，与内存子系统 proc_class 同源 |
| sched.sched_weight | 管理员可写 | CPU 调度权重 |
| sched.cpu_time_quota | 管理员可写 | 调度周期 CPU 硬时间配额，0 = 关闭配额限制 |
| sched.core_preference | 用户 / 管理员可写 | `auto / performance / balance / efficiency`核簇偏好 |
| sched.affinity_mask | 管理员可写 | CPU 逻辑核亲和掩码；精细绑定特定逻辑 CPU |
| sched.stat | 只读 | 周期 CPU 消耗、节流状态、任务迁移统计、当前运行簇信息 |

### 5.2 全局 CPU 调度管理对象 `/object/cpu/mgr`

表格

| 条目 | 权限 | 说明 |
| --- | --- | --- |
| global.tick_period | 管理员可写 | 调度统计周期，单位 ms |
| global.irq_rebalance_enable | 管理员可写 | 开启 / 关闭中断自动负载重平衡 |
| [cluster.info](https://cluster.info) | 只读 | 硬件簇信息：P‑Core/M‑Core/E‑Core 数量、主频区间 |
| cluster.load | 只读 | 各个核簇实时负载、运行任务类型统计 |
| irq.affinity_table | 管理员可写 | 各硬件中断号对应的中断亲和掩码配置表 |
| policy | 管理员可写 | 全局策略：`performance / balanced / powersave` |
| kthread.config | 管理员可写 | 内核线程全局调度参数（仅部分参数可调整） |

### 5.3 虚拟机 vCPU 调度（vm.md 对接）

虚拟机对象下 vCPU 子条目拥有一套和用户线程完全一致的`sched.*`调度条目。

## 6 系统压力与降级行为

1. **P‑Core 性能大核满载**
前台任务溢流 M‑Core 中核；后台任务严格禁止抢占空闲 P‑Core。
2. **整机 CPU 全部满载**
严格按照`sched_weight`分配时间片；超出配额任务进入节流降权；UI 交互类进程维持较高权重保障基础体验。
3. **全局 powersave 功耗模式**
调度策略自动调整：任务整体向 M‑Core、E‑Core 收拢；减少 P‑Core 的任务投放；中断尽量投递至能效核，降低整机功耗。
4. **中断过载压力**
开启`irq_rebalance_enable`时，内核自动重平衡中断亲和掩码，把过载硬件中断分散到多个 CPU，避免单核心被中断打满

5. **后台 / 遮挡窗口休眠**：图层中被遮挡或处于后台、且 CPU 与内存均未被使用的窗口，调度器将其标记为可休眠（不参与调度、零 CPU 消耗），状态交内存子系统换出到磁盘；用户切换回来时恢复调度并动态加载，从而节省 CPU 与内存。

## 7 CPU 调度 `.api` 契约文件（cpu.api 全文）

CPU 调度对外接口统一由以下契约文件声明，文件存放于 `/system/api/cpu.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# cpu.api CPU 调度子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = cpu
env = python
exec = /system/hwsched/cpu/scheduler
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 500次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/cpu cpu

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
cpu: cluster_info { } /internal/cpu/cluster
cpu: cluster_load { } /internal/cpu/load
cpu: proc_sched { } /internal/cpu/sched

# ---- 可写控制条目：转换代码为Python，实现调度配置 ----
cpu: set_weight {
    return self.set_weight(pid, weight)
} /internal/cpu/weight
cpu: set_preference {
    return self.set_preference(pid, pref)
} /internal/cpu/pref
cpu: set_affinity {
    return self.set_affinity(pid, mask)
} /internal/cpu/affinity
cpu: set_policy {
    return self.set_policy(policy)
} /internal/cpu/policy
cpu: set_irq_affinity {
    return self.set_irq_affinity(irq, mask)
} /internal/cpu/irq

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    cpu_throttled(pid: int) ｜ /internal/event/cpu_throttle

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    set_weight: 50次/秒
    set_affinity: 50次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    set_weight: kernel
    set_affinity: kernel
    set_policy: kernel
    set_irq_affinity: kernel
```

契约要点：

1. 调度权重 / 亲和掩码 / 全局策略 / 中断亲和修改仅授权 `kernel`（管理员）；核簇偏好 `set_preference` 用户 / 管理员可写。
2. 簇信息、负载、进程调度状态只读透传。
3. 节流事件供监控（`log.md`）审计。