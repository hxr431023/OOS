# update.md 系统更新子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；系统更新为**用户态服务**，运行于 Ring3，经标准接口与系统交互；服务以**更新权限账户**（`update`，见 `account.md`）身份运行，只授予更新所需最小权限。
> 2. Ring1/Ring2 闲置；系统更新受 `acl.md` 权限管控；更新事件写入审计日志（`log.md`）。
> 3. **条目不在此文档硬编码**：系统更新对外条目的集合、命名、权限由 `update.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义系统更新的行为语义，契约全文见第 8 节。
> 4. 对应目录树 `/system/update/`：`update_service`（更新后台守护）、`checker`（更新检测工具）、`rollback_rules`（回滚策略）、`verify`（更新包签名校验工具）。
> 5. 依赖：`os.md`、`acl.md`、`process.md`、`fs.md`、`log.md`、`boot.md`、`pkg.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 更新包 | 系统级更新单元：内核/服务/系统组件的新版本封装，含签名与校验信息 |
| 更新检查 | 检测是否有可用更新：版本比对、渠道检查 |
| 回滚 | 更新失败后恢复到更新前状态；回滚策略位于 `rollback_rules` |
| 签名校验 | 验证更新包来源与完整性（`verify` 工具） |

## 1 定位与职责

系统更新服务（`/system/update/update_service`）承担：

1. 检查系统更新（内核、服务、系统组件）；
2. 下载、校验、安装更新包；
3. 更新失败自动回滚；
4. 与 `pkg.md` 软件包升级协同（包级 vs 系统级）；
5. 更新记录审计留痕。

## 2 核心对象模型

系统更新以**更新管理对象** `/object/update/mgr`（纯控制对象）为核心载体，内部持有以下条目（**布局由 `update.api` 契约声明**）：

1. `status`：只读，当前更新状态、已安装版本、可用版本。
2. `check`：可写，触发更新检查。
3. `apply`：可写，应用指定更新包。
4. `rollback`：可写，回滚到指定历史版本。
5. `channel`：可读可写，更新渠道配置（稳定/测试）。
6. `components`：只读，可独立更新的系统部件清单（名称、当前版本、可用版本、状态）。
7. `update_component`：可写，触发更新**某一个单独的系统部件**（不触碰其他部件）。
8. `cancel`：可写，取消进行中的更新（全量或部件级）。

## 3 更新流程

1. 调用方向更新管理对象 `check` 条目写入检查请求；更新服务比对版本，返回可用更新列表。
2. 调用方向 `apply` 条目写入目标更新；校验调用方 ACL 权限。
3. 下载更新包 → `verify` 签名与完整性校验 → 失败则终止。
4. 备份当前版本（供回滚）→ 安装更新（内核/服务/组件）。
5. 安装完成更新 `status`，写入审计日志（更新包、版本、执行者）。
6. 涉及内核/服务的更新可能需要重启生效；更新服务标记待重启状态。
7. **部件级更新（update_component）**：外部程序 / 用户可只更新**某一个单独的系统部件**——内核、合成器 / 窗口管理器、服务、系统应用等；只下载、校验、替换该部件，不触碰其他部件。
8. 部件清单经 `components` 条目查询；更新前校验部件依赖（`deps`）与目标版本兼容（如服务依赖内核接口版本），不兼容拒绝。
9. 部件级更新同样走签名校验与备份回滚；进度 / 完成 / 失败经部件级事件上报（`update_progress` / `component_updated` / `component_failed`）。

## 4 回滚流程

1. 更新安装失败或启动异常：更新服务按 `rollback_rules` 策略触发回滚。
2. 回滚恢复更新前备份版本，验证系统可运行。
3. 回滚操作同样写入审计日志。

## 5 容错与安全

1. 下载中断：断点续传或重新下载，不使用损坏包。
2. 签名校验失败：拒绝安装，报告来源异常。
3. 安装失败：自动回滚，保持系统原状态。
4. 更新服务崩溃：init 重启；更新任务状态持久化，恢复后继续或终止。

## 6 与各子系统对接

1. **os.md**：更新管理对象为纯控制对象，复用对象‑条目模型。
2. **acl.md**：更新检查/应用/回滚受 ACL 管控。
3. **process.md**：安装脚本以子进程执行，遵循进程生命周期。
4. **fs.md**：更新包落盘遵循 `/system/update` 目录树。
5. **log.md**：更新/回滚/校验失败写入审计日志。
6. **boot.md**：涉及内核/服务的更新需重启生效。
7. **pkg.md**：系统更新与软件包升级协同。
8. **.api.md**：系统更新对外接口经 `update.api` 契约与桥接层。

## 7 容错总结

1. 更新失败 → 自动回滚，保证系统可启动可运行。
2. 签名校验失败 → 拒绝安装。
3. 服务崩溃 → init 重启，任务状态持久化。
4. 更新中断 → 不残留半成品状态。

## 8 系统更新 `.api` 契约文件（update.api 全文）

系统更新对外接口统一由以下契约文件声明，文件存放于 `/system/api/update.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# update.api 系统更新子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = update
env = python
exec = /system/update/update_service
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 50次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/update update

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
update: status { } /internal/update/status
update: available { } /internal/update/available
update: components { } /internal/update/components

# ---- 可写控制条目：转换代码为Python，实现更新操作 ----
update: check {
    return self.check_update()
} /internal/update/check
update: apply {
    if not self.verify_sig(pkg_path):
        raise PermissionError("签名校验失败")
    self.apply(pkg_path)
} /internal/update/apply
update: rollback { self.rollback(version) } /internal/update/rollback
update: update_component { self.update_component(component) } /internal/update/ctl/update_component
update: cancel { self.cancel() } /internal/update/ctl/cancel

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    update_available(version: str) ｜ /internal/event/update_avail
    update_applied(version: str) ｜ /internal/event/update_done
    update_failed(version: str, reason: str) ｜ /internal/event/update_fail
    update_rollback(version: str) ｜ /internal/event/update_rollback
    update_progress(component: str, percent: u8) ｜ /internal/event/update_progress
    component_updated(component: str, version: str) ｜ /internal/event/comp_updated
    component_failed(component: str, reason: str) ｜ /internal/event/comp_failed

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    apply: 5次/秒
    rollback: 5次/秒
    update_component: 5次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    apply: aux_system
    rollback: aux_system
    update_component: aux_system
```

契约要点：

1. 外部程序仅能访问 `/system/api/update` 标准虚拟对象，禁止直连更新管理对象内部条目。
2. `apply` 接口经 Python 转换代码先执行签名校验，失败直接拒绝。
3. 事件仅封装契约声明的标准参数；更新可用/完成/失败/回滚事件供系统管理工具监听。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。
5. `components` / `update_component` / `cancel`：外部程序可查询部件清单并触发**单个系统部件**更新；部件更新同样经签名校验与依赖兼容校验（第 3 节）。

## 9 设计取舍

1. 更新服务为独立用户态服务，崩溃可重启，不影响内核。
2. 签名校验前置在桥接层，杜绝未经验证的更新包安装。
3. 自动备份与回滚机制保证更新失败不破坏系统。
4. 对外只暴露 `/system/api/update` 标准接口，内部更新状态与备份完全隐藏。
