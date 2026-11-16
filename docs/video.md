# video.md 电影与视频子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；电影与视频为**系统自带的视频播放应用**，运行于 Ring3，功能：播放本地视频 + 全格式视频解码。
> 2. Ring1/Ring2 闲置；视频文件读取受 `acl.md` 权限管控；播放内容涉版权/敏感信息，访问经 `security.md` 授权与审计。
> 3. **条目不在此文档硬编码**：电影与视频对外条目的集合、命名、权限由 `video.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义电影与视频的行为语义，契约全文见第 7 节。
> 4. 对应目录树 `/system/SystemApps/video/`：应用主程序、播放器、视频解码库（全格式）、配置。
> 5. 视频帧渲染经 `ui.md` 窗口；音频输出经 `audio.md`；视频文件读取经 `fs.md`。
> 6. 依赖：`os.md`、`acl.md`、`fs.md`、`process.md`、`ui.md`、`audio.md`、`media.md`、`filemgr.md`、`security.md`、`log.md`、`pkg.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 电影与视频 | 系统自带视频播放应用 |
| 视频播放器 | 播放本地视频文件，支持播放 / 暂停 / 停止 / 跳转 / 全屏 |
| 视频解码库 | 解码常见公开视频格式（MP4 / AVI / MKV / MOV / WEBM / FLV / WMV / MPEG / TS 等），位于 `/system/SystemApps/video/codecs` |
| 全格式解码 | 支持常见公开视频容器与编码的完整解码 |

## 1 定位与功能

电影与视频为**系统自带视频播放应用**（`/system/SystemApps/video/video_app`），承担：

1. **播放视频**：播放本地视频文件（MP4 / AVI / MKV / MOV / WEBM / FLV 等）。
2. **全格式视频解码**：经视频解码库解码任意常见公开视频格式。
3. **播放控制**：播放 / 暂停 / 停止 / 进度跳转 / 音量 / 全屏。
4. **视频解码库**：提供全格式解码，供播放器使用。

## 2 播放流程

1. 用户 / 文件管理器打开视频文件，播放器经 `fs.md` 读取。
2. 经**视频解码库**解码为视频帧流 + 音频流：
   - 视频帧渲染到窗口（`ui.md`）；
   - 音频流经 `audio.md` 输出。
3. 播放控制：播放 / 暂停 / 停止 / 进度跳转 / 音量调节 / 全屏切换。
4. 播放结束 / 退出：释放解码器与句柄，窗口关闭。

## 3 视频解码库

1. 视频解码库存放于**系统目录** `/system/SystemApps/video/codecs`（本应用目录下）。
2. 解码库支持**所有常见公开视频格式**：MP4 / AVI / MKV / MOV / WEBM / FLV / WMV / MPEG / TS 等容器与对应编码。
3. 解码库为电影与视频的**关联组件**：本应用**卸载时连同视频解码库一起卸载**（所有应用一致：应用卸载连同其关联库/组件一起卸载，见 `pkg.md`）。
4. 系统共享媒体解码库 `media/decoders` 仅作补充；视频应用全格式解码以自身解码库为准。
5. 解码库只读使用，校验哈希防篡改（`security.md`）。

## 4 与各子系统对接

1. **os.md**：电影与视频对象为纯控制对象，复用对象‑条目模型。
2. **acl.md**：视频文件读取受 ACL 管控。
3. **fs.md**：视频文件读取遵循文件系统。
4. **process.md**：应用与播放进程生命周期；崩溃回收。
5. **ui.md**：视频帧渲染到窗口；全屏经合成器。
6. **audio.md**：视频音轨输出经音频子系统。
7. **media.md**：系统共享音视频解码库补充。
8. **filemgr.md**：文件管理器可调用视频应用打开视频。
9. **security.md**：播放内容版权 / 敏感授权与审计。
10. **log.md**：播放行为审计留痕。
11. **pkg.md**：系统核心应用不可卸载；卸载连带解码库。
12. **.api.md**：电影与视频对外接口经 `video.api` 契约与桥接层。

## 5 容错

1. 视频应用崩溃：init 重启，不影响已存在视频文件。
2. 打开损坏 / 不支持格式视频：报错提示，不崩溃。
3. 解码失败：降级尝试其它解码器，仍失败则提示。
4. 播放中断 / 内存不足：终止当前播放，释放资源，不影响系统。

## 6 电影与视频 `.api` 契约文件（video.api 全文）

电影与视频对外接口统一由以下契约文件声明，文件存放于 `/system/api/video.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# video.api 电影与视频子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = video
env = python
exec = /system/SystemApps/video/video_app
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 30次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/video video

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
video: current_file { } /internal/video/file
video: play_state { } /internal/video/play_state
video: duration { } /internal/video/duration
video: position { } /internal/video/position
video: fullscreen { } /internal/video/fullscreen

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
video: open { return self.open(path) } /internal/video/open
video: play { return self.play() } /internal/video/play
video: pause { return self.pause() } /internal/video/pause
video: stop { return self.stop() } /internal/video/stop
video: seek { return self.seek(pos) } /internal/video/seek
video: set_volume { return self.set_volume(vol) } /internal/video/volume
video: set_fullscreen { return self.set_fullscreen(on) } /internal/video/fullscreen

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    media_opened(path: str) ｜ /internal/event/video_open
    play_started() ｜ /internal/event/play_start
    play_paused() ｜ /internal/event/play_pause
    play_finished(path: str) ｜ /internal/event/play_end
    play_failed(reason: str) ｜ /internal/event/play_fail

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    seek: 30次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    play: user_foreground
```

契约要点：

1. 视频应用对象内部路径 `/object/video`。
2. 写字段为打开 / 播放 / 暂停 / 停止 / 跳转 / 音量 / 全屏命令。
3. 播放行为审计留痕（audit=true，内容版权敏感）。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 7 设计取舍

1. 视频全格式解码统一收口到系统自带视频应用，用户无需安装第三方解码器。
2. 视频解码库位于 `/system/SystemApps/video/codecs`，随应用卸载一并卸载（应用卸载连同关联库/组件，所有应用一致，见 `pkg.md`）。
3. 视频帧渲染复用窗口合成器，不直写帧缓冲。
4. **系统核心应用**：自带全格式视频解码，**不可卸载**（卸载会连带卸载相关权限与组件致系统损坏，见 `pkg.md`）。
5. **卸载连带影响**：本应用拥有**独特的 API、独特的能力、独特的库、独特的程序与能力**；若被卸载，将**连带卸载**其关联的 API 契约、库与组件（不止一个）。其它应用可能**依赖本应用的 API**——刚卸载一个，依赖它的下一个应用随即用不了。因此应用间因 API 依赖相互锁定，系统自带应用**不可卸载**（见 `pkg.md`）。
