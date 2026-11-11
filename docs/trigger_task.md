# trigger_task.md 触发任务器规范（x86‑64）

> 前提：
> 1. **触发任务器**（trigger tasker）对应 Windows 任务计划程序（Task Scheduler）：按计划 / 条件触发任务。
> 2. 由**用户态主程序**（init，`boot.md` 阶段 6）打开启动的**常驻前台程序**之一，与服务管理机、服务运行容器并列。
> 3. 触发动作：启动服务 / 应用、执行命令 / 脚本、发送系统事件（`service.md`、`process.md`、`ipc.md`）。
> 4. 依赖：`boot.md`、`service.md`、`process.md`、`ipc.md`、`acl.md`、`log.md`、`.api.md`。

## 1 定位与职责

1. 注册、查询、启停触发任务；
2. 按计划 / 条件检测并触发任务（时间、开机、事件、条件）；
3. 执行触发动作：启动服务 / 应用、执行命令 / 脚本、发送事件；
4. 执行结果记录与审计（`log.md`）。

## 2 触发类型

1. **时间触发**：一次性（指定时刻）、周期（每天 / 每周 / 每月 / 间隔）、日历（特定日期）；时间表达式由配置声明（类 cron 语法）。
2. **开机触发**：系统启动 / 用户登录后触发一次。
3. **事件触发**：系统事件、服务事件、对象事件（`ipc.md` EVENT）满足时触发。
4. **条件触发**：系统空闲、网络可用、电源状态等条件满足时触发（`hwsched`、`net.md`、`power.md`）。

## 3 任务对象模型

触发任务以**任务对象** `/object/trigger/<task_id>`（纯控制对象）承载，条目：

1. `id`：只读，任务唯一标识。
2. `type`：只读，触发类型（time / boot / event / condition）。
3. `trigger`：只读，触发条件描述（时间表达式 / 事件名 / 条件）。
4. `action`：只读，触发动作（启动服务 / 应用、命令、事件）。
5. `state`：只读，任务状态（enabled / disabled / running）。
6. `last_result`：只读，上次执行结果（success / failed / timeout）。

## 4 触发流程

1. **注册**：向触发任务器提交任务（类型、触发条件、动作、权限），创建任务对象。
2. **等待**：触发任务器按类型等待触发条件（时间轮询 / 事件监听 / 条件检测）。
3. **触发**：条件满足，校验 ACL 后执行动作（启动服务 / 应用、执行命令、发送事件）。
4. **记录**：执行结果写入任务对象与审计日志（`log.md`）。

## 5 与各子系统对接

1. **boot.md**：由用户态主程序（init）打开启动，作为常驻前台程序之一。
2. **service.md**：触发动作可启动 / 停止服务；触发任务器与服务管理机并列。
3. **process.md**：触发动作执行命令 / 脚本时派生进程。
4. **ipc.md**：事件触发监听系统事件；触发动作可发送事件。
5. **acl.md**：任务注册与触发动作受 ACL 管控。
6. **log.md**：任务注册、触发、执行结果写入审计日志。
7. **power.md / net.md / hwsched**：条件触发依赖电源、网络、负载状态。

## 6 设计取舍

1. 触发任务器为**用户态常驻程序**，不进入内核；触发执行经服务 / 进程接口，权限可控。
2. 时间触发采用秒级最小粒度调度；时间表达式由配置声明（类 cron），不内置完整 cron 实现。
3. 事件触发复用 `ipc.md` 事件条目，不引入独立事件总线。

## 7 触发任务器 `.api` 契约文件（trigger_task.api 全文）

```text
# trigger_task.api 触发任务器对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = trigger_task
env = python
exec = /system/userland/trigger/trigger_task
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 50次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/trigger_task trigger_task

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
trigger_task: task_list { } /internal/trigger/list
trigger_task: task_state { } /internal/trigger/state
trigger_task: trigger_log { } /internal/trigger/log

# ---- 可写控制条目：转换代码为Python，实现任务控制 ----
trigger_task: task_register { return self.register(task) } /internal/trigger/register
trigger_task: task_unregister { return self.unregister(task_id) } /internal/trigger/unregister
trigger_task: task_enable { return self.enable(task_id) } /internal/trigger/enable
trigger_task: task_disable { return self.disable(task_id) } /internal/trigger/disable
trigger_task: task_run_now { return self.run_now(task_id) } /internal/trigger/run_now

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    task_triggered(task_id: str) ｜ /internal/event/task_triggered
    task_completed(task_id: str, result: str) ｜ /internal/event/task_completed
    task_failed(task_id: str, reason: str) ｜ /internal/event/task_failed

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    task_run_now: 10次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    task_register: user_foreground
```

契约要点：

1. 任务对象内部路径 `/object/trigger/<task_id>`，与本文第 3 节任务对象一致。
2. 只读字段透传任务列表 / 状态 / 执行日志；写字段为任务控制命令（register / unregister / enable / disable / run_now）。
3. 全局限流 50 次/秒；任务触发、执行结果写入审计日志（audit=true）。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。