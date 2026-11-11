# process.md 进程管理子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；进程对象为**纯控制对象**，不挂载 VFS，仅原生对象通路访问。
> 2. Ring1/Ring2 闲置；进程对象受 `acl.md` 两级 ACL 管控；调度、内存、日志接口分别对接 `cpu.md`、`memory.md`、`log.md`。
> 3. **条目不在此文档硬编码**：进程对象对外条目的集合、命名、权限由 `process.api` 契约文件声明，经 `.api` 桥接层加载进对象管理器；本文档只定义进程子系统的行为语义，契约全文见第 8 节。
> 4. 进程对象路径约定：父对象 `/object/proc`，进程实例 `/object/proc/<pid>`，每个实例下挂调度、内存、日志、API 等各子系统条目分支。
> 5. 依赖：`os.md`、`objmgr.md`、`acl.md`、`cpu.md`、`memory.md`、`ipc.md`、`fs.md`、`log.md`、`.api.md`、`boot.md`、`hardware_svc.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| PCB | 进程控制块，内核内部数据结构，保存进程全部运行状态，用户态不可见 |
| PID | 进程唯一标识，由内核分配，进程创建时生成 |
| 进程对象 | 每个进程对应一个纯控制对象 `/object/proc/<pid>`，是进程在内核对象模型中的载体 |
| 进程类型 | `PROC_TYPE_NORMAL` / `PROC_TYPE_SERVICE`，PCB 标记，标识进程是否具备服务身份 |
| 进程分类 | 与 `cpu.md`、`memory.md` 完全同源的分级标签：`core_system` / `aux_system` / `user_foreground` / `user_background` |
| 僵尸进程 | 已退出但未被父进程回收的进程，PCB 保留供父进程读取退出码 |
| 孤儿进程 | 父进程已消亡的子进程，由 init 统一收养回收 |

## 1 进程对象存在性

1. `/object/proc` 为全局纯控制父对象，由内核对象管理器在基座初始化阶段创建（`boot.md` 阶段 4）。
2. 每次进程创建，内核在 `/object/proc` 下动态创建一个子对象，路径 `/object/proc/<pid>`，ACL 校验父对象 `CREATE_ENTRY` 权限。
3. 进程销毁时，删除对应子对象，回收全部子条目与引用计数（见第 7 节）。
4. **条目的具体布局不由本文档规定**：进程对象及其子系统的全部对外条目由 `process.api` 契约文件声明（见第 8 节），桥接层加载后挂载至对象，外部程序仅能经由 `/system/api/process` 标准接口访问，禁止直连 `/object/proc/<pid>` 内部条目。

## 2 进程生命周期与状态机

进程状态迁移：

```
created → ready ⇄ running
                 ↓
              blocked ⇄ ready
