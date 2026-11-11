# map.md 地图与定位子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；地图子系统由**地图应用**（用户态应用）与**定位服务**（系统后台服务）两部分构成，运行于 Ring3。
> 2. Ring1/Ring2 闲置；定位数据与地图访问受 `acl.md` 权限管控；位置信息属敏感隐私，读取经 `security.md` 授权与审计。
> 3. **条目不在此文档硬编码**：地图与定位对外条目的集合、命名、权限由 `map.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义地图与定位的行为语义，契约全文见第 6 节。
> 4. 对应目录树 `/system/SystemApps/map/`：地图应用主程序、定位服务、地图数据、配置。
> 5. 定位硬件（GPS 等）为硬件 IO 流对象，经 `hardware_svc.md` / `external_dev.md` 接入；网络定位（WiFi/基站）经 `net.md`。
> 6. 依赖：`os.md`、`acl.md`、`fs.md`、`process.md`、`net.md`、`hardware_svc.md`、`external_dev.md`、`security.md`、`log.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 定位服务 | 聚合各位置源、对外提供设备位置的系统后台服务 |
| 地图应用 | 地图查看、搜索、导航的用户态应用（类似 Windows 地图） |
| 位置源 | GPS 硬件 / 网络定位（WiFi、基站）等定位数据来源 |
| 地图数据 | 地图瓦片、POI 等数据（本地 / 在线获取） |

## 1 定位与构成

地图子系统由两部分构成，全部运行于 Ring3：

1. **定位服务**（`location_svc`）：聚合 GPS、网络定位等位置源，对外经定位对象条目提供设备位置；地图应用及其它应用可请求定位。
2. **地图应用**（`map_app`）：地图查看、搜索、导航；调用定位服务在地图上显示当前位置（类似 Windows 地图）。

## 2 定位服务

### 2.1 位置源

1. **GPS 硬件**：GPS 接收器为硬件对象，经 `hardware_svc.md` 服务绑定、`external_dev.md` 外设接入；内核服务将卫星数据写入 GPS 硬件对象条目。
2. **网络定位**：WiFi 信号、基站信号经 `net.md` 上报，定位服务估算位置（精度较低）。
3. 定位服务聚合多源，按精度加权输出最佳位置。

### 2.2 定位流程

1. 应用请求定位（开启定位 / 获取当前位置）。
2. 定位服务向各位置源发起定位：
   - GPS：读取 GPS 硬件对象条目（经 VFS 或原生对象通路），解析卫星定位结果；
   - 网络：经 `net.md` 上报 WiFi/基站信号强度，估算位置。
3. 聚合结果，输出**位置**（经纬度、精度、时间戳、位置源类型）。
4. 位置变化主动推送事件（`location_event`）。

### 2.3 定位对象条目

| 条目 | 权限 | 说明 |
| --- | --- | --- |
| `position` | 只读 | 当前经纬度、精度、时间戳 |
| `status` | 只读 | 定位状态：off/scanning/fixed/error；已启用位置源列表 |
| `provider` | 只读 | 当前采用的位置源（gps/wifi/cell） |
| `ctl` | 可读可写 | 开启/关闭定位、请求一次定位（enable/disable/fix） |
| `event` | 等待可读 | 定位状态变化、位置更新事件 |

## 3 地图应用

1. **地图查看**：加载地图瓦片（本地 `map_data` / 在线经 `net.md`），渲染到窗口（`ui.md`）。
2. **当前位置显示**：调用定位服务对象读取 `position`，在地图上标注当前位置。
3. **搜索与导航**：POI 搜索、路径规划（数据本地 / 在线）。
4. 用户态应用，崩溃可重启（`process.md`），不影响定位服务。

## 4 与各子系统对接

