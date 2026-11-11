# service.md 服务管理子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；服务管理为**用户态服务**，运行于 Ring3，负责系统守护进程的托管、自启、重启；本文档的用户态服务即本系统的驱动（性能收益不足或易出错的部分）。
> 2. Ring1/Ring2 闲置；服务管理操作受 `acl.md` 权限管控；服务启停写入审计日志（`log.md`）。
> 3. **条目不在此文档硬编码**：服务管理对外条目的集合、命名、权限由 `service.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义服务管理的行为语义，契约全文见第 8 节。
> 4. 对应目录树 `/system/userland/services/`：`manager`（服务管理器，命令行 + 图形界面）、`loader`（服务加载器，二进制程序，加载 `.service` 服务文件）、`host`（服务运行打包容器）、`daemons`（全部常驻后台守护进程）、`config`（服务自启、权限配置）。
> 5. 依赖：`os.md`、`acl.md`、`process.md`、`fs.md`、`log.md`、`boot.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 服务 | 常驻后台运行的守护进程，为系统或其它进程提供能力 |
| 服务管理器 | 统一管理服务生命周期的管理程序，提供**命令行 + 用户图形界面**（参考 Windows SCM / sc.exe / services.msc） |
| 服务加载器 | 二进制程序，负责加载 `.service` 服务文件、校验并交给服务运行打包容器执行 |
| 服务运行打包容器 | 服务实际运行的进程单元，可将多个不冲突服务打包为一个进程（每服务一个线程，参考 Windows svchost.exe） |
| 服务文件 | 服务二进制程序，后缀 .service，由服务加载器加载 |
| 自启配置 | 服务随系统启动自动拉起与否的配置，位于 `services/config` |
| 权限配置 | 服务运行身份、ACL 标签的配置 |

## 1 定位与职责

服务子系统由**三个不同的部分**组成：

1. **服务管理器**（`services/manager`）：统一管理服务生命周期，提供**命令行 + 用户图形界面**两种入口（类 Windows SCM / sc.exe / services.msc）；负责注册、启停、重启、自启配置、状态查询。
2. **服务加载器**（`services/loader`）：**二进制程序**，读取并校验 `.service` 服务文件，解析依赖与运行身份，交给服务运行打包容器执行。
3. **服务运行打包容器**（`services/host`）：服务实际运行的进程单元；多个**不冲突**服务打包到同一进程（每服务一个线程），类似 Windows svchost.exe；冲突或需隔离的服务独立进程运行。

三者共同承担：

1. 服务注册、启停、重启、状态查询；
2. 按自启配置随系统启动拉起服务（`services/config`）；
3. 服务进程崩溃自动重启（按策略）；
4. 服务运行身份与权限管理；
5. 服务启停事件审计留痕。


## 2 核心对象模型

服务管理以**服务管理对象** `/object/service/mgr`（纯控制对象）为核心载体，内部持有以下条目（**布局由 `service.api` 契约声明**）：

1. `list`：只读，已注册服务列表、状态（运行/停止/异常）、自启标记。
2. `start`：可写，启动服务请求。
3. `stop`：可写，停止服务请求。
4. `restart`：可写，重启服务请求。
5. `auto_start`：可读可写，服务自启配置。
6. `event`：等待可读，服务事件：启停、崩溃、重启。

## 3 服务启动流程

1. 系统启动时，**用户态主程序**（init，`boot.md` 阶段 6）通知服务管理器，读取自启配置（`services/config`），逐项拉起服务。
2. **服务加载器**加载 `.service` 服务文件，校验签名与依赖，确定运行身份，交由**服务运行打包容器**执行。
3. **打包容器**：多个**不冲突**服务（资源无竞争、权限互不干扰、无需独立崩溃隔离）合入**同一进程**，每个服务一个**线程**运行（类似 Windows 服务主进程 svchost.exe）；冲突或需隔离的服务单独进程运行。
4. 服务注册至服务管理对象 `list`，状态置为运行。
5. 服务异常退出：服务管理器按策略自动重启（记录次数，超过阈值停止重启并告警）；打包容器内单服务线程异常不影响容器内其它服务线程（按服务隔离策略）。
6. 服务启停写入审计日志（服务名、动作、执行者）。