ready/running → exited → zombie → (父进程 wait) → 回收
```

| 状态 | 触发条件 | 说明 |
| --- | --- | --- |
| created | 进程对象创建完成，资源已分配 | 尚未进入就绪队列 |
| ready | 具备运行条件，进入就绪队列 | 等待调度器分配 CPU |
| running | 调度器选中，占用 CPU | 运行于 Ring3 用户态 |
| blocked | 等待条目读、IPC 事件、锁等阻塞操作 | 被内核对象管理器阻塞休眠，不占用 CPU |
| exited | 主动退出或被终止 | PCB 保留退出码，转为僵尸 |
| zombie | 已退出等待父进程回收 | 仅保留 PCB 与退出码 |
| 回收 | 父进程 wait 读取退出码 | 内核释放 PCB，删除进程对象 |

## 3 PCB 语义

PCB 为内核内部结构，保存进程全部运行状态，包含以下概念字段（**只定义语义，不定义条目名**）：

1. **标识**：PID、父进程 PID、进程名、创建时间。
2. **类型**：`PROC_TYPE_NORMAL` / `PROC_TYPE_SERVICE`（见第 5 节）。
3. **分类**：`core_system` / `aux_system` / `user_foreground` / `user_background`（见第 6 节），与 `cpu.md` 调度分类、`memory.md` 内存分级完全同源。
4. **调度状态**：调度分类、核簇偏好、调度权重、CPU 亲和、节流状态（对接 `cpu.md`）。
5. **内存状态**：基准配额、生效配额、保底配额、长期峰值、压力事件（对接 `memory.md`）。
6. **资源持有**：进程持有内核对象句柄表、文件描述符表、窗口对象、套接字对象、图形缓冲、DMA 缓冲、服务注册条目（对接 `fs.md`、`ui.md`、`net.md`、`hardware_svc.md`）。
7. **日志配置**：独立日志等级、模块标记（对接 `log.md`）。

PCB 由内核对象管理器统一维护，进程退出时按第 7 节流程回收。

## 4 进程类型：NORMAL / SERVICE

1. 普通应用默认 `PROC_TYPE_NORMAL`，无硬件独占能力。
2. `PROC_TYPE_SERVICE` 为 PCB 标记，**不等于自动获得 ACL 权限**；仅用于服务绑定逻辑校验（`acl.md` 第 2 节）。
3. 进程完成服务注册（向 `/object/service_mgr` 创建注册条目）后，内核校验通过自动将 PCB 标记为 `PROC_TYPE_SERVICE`（`hardware_svc.md` 第 2 节）。
4. 未完成注册的进程后续操作硬件对象业务条目会被 ACL 拒绝。

## 5 进程分类与分级保护

进程分类与 `cpu.md` 调度分类、`memory.md` 进程优先级分类完全同源，三方行为协同：

| 分类 | 保护等级 | 说明 |
| --- | --- | --- |
| core_system | 最高 | init、窗口合成器、窗口管理器；极端资源枯竭才允许有限度缩配/换出 |
| aux_system | 高 | 网络、日志、通知、主题、统计服务；普通进程压榨完才挤压，极端可终止并由 init 重启 |
| user_foreground | 中 | 用户交互进程；不主动收缩配额，仅冷页换出 |
| user_background | 低 | 后台缓存、批处理；最先挤压、换出，OOM 优先终止 |

分类一经创建即确定，修改需管理员 ACL 权限；分类决定 CPU 核簇偏好、内存挤压顺序、OOM 终止顺序。

## 6 派生与父子关系

1. **init 进程（PID=1）**：内核对象管理器创建的第一个进程对象（`boot.md` 阶段 6），运行于 Ring3，负责拉起用户空间全部服务与进程。
2. **派生**：用户态进程通过 fork/exec 语义派生子进程；每次派生在 `/object/proc` 下创建新的进程子对象。
3. **孤儿收养**：父进程消亡后，其全部存活子进程由 init 收养；子进程退出后由 init 统一回收，避免僵尸堆积。
4. **退出码**：子进程退出码由父进程通过 wait 读取；未读取前 PCB 以僵尸态保留。

## 7 崩溃自动回收

进程消亡（主动退出 / 崩溃 / 被终止 / OOM）时，内核对象管理器自动执行统一回收，不依赖用户态处理：

1. 关闭该进程持有的全部内核对象句柄，递减引用计数；引用计数归零的对象自动销毁。
2. 关闭全部文件描述符（磁盘 fd 关闭 VFS 上下文，虚拟节点 fd 释放对象句柄）。
3. 销毁该进程全部窗口对象、图形缓冲引用（`ui.md` 第 7 节）。
4. 关闭全部套接字句柄，TCP 执行断开，释放 rx/tx 缓冲区（`net.md` 第 6 节）。
5. 撤销 MMIO 虚拟映射、清除 IOMMU 分组、释放 DMA 缓冲、硬件复位、删除服务注册条目（`hardware_svc.md` 第 5 节）。
6. 卸载该进程对应的 `.api` 契约，销毁 `/system/api/<target>` 虚拟对象（`.api.md` 第 9.6 节）。
7. 释放 PCB 与页表，删除 `/object/proc/<pid>` 进程对象。
8. 触发审计日志，记录 PID、分类、终止原因（`log.md` 第 9.3 节）。

## 8 进程文字输出（控制台）

1. **全屏阶段**：系统启动期间显卡文字模式**全屏**显示开机提示（`boot.md` 第 1 节），此时窗口系统尚未就绪。
2. **图形阶段**：图形环境就绪后，文字模式不再全屏；**每个进程可拥有一个文本输出控制台**。
3. **写入渲染**：进程向控制台写入字符，合成器渲染到**该进程自己的窗口**（`ui.md` 文本输出窗口），而非全屏。
4. **无窗口服务**：后台服务（无窗口）的文本输出走审计日志（`log.md`），不创建控制台窗口。
5. **控制台对象**：进程控制台登记 `/object/process/<名称>/console`，经 `process.api` 访问。
6. **重定向**：进程可将自身输出重定向到其他进程的终端 / 虚拟终端设备（`console_attach`）。
7. **虚拟设备支持**：控制台 / 终端可由服务实现为**虚拟设备**（`hardware_svc.md` 第 7 节）——如虚拟终端、虚拟串口，无真实硬件、纯软件模拟；进程输出可连接虚拟终端设备节点。
8. **回收**：进程消亡时控制台窗口与虚拟终端连接一并清理（第 7 节回收流程）。

## 9 进程子系统 `.api` 契约文件（process.api 全文）

进程管理子系统对外接口统一由以下契约文件声明，文件存放于 `/system/api/process.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# process.api 进程管理子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = process
env = python
exec = /system/kernel/core
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = core_system
# 全局默认调用限流规则
global_limit = 1000次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/process process

# ---- 只读条目：<对象>: <条目名> { 转换代码 } <内部目录> ----
# 无转换逻辑的条目使用空大括号 {}，直接透传内部数据
process: pid { } /internal/proc/pid
process: ppid { } /internal/proc/ppid
process: name { } /internal/proc/name
process: state { } /internal/proc/state
process: proc_type { } /internal/proc/type
process: proc_class { } /internal/proc/class
process: thread_count { return len(self.threads) } /internal/proc/thread_count
process: stat.runtime_ms { } /internal/proc/stat/runtime
process: stat.exit_code { } /internal/proc/stat/exit_code
process: console { } /internal/proc/console

