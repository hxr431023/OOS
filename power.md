# power.md 电源与休眠子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；电源与休眠为**用户态服务**（内核 ACPI 底层由内核模块承担），运行于 Ring3。
> 2. Ring1/Ring2 闲置；电源/休眠操作受 `acl.md` 权限管控；敏感操作写入审计日志（`log.md`）。
> 3. **条目不在此文档硬编码**：电源休眠对外条目的集合、命名、权限由 `power.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义电源休眠的行为语义，契约全文见第 8 节。
> 4. 对应目录树 `/system/suspend/`：`suspend_daemon`（待机/休眠管理守护进程）、`hibernate_tools`（休眠镜像校验、处理工具）、`power_rules`（电源策略、唤醒源白名单）、`config`（休眠压缩参数、唤醒脚本）、`data`（运行时休眠镜像、进程转存快照）。
> 5. 依赖：`os.md`、`acl.md`、`process.md`、`fs.md`、`log.md`、`cpu.md`、`boot.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 待机（Suspend） | 低功耗状态，内存保持供电，系统状态驻留内存，恢复快 |
| 休眠（Hibernate） | 系统状态写入磁盘镜像后断电，恢复时从镜像加载 |
| 唤醒源 | 允许唤醒系统的设备/事件白名单 |
| 休眠镜像 | 休眠时保存的系统内存快照，位于 `/system/suspend/data` |
| 电源策略 | 全局电源策略：性能/平衡/节能，联动 `cpu.md` 调度策略 |

## 1 定位与职责

电源休眠守护进程（`/system/suspend/suspend_daemon`）承担：

1. 待机/休眠状态管理与切换；
2. 休眠镜像创建、校验、恢复（`hibernate_tools`）；
3. 电源策略与唤醒源管理（`power_rules`）；
4. 休眠压缩参数与唤醒脚本（`config`）；
5. 电源事件审计留痕。

## 2 核心对象模型

电源休眠以**电源管理对象** `/object/power/mgr`（纯控制对象）为核心载体，内部持有以下条目（**布局由 `power.api` 契约声明**）：

1. `state`：只读，当前电源状态：运行/待机/休眠。
2. `sleep`：可写，触发待机或休眠。
3. `wakeup`：可写，设置唤醒源白名单。
4. `policy`：可读可写，电源策略（性能/平衡/节能）。
5. `event`：等待可读，电源事件：待机/休眠完成、唤醒、电量告警。

## 3 待机流程

1. 调用方向电源管理对象 `sleep` 条目写入待机请求（联动 `cpu.md` powersave 策略）。
2. 系统挂起各子系统：暂停非关键进程、刷新文件缓存、进入低功耗。
3. 进入待机；`state` 置为待机。
4. 唤醒源触发（`wakeup` 白名单）→ 系统恢复运行，`state` 置回运行。
5. 写入审计日志（进入/退出待机时间、唤醒源）。

## 4 休眠流程

1. 调用方向 `sleep` 条目写入休眠请求。
2. 系统创建休眠镜像：将内存状态与进程转存写入 `/system/suspend/data`（`hibernate_tools` 校验、压缩）。
3. 断电；恢复时引导加载休眠镜像，恢复系统状态。
4. 镜像损坏：`hibernate_tools` 检测并提示，系统按正常冷启动。
5. 写入审计日志（休眠/恢复时间、结果）。

## 5 电源策略与唤醒源

1. **电源策略**：`policy` 条目配置性能/平衡/节能；联动 `cpu.md` 调度策略、`ui.md` 显示亮度。
2. **唤醒源白名单**：`wakeup` 条目配置允许唤醒的设备/事件（键盘、网络唤醒、定时器）。
3. 未在白名单的唤醒事件被忽略，防误唤醒。
4. 电量告警事件写入 `event` 供系统提示用户。

## 6 与各子系统对接

1. **os.md**：电源管理对象为纯控制对象，复用对象‑条目模型。
2. **acl.md**：休眠/唤醒源/策略配置受 ACL 管控。
3. **process.md**：休眠镜像含进程转存；恢复时按进程快照恢复。
4. **fs.md**：休眠镜像落盘遵循 `/system/suspend/data`。
5. **log.md**：电源事件、休眠异常写入审计日志。
6. **cpu.md**：电源策略联动调度策略（powersave）。
7. **boot.md**：恢复启动路径与正常启动路径分离。
8. **.api.md**：电源休眠对外接口经 `power.api` 契约与桥接层。

## 7 容错与安全

1. 休眠镜像损坏：检测后放弃恢复，按冷启动处理，不卡死。
2. 待机/休眠过程中断：自动回到运行状态并告警。
3. 唤醒源白名单缺失：默认仅允许电源键等基础唤醒源。
4. 休眠镜像包含敏感数据：镜像文件权限受限，受 `security.md` 保护。

## 8 电源休眠 `.api` 契约文件（power.api 全文）

电源休眠对外接口统一由以下契约文件声明，文件存放于 `/system/api/power.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# power.api 电源休眠子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = power
env = python
exec = /system/suspend/suspend_daemon
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 50次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/power power

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
power: state { } /internal/power/state

# ---- 可写控制条目：转换代码为Python，实现电源操作 ----
power: sleep {
    if mode not in ("suspend", "hibernate"):
        raise ValueError("非法电源模式")
    self.sleep(mode)
} /internal/power/sleep
power: wakeup {
    self.set_wakeup_sources(sources)
} /internal/power/wakeup
power: set_policy { self.set_policy(policy) } /internal/power/policy

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    sleep_done(mode: str) ｜ /internal/event/sleep_done
    wakeup_done(source: str) ｜ /internal/event/wakeup_done
    battery_low(level: u32) ｜ /internal/event/battery_low
    hibernate_failed(reason: str) ｜ /internal/event/hibernate_fail

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    sleep: 5次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    sleep: aux_system
    wakeup: aux_system
```

契约要点：

1. 外部程序仅能访问 `/system/api/power` 标准虚拟对象，禁止直连电源管理对象内部条目。
2. `sleep` 接口经 Python 转换代码校验电源模式（suspend/hibernate），非法拒绝。
3. 事件仅封装契约声明的标准参数；休眠/唤醒/电量告警事件供系统管理监听。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 9 设计取舍

1. 电源休眠为独立用户态服务，崩溃可重启，不影响运行态。
2. 休眠镜像校验前置，损坏镜像不阻塞冷启动。
3. 唤醒源白名单限制，防误唤醒、省电。
4. 对外只暴露 `/system/api/power` 标准接口，内部休眠镜像与策略完全隐藏。

## 10 关机流程

系统关机遵循固定顺序，逐层关闭以保证数据与状态安全落盘：

1. **关闭应用**：关闭全部用户应用，应用经自身 API 优雅退出并保存状态。
2. **通知 Linux 兼容层关闭 Linux 应用**：通知 Linux 兼容层（`Linux.md`）关闭其管理的 Linux 应用，兼容层执行对应关闭流程。
3. **关闭系统附属不重要程序**：关闭系统附属的非关键程序（后台服务、工具进程等）。
4. **关闭系统**：最后关闭系统核心——停止系统服务、写回缓存、卸载文件系统、调用 ACPI 关机断电（联动 `boot.md` 与内核）。
