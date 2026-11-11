# pkg.md 软件包管理子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；软件包管理为**用户态服务**，运行于 Ring3，经标准接口与系统交互。
> 2. Ring1/Ring2 闲置；软件包安装、卸载受 `acl.md` 权限管控；安装/卸载事件写入审计日志（`log.md`）。
> 3. **条目不在此文档硬编码**：软件包管理对外条目的集合、命名、权限由 `pkg.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义软件包管理的行为语义，契约全文见第 7 节。
> 4. 对应目录树 `/system/pkg/`：`pkg_manager`（软件包管理器主程序）、`sources`（软件源配置）、`install_rules`（安装权限规则）、`cache_template`（包缓存目录模板）。
> 5. 依赖：`os.md`、`acl.md`、`process.md`、`fs.md`、`log.md`、`.api.md`、`update.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 软件包 | 可安装软件单元的封装：程序二进制、依赖描述、元数据、安装脚本 |
| 软件源 | 软件包来源仓库配置，位于 `/system/pkg/sources`，可配置多个源 |
| 安装规则 | 安装权限、签名校验、路径白名单规则，位于 `/system/pkg/install_rules` |
| 包缓存 | 下载/构建产生的缓存目录，遵循 `cache_template` 模板 |
| 依赖解析 | 安装时解析软件包依赖关系并确保依赖就绪 |

## 1 定位与职责

软件包管理器（`/system/pkg/pkg_manager`）承担：

1. 软件包下载、校验、安装、升级、卸载、查询；
2. 软件源管理（增删源、刷新索引）；
3. 依赖解析与冲突检测；
4. 安装/卸载审计留痕（`log.md`）；
5. 与 `update.md` 系统更新联动（包级更新 vs 系统级更新）。

## 2 核心对象模型

软件包管理以**包管理对象** `/object/pkg/mgr`（纯控制对象）为核心载体，内部持有以下条目（**布局由 `pkg.api` 契约声明**）：

1. `list`：只读，已安装软件包列表、版本、来源。
2. `install`：可写，安装请求：包名、版本、来源。
3. `remove`：可写，卸载请求：包名。
4. `upgrade`：可写，升级请求：包名或全部。
5. `source`：可读可写，软件源配置管理。
6. `state`：只读，当前安装任务状态。

## 3 安装流程

1. 调用方向包管理对象 `install` 条目写入安装请求。
2. 内核校验调用方 ACL 权限与 `install_rules` 安装权限规则；未授权直接拒绝。
3. 包管理器校验软件包签名与完整性（对接 `security.md` 加密校验）。
4. 解析依赖：缺失依赖先安装依赖包，冲突则终止并报告。
5. 将软件包安装至目标目录（`/system/pkg` 或用户路径），执行安装脚本。
6. 更新已安装列表，写入审计日志（安装包名、版本、来源、执行者）。

## 4 卸载与升级流程

1. **卸载**：向 `remove` 条目写入包名；校验权限；执行卸载脚本，删除文件，更新列表，写入审计日志。
2. **升级**：向 `upgrade` 条目写入目标；下载新版本、校验、替换文件、保留配置；失败回滚到旧版本。

### 系统自带应用不可卸载

**系统自带应用（截屏与图片 `img.md`、摄像头 `cam.md`、录音与音乐 `rec.md`、便签文档 `note.md`、地图定位 `map.md`、电影与视频 `video.md` 等）禁止卸载。**

原因：

1. 每个系统自带应用**独占相关权限与组件**：如录音与音乐独占麦克风、摄像头独占摄像头、截屏与图片承担系统唯一截屏、便签文档承担唯一文档编辑、地图集成定位服务、电影与视频承担全格式视频解码；
2. 这些权限 / 组件**只能经对应程序本身调用**，卸载一个应用会连带卸载它相关的权限与组件；
3. 一旦卸载，相关功能权限即失效，系统对应能力损坏（卸载即坏）；
4. 因此包管理器对系统自带应用**拒绝卸载**（`remove` 请求返回拒绝，写入审计日志）；系统自带应用的替换只能经系统更新（`update.md`）统一进行。

### 应用卸载连带关联库/组件

1. 每个应用可能关联专属库 / 组件（如录音与音乐的 `codecs` 音乐解码库、电影与视频的 `codecs` 视频解码库、截屏与图片的滤镜库等），存放于系统目录（`/system` 下对应应用目录）。
2. **卸载一个应用时，其关联的库 / 组件一并卸载**，所有应用一致。
3. 卸载是整体性的：关联库不独立残留，也不可独立于宿主应用存在。

## 5 容错与安全

1. 安装失败：回滚已写文件，保持系统原状态，记录错误日志。
2. 依赖解析失败：终止安装并明确提示缺失依赖。
3. 包缓存损坏：重新下载，不直接使用损坏缓存。
4. 安装权限规则限制：普通用户仅可安装白名单路径软件；系统级安装需授权。

## 6 与各子系统对接

1. **os.md**：包管理对象为纯控制对象，复用对象‑条目模型。
2. **acl.md**：安装/卸载/源管理受 ACL 管控。
3. **fs.md**：软件包文件落盘遵循 `/system/pkg` 目录树与 VFS。
4. **process.md**：安装脚本以子进程执行，遵循进程生命周期与回收。
5. **log.md**：安装/卸载/越权操作写入审计日志。
6. **update.md**：系统级更新与软件包升级联动。
7. **.api.md**：软件包管理对外接口经 `pkg.api` 契约与桥接层。

## 7 软件包管理 `.api` 契约文件（pkg.api 全文）

软件包管理对外接口统一由以下契约文件声明，文件存放于 `/system/api/pkg.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# pkg.api 软件包管理子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = pkg
env = python
exec = /system/pkg/pkg_manager
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 100次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/pkg pkg

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
pkg: list { } /internal/pkg/list
pkg: state { } /internal/pkg/state

# ---- 可写控制条目：转换代码为Python，实现包管理操作 ----
pkg: install {
    self.install(name, version, source)
} /internal/pkg/install
pkg: remove {
    self.remove(name)
} /internal/pkg/remove
pkg: upgrade { self.upgrade(name) } /internal/pkg/upgrade
pkg: add_source {
    self.add_source(url, priority)
} /internal/pkg/source/add

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    pkg_installed(name: str, version: str) ｜ /internal/event/pkg_install_done
    pkg_removed(name: str) ｜ /internal/event/pkg_remove_done
    pkg_failed(name: str, reason: str) ｜ /internal/event/pkg_fail

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    install: 10次/秒
    remove: 10次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    install: aux_system
    remove: aux_system
    add_source: aux_system
```

契约要点：

1. 外部程序仅能访问 `/system/api/pkg` 标准虚拟对象，禁止直连包管理对象内部条目。
2. 安装/卸载/源管理操作仅授权 `aux_system` 分类进程调用。
3. 事件仅封装契约声明的标准参数；安装/卸载完成与失败事件供系统管理工具监听。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 8 设计取舍

1. 软件包管理为独立用户态服务，崩溃可重启，不影响内核。
2. 安装/卸载统一走对象条目与 ACL，权限可审计。
3. 依赖解析、回滚机制集中在包管理器，保证安装一致性。
4. 对外只暴露 `/system/api/pkg` 标准接口，内部仓库与缓存结构完全隐藏。