# ---- 可写控制条目：转换代码为Python，实现参数校验与内部指令下发 ----
process: kill { self.terminate(pid) } /internal/proc/ctl/kill
process: suspend { self.suspend(pid) } /internal/proc/ctl/suspend
process: resume { self.resume(pid) } /internal/proc/ctl/resume
process: set_class { self.set_class(pid, class) } /internal/proc/ctl/set_class
process: console_write { self.console_write(pid, text) } /internal/proc/ctl/console_write
process: console_attach { self.console_attach(pid, target) } /internal/proc/ctl/console_attach

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    # 对外标准事件(标准参数) ｜ 内部原生事件源
    proc_created(pid: u64, ppid: u64, name: str) ｜ /internal/event/proc_new
    proc_exited(pid: u64, exit_code: i32) ｜ /internal/event/proc_exit
    proc_crashed(pid: u64, reason: str) ｜ /internal/event/proc_crash
    oom_killed(pid: u64) ｜ /internal/event/proc_oom

# ---- 单接口独立限流规则（覆写全局限流，保持原规范） ----
LIMIT_OVERRIDE:
    kill: 100次/秒
    set_class: 50次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl，保持原规范） ----
ACL_OVERRIDE:
    kill: core_system
    suspend: core_system
    resume: core_system
    set_class: core_system
    console_write: user_foreground
    console_attach: user_foreground
```

契约要点：

1. 外部进程仅能访问 `/system/api/process` 标准虚拟对象，禁止直连 `/object/proc/<pid>` 内部条目。
2. `kill`、`suspend`、`resume`、`set_class` 等控制接口仅授权 `core_system` 分类进程调用。
3. 事件仅封装契约声明的标准参数，内部私有字段、调试字段全部剥离（`.api.md` 第 5.3 节）。
4. 主版本变更代表接口不兼容；内部私有路径可任意重构，只要版本号不变，外部无感知。
5. 条目的转换代码以 Python 编写：无转换逻辑用空大括号 `{}`；有校验/转换/指令下发逻辑则写入大括号内。
6. console / console_write / console_attach：进程文字输出控制台；图形模式下写入字符渲染到该进程窗口（第 8 节）。

## 10 与各子系统对接

1. **os.md**：进程是纯控制对象；进程对象受对象管理器统一生命周期管控。
2. **acl.md**：进程对象的读、写、创建、删除受两级 ACL；`PROC_TYPE_SERVICE` 标记不等于 ACL 授权。
3. **cpu.md**：进程调度分类、权重、核簇偏好、配额、亲和、节流挂接进程对象；线程可覆写。
4. **memory.md**：进程配额、压力事件、OOM 优先级按分类协同；进程创建/销毁触发配额重计算。
5. **ipc.md**：进程是对象句柄的持有者；句柄传递、引用计数、崩溃回收由对象管理器与进程子系统联动。
6. **fs.md**：进程 fd 表；虚拟节点 fd 内部保存对象句柄；close/dup 管理引用计数。
7. **log.md**：进程独立日志等级、模块标记；进程启停/崩溃写入审计日志。
8. **net.md**：进程句柄表保存套接字对象句柄；进程崩溃自动回收套接字。
9. **ui.md**：进程消亡自动清理关联窗口对象、图形缓冲引用。
10. **hardware_svc.md**：服务身份标记、注册、绑定、崩溃回收与进程生命周期绑定。
11. **.api.md**：进程对外接口全部经 `process.api` 契约与桥接层；进程销毁自动卸载契约。
12. **boot.md**：init（PID=1）由内核对象管理器在基座阶段创建；后续全部进程由用户态派生。
13. **objmgr.md**：进程对象由对象管理器创建、登记、回收；句柄、引用计数由对象管理器承载。
14. **dllo.md**：进程加载动态链接库对象（DLLO）经 dllo.md 规范。
15. **exeo.md**：可执行对象（.exeo）启动经 exeo.md 规范。
16. **tpmeo.md**：临时文件对象（.tpmeo）生命周期经 tpmeo.md 规范。
17. **procmon.md**：进程资源监视器以只读快照对象展示进程与资源占用，结束进程经 ACL 校验。
18. **account.md**：进程令牌来源与账户角色、派生继承、提权不继承、服务按运行身份取令牌（account.md）。

## 11 设计取舍

1. 进程对象承载全部子系统条目分支，避免每个子系统各自发明一套进程管理接口。
2. 条目布局交由 `process.api` 契约声明，文档不硬编码，保证接口可版本化、可动态加载。
3. 生命周期、崩溃回收集中在对象管理器与进程子系统，保证资源不泄漏。
4. 分类标签与 CPU、内存子系统同源，实现"算力、内存、OOM"策略行为一致。
5. 对外只暴露 `/system/api/process` 标准接口，内部 PCB、私有条目完全隐藏，缩小攻击面。
