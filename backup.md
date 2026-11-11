# backup.md 备份与还原子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；备份还原为**用户态服务**，运行于 Ring3。
> 2. Ring1/Ring2 闲置；备份/还原操作受 `acl.md` 权限管控；敏感操作写入审计日志（`log.md`）。
> 3. **条目不在此文档硬编码**：备份还原对外条目的集合、命名、权限由 `backup.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义备份还原的行为语义，契约全文见第 7 节。
> 4. 对应目录树 `/system/backup/`：`backup_daemon`（备份后台守护进程）、`restore_tools`（系统还原工具集）、`rules`（增量/全量备份策略）、`config`（备份保留周期配置）。
> 5. 依赖：`os.md`、`acl.md`、`fs.md`、`log.md`、`storage.md`、`security.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 全量备份 | 对目标数据完整副本的一次备份 |
| 增量备份 | 仅备份自上次备份以来变更的数据 |
| 备份保留周期 | 备份副本的保存时长与轮换策略，位于 `config` |
| 还原点 | 可还原到某一历史备份状态 |

## 1 定位与职责

备份守护进程（`/system/backup/backup_daemon`）承担：

1. 按策略执行全量/增量备份；
2. 备份数据校验与完整性维护；
3. 提供还原点与系统还原能力；
4. 备份保留周期管理与轮换；
5. 备份/还原事件审计留痕。

## 2 核心对象模型

备份还原以**备份管理对象** `/object/backup/mgr`（纯控制对象）为核心载体，内部持有以下条目（**布局由 `backup.api` 契约声明**）：

1. `status`：只读，最近备份时间、备份集状态、保留策略。
2. `backup`：可写，触发备份请求（全量/增量、目标）。
3. `restore`：可写，还原请求（还原点、目标）。
4. `list_points`：只读，可用还原点列表。
5. `config`：可读可写，备份策略与保留周期配置。

## 3 备份流程

1. 调用方向备份管理对象 `backup` 条目写入备份请求（全量/增量、目标路径）。
2. 校验调用方 ACL 权限；按 `rules` 策略确定备份方式。
3. 备份守护进程读取目标数据，写入备份存储（本地卷/外部存储，经 `storage.md`）。
4. 校验备份完整性，更新备份集列表与还原点。
5. 按 `config` 保留周期轮换过期备份。
6. 写入审计日志（备份类型、目标、结果）。

## 4 还原流程

1. 调用方查询 `list_points` 获取还原点。
2. 向 `restore` 条目写入还原请求（还原点、目标）。
3. 校验权限；备份守护进程从备份集恢复数据。
4. 还原完成验证，写入审计日志。
5. 系统级还原（整机）可能需要重启生效（联动 `boot.md`）。

## 5 容错与安全

1. 备份中断：标记备份集不完整，不覆盖有效还原点。
2. 备份校验失败：告警并重试，避免使用损坏备份。
3. 备份数据加密：可选启用，密钥由 `security.md` 管理。
4. 还原操作破坏性大：需授权 + 审计，还原前可再次确认备份完整性。

## 6 与各子系统对接

1. **os.md**：备份管理对象为纯控制对象，复用对象‑条目模型。
2. **acl.md**：备份/还原/策略配置受 ACL 管控。
3. **fs.md**：备份数据读写遵循 VFS 目录树。
4. **log.md**：备份/还原/失败写入审计日志。
5. **storage.md**：备份目标卷管理与健康监控协同。
6. **security.md**：备份数据加密与密钥管理。
7. **.api.md**：备份还原对外接口经 `backup.api` 契约与桥接层。

## 7 备份还原 `.api` 契约文件（backup.api 全文）

备份还原对外接口统一由以下契约文件声明，文件存放于 `/system/api/backup.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# backup.api 备份还原子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = backup
env = python
exec = /system/backup/backup_daemon
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 50次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/backup backup

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
backup: status { } /internal/backup/status
backup: list_points { } /internal/backup/points

# ---- 可写控制条目：转换代码为Python，实现备份还原操作 ----
backup: backup {
    self.backup(backup_type, target)
} /internal/backup/run
backup: restore {
    if not self.verify_point(point_id):
        raise ValueError("还原点无效")
    self.restore(point_id, target)
} /internal/backup/restore
backup: set_policy { self.set_policy(policy) } /internal/backup/policy

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    backup_done(backup_id: u64) ｜ /internal/event/backup_done
    backup_failed(backup_id: u64, reason: str) ｜ /internal/event/backup_fail
    restore_done(point_id: u64) ｜ /internal/event/restore_done

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    backup: 5次/秒
    restore: 5次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    backup: aux_system
    restore: aux_system
```

契约要点：

1. 外部程序仅能访问 `/system/api/backup` 标准虚拟对象，禁止直连备份管理对象内部条目。
2. `restore` 接口经 Python 转换代码先校验还原点有效性，无效直接拒绝。
3. 事件仅封装契约声明的标准参数；备份/还原完成与失败事件供管理工具监听。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 8 设计取舍

1. 备份还原为独立用户态服务，崩溃可重启，不影响业务进程。
2. 还原点机制保证可恢复到历史状态。
3. 破坏性还原操作经 ACL 与审计双重管控。
4. 对外只暴露 `/system/api/backup` 标准接口，内部备份集与策略完全隐藏。
