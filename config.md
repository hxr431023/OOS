# config.md 系统配置子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；系统配置为**用户态服务 + 配置存储体系**，运行于 Ring3。
> 2. Ring1/Ring2 闲置；系统配置表读写受 `acl.md` 权限管控；敏感配置修改写入审计日志（`log.md`）。
> 3. **条目不在此文档硬编码**：系统配置对外条目的集合、命名、权限由 `config.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义系统配置的行为语义，契约全文见第 5 节。
> 4. 对应目录树 `/system/config/`：`settings`（设置与控制面板）、`sysconfig`（系统配置表）、`appconf`（程序配置目录）。
> 5. 依赖：`os.md`、`acl.md`、`fs.md`、`process.md`、`ui.md`、`pkg.md`、`log.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 设置与控制面板 | `/system/config/settings`，含设置应用与控制面板两个程序 |
| 设置应用 | 用户图形化设置界面，**更改与查看系统配置表**；向程序提供设置 API |
| 控制面板 | **更改与查看程序配置目录**（程序级配置）；向程序提供控制面板 API |
| 系统配置表 | 系统级配置存储，类 Windows 注册表，树形键值，位于 `sysconfig` |
| 程序配置目录 | 与程序有关的配置存储，类 Linux `/etc`，程序级，位于 `appconf` |

## 1 定位与职责

系统配置子系统（`/system/config`）承担：

1. **设置应用**（`settings_app`）：用户修改系统配置（显示、声音、网络、电源、个性化等），**更改与查看系统配置表**。
2. **控制面板**（`control_panel`）：**更改与查看程序配置目录**（程序配置）。
3. **系统配置表**（`sysconfig`）：系统级配置存储，类 Windows 注册表（树形键值），受 ACL 管控。
4. **程序配置目录**（`appconf`）：程序级配置，类 Linux `/etc`（程序各自配置），与系统配置表分离。
5. **向程序提供配置 API**：设置应用与控制面板均向程序提供 API（`config.api`）；程序可经 API 查看 / 更改系统配置表与程序配置目录。**程序不能自己创建 / 管理配置目录，必须经控制面板提供的 API 更改程序配置。**

## 2 配置存储体系

### 2.1 系统配置表（`sysconfig`，类 Windows 注册表）

1. 系统级配置统一存于 `/system/config/sysconfig`，树形键值结构（类似注册表）。
2. 由**设置应用**更改与查看；系统组件（内核、服务、服务）启动时读取；程序可经**设置 API** 查看 / 更改。
3. 权限：系统配置表写入仅授权授权进程（`kernel` / 管理员），普通进程只读（经设置 API 受限写入）。

### 2.2 程序配置目录（`appconf`，类 Linux /etc）

1. 与程序有关的配置存于 `/system/config/appconf`，按程序分目录。
2. 由**控制面板**更改与查看；程序经**控制面板 API** 查看 / 更改程序配置。
3. **程序不能自己创建 / 管理配置目录**，必须通过控制面板提供的 API 更改程序配置。
4. 程序配置与系统配置表分离：系统配置表管系统，程序配置目录管程序。

## 3 设置与控制面板

### 3.1 设置应用（`settings_app`）

1. 用户经 GUI 查看与修改系统设置（`ui.md` 渲染）。
2. **查看与写入系统配置表**（`sysconfig`），随系统持久生效。
3. 向程序提供**设置 API**（`config.api`）：程序可经此查看 / 更改系统配置表。
4. 修改敏感配置写入审计日志（`log.md`）。

### 3.2 控制面板（`control_panel`）

1. 集中查看系统各项配置与状态。
2. **更改与查看程序配置目录**（`appconf`）——程序级配置。
3. 向程序提供**控制面板 API**（`config.api`）：程序经此查看 / 更改程序配置；**程序不得自行创建配置目录**。
4. 也可查看系统配置表（只读视图），不直接修改系统配置表。

## 4 与各子系统对接

1. **os.md**：配置对象为纯控制对象，复用对象‑条目模型。
2. **acl.md**：系统配置表读写受 ACL 管控。
3. **fs.md**：配置存储遵循 `/system/config` 目录树。
4. **process.md**：设置 / 控制面板应用生命周期。
5. **ui.md**：设置与控制面板界面渲染。
6. **pkg.md**：程序安装时经控制面板 API 向 `appconf` 写入程序配置。
7. **log.md**：敏感配置修改写入审计日志。
8. **.api.md**：系统配置对外接口经 `config.api` 契约与桥接层。9. **locale.md**：本地化（语言/区域/键盘布局）持久化于 sysconfig。
10. **secure_island.md**：sysconfig 敏感项（凭据/密钥）由安全岛（secure_island.md）密封保护。

## 5 系统配置 `.api` 契约文件（config.api 全文）

系统配置对外接口统一由以下契约文件声明，文件存放于 `/system/api/config.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# config.api 系统配置子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = config
env = python
exec = /system/config/settings/settings_app
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 200次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/config cfg

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
cfg: sysconfig { } /internal/config/sys
cfg: appconf { } /internal/config/app

# ---- 可写控制条目：转换代码为Python，实现配置读写 ----
# 设置 API：查看 / 更改系统配置表
cfg: get {
    return self.get(key)
} /internal/config/get
cfg: set {
    return self.set(key, value)
} /internal/config/set
# 控制面板 API：程序经此查看 / 更改程序配置目录（程序不自建配置目录）
cfg: get_appconf {
    return self.get_appconf(app, key)
} /internal/config/app/get
cfg: set_appconf {
    return self.set_appconf(app, key, value)
} /internal/config/app/set
cfg: open_panel {
    return self.open_panel(name)
} /internal/config/panel

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    config_changed(key: str) ｜ /internal/event/config_change

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    set: 20次/秒
    set_appconf: 100次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    set: kernel
    set_appconf: user_foreground
```

契约要点：

1. `sysconfig` / `appconf` 只读透传系统配置表与程序配置目录。
2. **设置 API**：`get` / `set` 查看 / 更改系统配置表；`set` 仅授权 `kernel`（管理员），程序经设置授权受限写入。
3. **控制面板 API**：`get_appconf` / `set_appconf` 查看 / 更改程序配置目录；`set_appconf` 授权前台进程（程序）调用——**程序必须经此更改程序配置，不得自行创建配置目录**。
4. `open_panel` 打开控制面板 / 设置界面。
5. 配置变更事件供组件热更新与审计（`log.md`）监听。

## 6 设计取舍

1. 系统配置表（类注册表）与程序配置目录（类 /etc）分离：一个管系统、一个管程序。
2. **设置管系统配置表（改 + 查），控制面板管程序配置目录（改 + 查）**，职责清晰。
3. 两者均向程序提供 API；**程序不自建配置目录，统一经控制面板 API 更改程序配置**，配置可控可审计。
4. 系统配置表写入经 ACL + 审计，防篡改可追溯。
5. 对外只暴露 `/system/api/config` 标准接口，内部存储结构隐藏。
