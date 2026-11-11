# tpmeo.md 临时文件对象（.tpmeo）规范

> 
> 前提说明
> 
> 1. **临时文件对象**：`.tpmeo` 为系统临时文件登记对象；临时文件在**开机时**分配**固定对象路径** `/object/tpmeo/<名称>`，对象身份不随内容变化。
> 2. **需要 `.api` 契约**：临时文件的创建、读写、过期与清理经 `tpmeo.api` 契约与桥接层完成。
> 3. 与 `fs.md` 联动：临时文件存放于系统临时目录，生命周期由对象登记管理（可追踪、可清理）。
> 4. Ring1/Ring2 闲置；临时文件访问受 `acl.md` 权限管控。
> 5. 依赖：`os.md`、`objmgr.md`、`process.md`、`fs.md`、`storage.md`、`acl.md`、`log.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 临时文件对象 | 以 `.tpmeo` 登记的临时文件对象，开机分配固定对象路径 |
| 固定对象 | 开机登记、不随内容变化的对象身份 `/object/tpmeo/<名称>` |
| 过期时间（ttl） | 临时文件存活期限，过期自动清理 |
| 引用计数 | 当前引用方数量；归零可清理 |

## 1 总体约束

1. 开机时登记临时文件对象；对象路径 `/object/tpmeo/<名称>` 一经分配不再变化。
2. 临时文件存放于系统临时目录（`fs.md`），对象登记使生命周期可追踪。
3. 创建、读写、过期、清理均经 `tpmeo.api` 契约（跨进程操作，走桥接层）。
4. 清理规则：ttl 过期 / 所有者退出 / 显式删除 / 关机全清。
5. 崩溃残留：开机时清理上次未清除的临时文件对象（系统启动阶段）。

## 2 临时文件对象条目布局

### 临时文件对象，路径 `/object/tpmeo/<名称>`

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `path` | 只读 | 临时文件路径 |
| `size` | 只读 | 当前大小 |
| `refcount` | 只读 | 引用方计数 |
| `owner` | 只读 | 创建者（进程） |
| `ttl` | 可读可写 | 过期时间（自动清理） |
| `data` | 可读可写 | 内容读写 |

## 3 生命周期

1. **创建**：进程请求创建临时文件对象（可指定 ttl），登记后返回对象句柄；
2. **写入 / 读取**：经 `data` 条目读写内容；
3. **过期 / 清理**：ttl 到期、所有者退出或显式删除时清理；
4. **关机**：全部临时文件对象清理；开机时再登记新对象。

## 4 清理规则

1. ttl 到期自动清理，广播 `tpmeo_cleaned`；
2. 所有者（创建进程）退出，其临时文件对象随进程回收（`process.md`）；
3. 显式删除：经 `tpmeo_delete`（仅所有者或 `aux_system`）；
4. 开机清理：上次异常退出残留的临时对象在启动阶段清除并审计（`log.md`）。

## 5 与各子系统对接

1. **os.md**：复用对象‑条目模型。
2. **objmgr.md**：临时文件对象登记、回收。
3. **process.md**：所有者进程退出联动清理。
4. **fs.md**：临时文件存放与读写。
5. **storage.md**：临时目录占用空间管理。
6. **acl.md**：访问权限管控。
7. **log.md**：清理与异常残留审计。
8. **.api.md**：创建 / 读写 / 清理经 `tpmeo.api` 契约与桥接层。

## 6 设计取舍

1. 开机固定对象：临时文件身份可追踪，崩溃残留可清理。
2. ttl + 所有者退出双保险：临时文件不泄漏。
3. 走 `.api` 契约：跨进程读写受 ACL 管控，清理可审计。

## 7 临时文件对象 `.api` 契约文件（tpmeo.api 全文）

临时文件对象对外接口统一由以下契约文件声明，文件存放于 `/system/api/tpmeo.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# tpmeo.api 临时文件对象对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = tpmeo
env = python
exec = /system/tpmeo/tpmeo_mgr
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 200次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/tpmeo tpmeo

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
tpmeo: tpmeo_list { } /internal/tpmeo/list
tpmeo: tpmeo_info { } /internal/tpmeo/info

# ---- 可写控制条目：转换代码为Python，实现创建/读写/删除 ----
tpmeo: tpmeo_create {
    return self.create(name, ttl)
} /internal/tpmeo/create
tpmeo: tpmeo_write {
    return self.write(name, data)
} /internal/tpmeo/write
tpmeo: tpmeo_read {
    return self.read(name)
} /internal/tpmeo/read
tpmeo: tpmeo_delete {
    return self.delete(name)
} /internal/tpmeo/delete

# ---- 对外可监听标准事件 ----
EVENT:
    tpmeo_created(name: str) ｜ /internal/event/tpmeo_create
    tpmeo_cleaned(name: str, reason: str) ｜ /internal/event/tpmeo_clean

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    tpmeo_delete: user_foreground
```

契约要点：

1. 外部程序经 `/system/api/tpmeo` 标准虚拟对象创建与读写临时文件对象，禁止直连临时文件内部。
2. `tpmeo_create` 可指定 ttl；ttl 到期自动清理并广播 `tpmeo_cleaned`。
3. `tpmeo_delete` 仅所有者或 `aux_system` 可调用。
4. 创建 / 清理事件供系统监视与审计监听。