## 4 服务停止与重启流程

1. **停止**：向 `stop` 条目写入服务名；校验权限；服务运行打包容器终止对应服务线程（独立进程服务则终止进程），并按 `process.md` 回收资源。
2. **重启**：向 `restart` 条目写入服务名；先停止再按启动流程拉起。
3. 手动启停与自动重启均记录审计日志。

## 5 容错与策略

1. 服务崩溃：按策略自动重启；连续崩溃触发熔断（停止重启并告警）。
2. 服务管理器崩溃：init 重启服务管理器，重新接管服务状态（服务进程本身保留）。
3. 服务加载器 / 服务运行打包容器崩溃：由服务管理器或 init 重启，已托管服务按策略恢复。
4. 自启失败：记录日志，不阻塞系统其它服务拉起。

## 6 与各子系统对接

1. **os.md**：服务管理对象为纯控制对象，复用对象‑条目模型。
2. **acl.md**：服务启停、自启配置受 ACL 管控。
3. **process.md**：服务进程生命周期、崩溃回收由进程子系统承载。
4. **fs.md**：服务可执行与配置文件遵循 `/system/userland` 目录树。
5. **log.md**：服务启停、崩溃、重启写入审计日志。
6. **boot.md**：用户态主程序（init）通知服务管理器，打开启动常驻程序并管理；自启服务随启动流程执行。
7. **.api.md**：服务管理对外接口经 `service.api` 契约与桥接层。
8. **hardware_svc.md**：用户态第三方服务（硬件服务）注册、绑定、崩溃回收机制见该文档，与服务管理机体系相互独立。

## 7 容错总结

1. 服务崩溃 → 自动重启 + 熔断保护。
2. 服务管理器 / 加载器 / 打包容器崩溃 → init 重启接管。
3. 自启失败 → 记录日志，不阻塞整体启动。

## 8 服务管理 `.api` 契约文件（service.api 全文）

服务管理对外接口统一由以下契约文件声明，文件存放于 `/system/api/service.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# service.api 服务管理子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = service
env = python
exec = /system/userland/services/manager
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 100次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/service service

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
service: list { } /internal/service/list
service: state { } /internal/service/state

# ---- 可写控制条目：转换代码为Python，实现服务操作 ----
service: start {
    self.start(name)
} /internal/service/start
service: stop { self.stop(name) } /internal/service/stop
service: restart { self.restart(name) } /internal/service/restart
service: set_auto {
    self.set_auto(name, enabled)
} /internal/service/auto

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    svc_started(name: str) ｜ /internal/event/svc_start
    svc_stopped(name: str) ｜ /internal/event/svc_stop
    svc_crashed(name: str, restart_count: u32) ｜ /internal/event/svc_crash
    svc_fuse(name: str) ｜ /internal/event/svc_fuse

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    start: 50次/秒
    restart: 50次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    start: aux_system
    stop: aux_system
    restart: aux_system
```

契约要点：

1. 外部程序仅能访问 `/system/api/service` 标准虚拟对象，禁止直连服务管理对象内部条目。
2. 服务启停、重启仅授权 `aux_system` 分类进程调用。
3. 事件仅封装契约声明的标准参数；服务崩溃、熔断事件供系统管理监听。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 9 设计取舍

1. 服务管理为独立用户态服务，崩溃可重启，服务进程不受影响。
2. 服务生命周期托管统一，崩溃自动重启 + 熔断，提升系统健壮性。
3. 启停操作经 ACL 与审计双重管控。
4. 对外只暴露 `/system/api/service` 标准接口，内部服务托管细节完全隐藏。