1. **os.md**：定位、地图均为纯控制对象，复用对象‑条目模型。
2. **acl.md**：定位读取、地图访问受 ACL 管控；位置为敏感数据。
3. **fs.md**：地图数据存放遵循 `/system/map` 目录树。
4. **process.md**：地图应用与定位服务生命周期；崩溃回收。
5. **net.md**：在线地图下载、网络定位（WiFi/基站）。
6. **hardware_svc.md / external_dev.md**：GPS 硬件服务与接入。
7. **ui.md**：地图渲染到窗口。
8. **security.md**：位置隐私授权与审计。
9. **log.md**：定位请求、敏感访问写入审计日志。
10. **.api.md**：地图与定位对外接口经 `map.api` 契约与桥接层。

## 5 地图数据与存放

1. 地图数据（瓦片、POI）存放于 `/system/SystemApps/map/map_data`，只读。
2. 在线数据经 `net.md` 下载缓存；离线数据预置本地。
3. 数据校验哈希防篡改（`security.md`）。

## 6 地图与定位 `.api` 契约文件（map.api 全文）

地图与定位对外接口统一由以下契约文件声明，文件存放于 `/system/api/map.api` 与 `/system/api/map.app.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# map.location.api 定位服务对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = map.location
env = python
exec = /system/SystemApps/map/location_svc
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 20次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/map/location map_loc

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
map_loc: latitude { } /internal/loc/lat
map_loc: longitude { } /internal/loc/lon
map_loc: accuracy { } /internal/loc/accuracy
map_loc: timestamp { } /internal/loc/ts
map_loc: provider { } /internal/loc/provider
map_loc: satellite_count { } /internal/loc/satellites

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
map_loc: enable { return self.enable() } /internal/loc/enable
map_loc: disable { return self.disable() } /internal/loc/disable
map_loc: request_fix { return self.request_fix() } /internal/loc/fix

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    location_fixed(lat: f64, lon: f64) ｜ /internal/event/loc_fix
    location_enabled() ｜ /internal/event/loc_on
    location_disabled() ｜ /internal/event/loc_off

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    request_fix: 5次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    enable: user_foreground
```

```text
# map.app.api 地图应用对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = map.app
env = python
exec = /system/SystemApps/map/map_app
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 20次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/map/app map_app

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
map_app: current_view { } /internal/map/view
map_app: zoom_level { } /internal/map/zoom

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
map_app: open_map { return self.open_map(path) } /internal/map/open
map_app: pan { return self.pan(x, y) } /internal/map/pan
map_app: zoom { return self.zoom(level) } /internal/map/zoom
map_app: center_on { return self.center_on(lat, lon) } /internal/map/center

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    map_loaded() ｜ /internal/event/map_loaded
    view_changed() ｜ /internal/event/view_changed

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    zoom: 20次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    open_map: user_foreground
```

契约要点：

1. 定位对象内部路径 `/object/loc`，地图对象 `/object/map`；定位数据经定位服务条目暴露。
2. 定位只读字段透传位置/状态；写字段为开启/关闭/请求定位命令。
3. 定位访问审计留痕（audit=true，位置隐私）；地图浏览不强制审计。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 7 设计取舍

1. 定位服务独立于地图应用：其它应用也可复用定位，地图崩溃不影响定位。
2. 多位置源聚合：GPS 精度高、网络定位覆盖广，按精度加权。
3. 位置隐私敏感：定位读取经 ACL + 审计双重管控。
4. 地图应用为用户态应用，崩溃可重启，不伤系统稳定性。
5. **系统核心应用**：集成定位服务，**不可卸载**（卸载会连带卸载相关权限与组件致系统损坏，见 `pkg.md`）。
6. **卸载连带影响**：本应用拥有**独特的 API、独特的能力、独特的库、独特的程序与能力**；若被卸载，将**连带卸载**其关联的 API 契约、库与组件（不止一个）。其它应用可能**依赖本应用的 API**——刚卸载一个，依赖它的下一个应用随即用不了。因此应用间因 API 依赖相互锁定，系统自带应用**不可卸载**（见 `pkg.md`）。
