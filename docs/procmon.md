# procmon.md 进程资源监视器规范（x86‑64）

> 前提：
> 1. **进程资源监视器**（procmon）对应 Windows 任务管理器 + 资源监视器：进程列表与操作 + CPU / 内存 / 磁盘 / 网络性能监控。
> 2. 系统应用，部署于 `/system/SystemApps/procmon/`（`dirtree.md`）；按需启动，可常驻托盘。
> 3. 进程数据来自进程对象与 CPU / 内存 / IO / 网络统计对象，不直接读取内核私有结构。
> 4. 依赖：`process.md`、`cpu.md`、`memory.md`、`hwsched`、`storage.md`、`net.md`、`acl.md`、`ui.md`、`log.md`、`.api.md`。

## 1 定位与职责

1. **进程列表**：名称、PID、CPU、内存、磁盘 IO、网络、线程数、状态、所属用户；
2. **进程操作**：结束进程、启动应用、查看进程详情（模块 / 句柄）；
3. **性能监控**：CPU / 内存 / 磁盘 / 网络实时曲线（类 Windows 资源监视器）；
4. **资源监视**：句柄、线程、模块占用（类资源监视器）；
5. **高占用标记与告警**。

## 2 数据来源

1. `process.md`：进程对象、状态、生命周期事件；
2. `cpu.md` 与 `hwsched`：CPU 占用、调度统计；
3. `memory.md`：内存占用、配额、压力；
4. `storage.md`：磁盘 IO；
5. `net.md`：网络吞吐与连接。

## 3 进程视图对象条目

进程视图以**只读快照对象** `/object/procmon/<pid>` 承载，条目：

1. `pid`：只读，进程标识。
2. `name`：只读，进程名 / 可执行路径。
3. `cpu`：只读，CPU 占用（百分比）。
4. `mem`：只读，内存占用（字节 / 占比）。
5. `io`：只读，磁盘读写累计与速率。
6. `net`：只读，网络吞吐。
7. `threads`：只读，线程数。
8. `state`：只读，状态（running / sleeping / stopped）。
9. `user`：只读，所属用户 / 会话。

## 4 核心操作流程

1. **启动**：拉取进程快照 → 周期刷新（间隔可配置，默认 1 秒）。
2. **结束进程**：ACL 校验 → 发送终止信号（`process.md`）→ 关键系统进程拒绝并提示。
3. **启动应用**：经应用启动器（`userland/app_launcher`）拉起。
4. **查看详情**：按 PID 展开模块 / 句柄 / 线程。
5. **导出**：进程列表可导出 / 复制。

## 5 与各子系统对接

1. **process.md**：进程对象、状态、终止信号；监控事件订阅。
2. **cpu.md / hwsched**：CPU 占用与负载采样。
3. **memory.md**：内存占用、配额、压力。
4. **storage.md**：磁盘 IO 统计。
5. **net.md**：网络吞吐与连接统计。
6. **acl.md**：结束进程 / 查看系统进程受 ACL 管控。
7. **ui.md**：窗口、图标、实时曲线绘制。
8. **log.md**：结束进程、异常高占用写入审计。
9. **boot.md**：非启动必需组件，登录后按需启动。

## 6 设计取舍

1. 只读监控优先：采样周期可配（默认 1 秒），避免监控自身成为负载。
2. 结束进程必须 ACL 校验；内核关键进程（PID 1 等）拒绝结束。
3. 数据统一来自对象条目与统计接口，不直接访问内核私有结构，符合 `.api.md` 隔离约束。
4. 不内置 GPU 监控（留待 `ui.md` / 显卡服务扩展）。

## 7 进程资源监视器 `.api` 契约文件（procmon.api 全文）

```text
# procmon.api 进程资源监视器对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = procmon
env = python
exec = /system/SystemApps/procmon/procmon_app
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 100次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/procmon procmon

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
procmon: proc_list { } /internal/procmon/list
procmon: proc_detail { } /internal/procmon/detail
procmon: cpu_usage { } /internal/procmon/cpu
procmon: mem_usage { } /internal/procmon/mem
procmon: io_stats { } /internal/procmon/io
procmon: net_stats { } /internal/procmon/net

# ---- 可写控制条目：转换代码为Python，实现进程操作 ----
procmon: kill_process { return self.kill_process(pid) } /internal/procmon/kill
procmon: start_app { return self.start_app(path, args) } /internal/procmon/start
procmon: set_interval { return self.set_interval(ms) } /internal/procmon/interval

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    proc_started(pid: u64, name: str) ｜ /internal/event/proc_started
    proc_exited(pid: u64, code: i32) ｜ /internal/event/proc_exited
    high_usage_alert(pid: u64, kind: str) ｜ /internal/event/high_usage

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    kill_process: 10次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    kill_process: user_foreground
```

契约要点：

1. 只读条目透传进程列表、详情与性能统计；写条目为结束进程 / 启动应用 / 采样间隔设置。
2. 结束进程独立限流 10 次/秒；关键系统进程拒绝执行（`process.md` ACL）。
3. 进程启停、高占用事件供托盘与告警提示（`ui.md`、`log.md`）。
4. 采样数据只读，不暴露句柄内容，符合 `.api.md` 隔离约束。