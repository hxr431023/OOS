# cam.md 摄像头子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；摄像头为**系统唯一使用摄像头硬件的应用**，运行于 Ring3，功能：拍照 + 录像（拍视频）+ 取景预览。
> 2. Ring1/Ring2 闲置；摄像头访问受 `acl.md` 权限管控；拍照 / 录像涉隐私，访问经 `security.md` 授权与审计。
> 3. **条目不在此文档硬编码**：摄像头对外条目的集合、命名、权限由 `cam.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义摄像头的行为语义，契约全文见第 7 节。
> 4. 对应目录树 `/system/SystemApps/cam/`：应用主程序、拍照组件、录像组件、配置。
> 5. 摄像头为硬件 IO 流对象，经 `hardware_svc.md` / `external_dev.md` 接入；照片 / 视频保存经 `fs.md`；视频文件可由视频应用（`video.md`）播放。
> 6. 依赖：`os.md`、`acl.md`、`fs.md`、`process.md`、`hardware_svc.md`、`external_dev.md`、`ui.md`、`media.md`、`video.md`、`img.md`、`security.md`、`log.md`、`pkg.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 摄像头 | 系统唯一使用摄像头硬件的应用：拍照、录像、预览 |
| 拍照 | 采集摄像头一帧图像保存为照片 |
| 录像 | 采集摄像头视频流保存为视频文件（拍视频） |
| 取景预览 | 摄像头画面实时显示到窗口 |
| 摄像头对象 | 摄像头硬件 IO 流对象，仅供本应用访问 |

## 1 定位与功能

摄像头为**系统唯一使用摄像头硬件的应用**（`/system/SystemApps/cam/cam_app`），承担：

1. **拍照**：采集摄像头一帧图像，保存为照片（可交给图片应用 `img.md` 编辑）。
2. **录像（拍视频）**：采集摄像头视频流，保存为视频文件（可由视频应用 `video.md` 播放）。
3. **取景预览**：摄像头画面实时显示到窗口（`ui.md`）。

## 2 摄像头独占

1. 摄像头为硬件 IO 流对象（经 `hardware_svc.md` 绑定、`external_dev.md` 接入）。
2. 摄像头对象 ACL 仅授权本应用；**其它应用禁止打开摄像头对象**（唯一摄像头使用方）。
3. 应用退出 / 崩溃，内核回收摄像头句柄，摄像头复位。
4. 摄像头启用时有可见状态指示（隐私提示）。

## 3 拍照

1. 打开摄像头对象，配置分辨率 / 画面比例。
2. 向摄像头对象条目下发拍照命令，内核采集一帧图像。
3. 照片保存到 `fs.md`（PNG / JPEG）；可调用图片应用（`img.md`）进一步编辑。

## 4 录像（拍视频）

1. 打开摄像头对象，配置分辨率 / 帧率。
2. 启动录像：摄像头视频流持续采集，编码为视频文件（复用 `media.md` 编码 / 摄像头应用编码组件）。
3. 停止录像：结束采集，保存视频文件到 `fs.md`；可由视频应用（`video.md`）播放。

## 5 与各子系统对接

1. **os.md**：摄像头对象为纯控制对象，复用对象‑条目模型。
2. **acl.md**：摄像头访问受 ACL 管控，仅授权本应用。
3. **fs.md**：照片 / 视频文件读写遵循文件系统。
4. **process.md**：应用生命周期；崩溃回收。
5. **hardware_svc.md / external_dev.md**：摄像头硬件服务与接入。
6. **ui.md**：取景预览实时渲染到窗口。
7. **media.md**：图像 / 视频编码解码库复用。
8. **video.md**：录制的视频文件由视频应用播放。
9. **img.md**：拍摄的照片由图片应用编辑。
10. **security.md**：摄像头隐私授权、审计。
11. **log.md**：拍照、录像等敏感操作写入审计日志。
12. **pkg.md**：系统核心应用不可卸载；卸载连带关联组件。
13. **.api.md**：摄像头对外接口经 `cam.api` 契约与桥接层。

## 6 容错

1. 摄像头应用崩溃：init 重启，不影响已保存照片 / 视频。
2. 摄像头不可用 / 无权限：拍照 / 录像请求失败提示，不崩溃。
3. 录像中断：保留已录制部分，提示未完整保存。

## 7 摄像头 `.api` 契约文件（cam.api 全文）

摄像头对外接口统一由以下契约文件声明，文件存放于 `/system/api/cam.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# cam.api 摄像头子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = cam
env = python
exec = /system/SystemApps/cam/cam_app
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 10次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/cam cam

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
cam: status { } /internal/cam/state
cam: mode { } /internal/cam/mode
cam: resolution { } /internal/cam/resolution
cam: last_capture { } /internal/cam/last

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
cam: open_camera { return self.open() } /internal/cam/open
cam: capture_photo { return self.capture() } /internal/cam/photo
cam: start_record { return self.start_record() } /internal/cam/rec_start
cam: stop_record { return self.stop_record() } /internal/cam/rec_stop
cam: close_camera { return self.close() } /internal/cam/close

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    camera_opened() ｜ /internal/event/cam_open
    photo_captured(path: str) ｜ /internal/event/photo_done
    record_started() ｜ /internal/event/rec_start
    record_stopped(path: str) ｜ /internal/event/rec_stop

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    capture_photo: 5次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    open_camera: user_foreground
    capture_photo: user_foreground
```

契约要点：

1. 摄像头对象内部路径 `/object/cam`。
2. 写字段为打开 / 拍照 / 开始录像 / 停止录像 / 关闭命令。
3. 拍照与录像均审计留痕（audit=true，隐私敏感）。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 8 设计取舍

1. 摄像头独占收口到本应用，杜绝后台偷拍偷录。
2. 拍照 + 录像一体的摄像头应用，职责清晰，与图片 / 视频应用协作分工。
3. 取景预览经窗口渲染，不直写帧缓冲。
4. 摄像头启用可见状态指示 + 审计，全程可追踪。
5. **系统核心应用**：独占摄像头硬件，**不可卸载**（卸载会连带卸载相关权限与组件致系统损坏，见 `pkg.md`）。
6. **卸载连带影响**：本应用拥有**独特的 API、独特的能力、独特的库、独特的程序与能力**；若被卸载，将**连带卸载**其关联的 API 契约、库与组件（不止一个）。其它应用可能**依赖本应用的 API**——刚卸载一个，依赖它的下一个应用随即用不了。因此应用间因 API 依赖相互锁定，系统自带应用**不可卸载**（见 `pkg.md`）。
