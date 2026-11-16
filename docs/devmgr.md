# devmgr.md 设备管理器子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；设备管理器为**用户态服务**，运行于 Ring3，负责设备发现、服务匹配、绑定编排；服务匹配按归属判据选择内核扩展文件或用户态服务。
> 2. Ring1/Ring2 闲置；设备管理操作受 `acl.md` 权限管控；敏感操作写入审计日志（`log.md`）。
> 3. **条目不在此文档硬编码**：设备管理器对外条目的集合、命名、权限由 `devmgr.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义设备管理器的行为语义，契约全文见第 7 节。
> 4. 对应目录树 `/system/devmgr/`：`manager`（设备管理器主程序）、`match_rules`（硬件服务匹配规则）、`config`（设备管理配置）。**第一方服务嵌入内核态，位于内核附属文件夹 `kernel/aux_modules/`；第三方服务不固定位置，可存放于系统任意路径。**
> 5. 依赖：`os.md`、`acl.md`、`hardware_svc.md`、`external_dev.md`、`process.md`、`log.md`、`boot.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 设备管理器 | 统一管理设备发现、服务匹配、绑定编排的用户态服务 |
| 匹配规则 | 硬件设备与服务程序的匹配规则（厂商 ID、类别、接口），位于 `match_rules` |
| 官方服务 | 系统第一方服务，嵌入内核态，位于内核附属文件夹 `kernel/aux_modules` |
| 第三方服务 | 第三方用户态服务，可位于系统任意路径，不固定目录 |

## 1 定位与职责

设备管理器（`/system/devmgr/manager`）承担：

1. 设备发现：监听内核设备枚举事件，维护设备清单；
2. 服务匹配：按 `match_rules` 匹配硬件与可用服务；
3. 绑定编排：拉起匹配服务进程并完成硬件绑定（复用 `hardware_svc.md`）；
4. 服务管理：第一方（内核态）与第三方服务加载、版本、状态查询；
5. 设备/服务事件审计留痕。

## 2 设备发现与匹配流程

1. 内核基座阶段枚举 PCI 硬件对象（`boot.md` 阶段 4）；热插拔外设由内核创建设备对象（`external_dev.md`）。
2. 设备管理器监听设备对象 `event`，发现新设备后按 `match_rules` 匹配服务。
3. 匹配依据：厂商 ID、产品 ID、设备类别、接口描述。
4. 匹配成功：拉起对应服务进程（`process.md` 派生）——第一方服务嵌入内核态、位于内核附属文件夹 `kernel/aux_modules`；第三方服务按安装位置加载（可位于任意路径），服务完成注册与绑定（`hardware_svc.md`）。
5. 匹配失败或无服务：设备保持未绑定状态，标记待服务，系统管理工具可查询。

## 3 服务管理

1. **第一方服务嵌入内核态**，存放于内核附属文件夹 `kernel/aux_modules`，随内核装载；**第三方服务可位于系统任意路径**，不固定目录。
2. 第三方服务安装需授权（联动 `pkg.md` 安装规则）。
3. 服务状态查询：已加载、已绑定设备、版本、健康状态。
4. 服务崩溃：设备管理器按 `process.md` 回收，可重新拉起（按策略）。

## 4 与各子系统对接

1. **os.md**：设备管理对象为纯控制对象，复用对象‑条目模型。
2. **acl.md**：设备绑定、服务管理受 ACL 管控。
3. **hardware_svc.md**：服务注册、绑定、回收完全复用用户态服务规范。
4. **external_dev.md**：外部设备热插拔、设备对象创建协同。
5. **process.md**：服务进程生命周期、崩溃回收。
6. **log.md**：设备接入/移除、服务绑定/崩溃写入审计日志。
7. **boot.md**：设备管理器由 init 拉起；硬件对象启动阶段已枚举。
8. **.api.md**：设备管理器对外接口经 `devmgr.api` 契约与桥接层。

## 5 设备管理器 `.api` 契约文件（devmgr.api 全文）

设备管理器对外接口统一由以下契约文件声明，文件存放于 `/system/api/devmgr.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# devmgr.api 设备管理器子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = devmgr
env = python
exec = /system/devmgr/manager
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 100次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/devmgr devmgr

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
devmgr: devices { } /internal/devmgr/devices
devmgr: services { } /internal/devmgr/services
devmgr: match_rules { } /internal/devmgr/match_rules

# ---- 可写控制条目：转换代码为Python，实现设备管理 ----
devmgr: scan { return self.scan() } /internal/devmgr/scan
devmgr: bind_service {
    self.bind(dev_id, service)
} /internal/devmgr/bind
devmgr: set_rule {
    self.set_rule(rule)
} /internal/devmgr/match/rule

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    dev_found(dev_id: u64, dev_type: str) ｜ /internal/event/dev_found
    service_bound(dev_id: u64, service: str) ｜ /internal/event/service_bound
    service_missing(dev_id: u64) ｜ /internal/event/service_missing

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    scan: 5次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    bind_service: aux_system
    set_rule: aux_system
```

契约要点：

1. 外部程序仅能访问 `/system/api/devmgr` 标准虚拟对象，禁止直连设备管理对象内部条目。
2. `bind_service`、`set_rule` 仅授权 `aux_system` 分类进程调用。
3. 事件仅封装契约声明的标准参数；设备发现、服务绑定、服务缺失事件供管理工具监听。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 6 设计取舍

1. 设备管理器为独立用户态服务，崩溃可重启，不影响内核设备对象。
2. 服务匹配规则集中配置，设备接入自动编排，无需人工干预。
3. 绑定/规则修改经 ACL 与审计双重管控。
4. 对外只暴露 `/system/api/devmgr` 标准接口，内部匹配逻辑完全隐藏。
