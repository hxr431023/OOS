# storage.md 存储管理子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；块设备硬件由内核管理（`fs.md` 块设备对象），存储管理为**用户态服务**，负责卷、RAID、健康监控、磁盘加密、挂载策略。
> 2. Ring1/Ring2 闲置；存储管理操作受 `acl.md` 权限管控；敏感操作写入审计日志（`log.md`）。
> 3. **条目不在此文档硬编码**：存储管理对外条目的集合、命名、权限由 `storage.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义存储管理的行为语义，契约全文见第 8 节。
> 4. 对应目录树 `/system/storagemgr/`：`storage_daemon`（存储管理守护进程）、`raid_ctrl`（RAID 管理工具）、`disk_health`（SMART 磁盘健康检测）、`encrypt_cfg`（磁盘加密参数模板）、`mount_rules`（自动挂载策略）、`config`（存储全局配置）。
> 5. 依赖：`os.md`、`acl.md`、`fs.md`、`log.md`、`security.md`、`boot.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 存储卷 | 逻辑存储单元：物理磁盘、分区或 RAID 阵列 |
| RAID | 磁盘冗余阵列：多盘组合提升性能或冗余 |
| SMART | 磁盘健康自监测技术，用于故障预警 |
| 磁盘加密 | 卷级加密，防止数据被未授权读取 |
| 挂载策略 | 自动挂载规则：设备接入时按规则挂载 |

## 1 定位与职责

存储管理守护进程（`/system/storagemgr/storage_daemon`）承担：

1. 存储卷管理：卷创建、格式化、删除；
2. RAID 阵列管理（`raid_ctrl`）；
3. 磁盘健康监控与故障预警（`disk_health`）；
4. 磁盘加密配置（`encrypt_cfg`）；
5. 自动挂载策略执行（`mount_rules`）；
6. 存储事件审计留痕。

## 2 核心对象模型

存储管理以**存储管理对象** `/object/storage/mgr`（纯控制对象）为核心载体，内部持有以下条目（**布局由 `storage.api` 契约声明**）：

1. `volume_list`：只读，存储卷列表、容量、状态。
2. `health`：只读，磁盘健康状态（SMART）。
3. `format`：可写，卷格式化请求。
4. `raid`：可写，RAID 阵列创建/删除/重建。
5. `mount`：可读可写，挂载/卸载请求与策略。
6. `encrypt`：可写，卷加密配置。
7. `event`：等待可读，存储事件：磁盘接入/移除、健康告警。

## 3 存储卷管理流程

1. **卷枚举**：存储守护进程监听块设备对象（`fs.md` `/object/blk/<blk_id>`），维护卷列表。
2. **格式化**：向 `format` 条目写入格式化请求；校验权限；调用磁盘文件系统模块完成格式化。
3. **挂载**：按 `mount_rules` 自动挂载策略或手动请求挂载；挂载操作经 `fs.md` 挂载控制条目执行。
4. **卸载**：安全卸载，先停用相关进程访问，再卸载卷。

## 4 RAID 管理

1. RAID 阵列由 `raid_ctrl` 管理：创建（级别、成员盘）、删除、重建。
2. RAID 状态（正常/降级/重建中）写入 `health` 与 `event`，异常自动告警。
3. RAID 与磁盘加密可叠加：阵列之上启用卷加密。

## 5 磁盘健康监控

1. `disk_health` 定期读取 SMART 数据，评估磁盘健康状态。
2. 健康劣化/异常：写入 `event` 健康告警事件，联动 `log.md` 审计日志。
3. 故障预警提示用户备份，避免数据丢失。

## 6 磁盘加密

1. 卷加密配置经 `encrypt_cfg` 模板 + `encrypt` 条目下发；密钥由 `security.md` 加密体系管理。
2. 加密卷在挂载时需密钥；密钥缺失则卷不可访问。
3. 加密操作受 ACL 管控，写入审计日志。

## 7 与各子系统对接

1. **os.md**：存储管理对象为纯控制对象，复用对象‑条目模型。
2. **acl.md**：卷格式化、RAID、加密、挂载操作受 ACL 管控。
3. **fs.md**：块设备对象、挂载操作由 fs 子系统提供底层能力。
4. **log.md**：存储事件、健康告警、敏感操作写入审计日志。
5. **security.md**：磁盘加密密钥与加密体系协同。
6. **boot.md**：启动阶段挂载根文件系统；存储守护进程由 init 拉起。
7. **.api.md**：存储管理对外接口经 `storage.api` 契约与桥接层。
8. **backup.md**：备份目标卷管理与存储健康监控协同。

## 8 存储管理 `.api` 契约文件（storage.api 全文）

存储管理对外接口统一由以下契约文件声明，文件存放于 `/system/api/storage.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# storage.api 存储管理子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = storage
env = python
exec = /system/storagemgr/storage_daemon
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 100次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/storage storage

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
storage: volume_list { } /internal/storage/volumes
storage: health { } /internal/storage/health

# ---- 可写控制条目：转换代码为Python，实现存储操作 ----
storage: format {
    self.format(vol_id, fs_type)
} /internal/storage/format
storage: mount {
    self.mount(vol_id, target)
} /internal/storage/mount
storage: unmount { self.unmount(vol_id) } /internal/storage/unmount
storage: raid_create {
    self.raid_create(level, members)
} /internal/storage/raid/create
storage: encrypt {
    self.encrypt_volume(vol_id, key_id)
} /internal/storage/encrypt

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    vol_attached(vol_id: u64) ｜ /internal/event/vol_attach
    vol_detached(vol_id: u64) ｜ /internal/event/vol_detach
    health_warning(vol_id: u64, msg: str) ｜ /internal/event/health_warn
    raid_degraded(vol_id: u64) ｜ /internal/event/raid_degrade

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    format: 5次/秒
    raid_create: 5次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    format: aux_system
    raid_create: aux_system
    encrypt: aux_system
```

契约要点：

1. 外部程序仅能访问 `/system/api/storage` 标准虚拟对象，禁止直连存储管理对象内部条目。
2. 卷格式化、RAID、加密等破坏性操作仅授权 `aux_system` 分类进程调用。
3. 事件仅封装契约声明的标准参数；磁盘接入/移除、健康告警、RAID 降级供管理工具监听。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 9 设计取舍

1. 存储管理为独立用户态服务，崩溃可重启，不影响底层块设备。
2. 卷管理、RAID、加密集中在存储守护进程，底层只保留块设备对象。
3. 破坏性操作经 ACL 与审计双重管控。
4. 对外只暴露 `/system/api/storage` 标准接口，内部卷表与密钥完全隐藏。
