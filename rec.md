# rec.md 录音与音乐子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；录音与音乐为**系统唯一与麦克风相关的应用**，运行于 Ring3，功能：录音 + 播放音乐 + 音乐编辑 + 语音识别（集成 1.5B 本地模型）+ 音乐解码库。
> 2. Ring1/Ring2 闲置；麦克风访问受 `acl.md` 权限管控；录音内容涉隐私，访问经 `security.md` 授权与审计。
> 3. **条目不在此文档硬编码**：录音与音乐对外条目的集合、命名、权限由 `rec.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义录音与音乐的行为语义，契约全文见第 8 节。
> 4. 对应目录树 `/system/SystemApps/rec/`：应用主程序、音乐播放器、音乐编辑器、语音识别引擎与模型、音乐解码库、配置。
> 5. 麦克风为硬件 IO 流对象，经 `hardware_svc.md` / `external_dev.md` 接入；音频采集/播放复用 `audio.md`。
> 6. 依赖：`os.md`、`acl.md`、`fs.md`、`process.md`、`audio.md`、`hardware_svc.md`、`external_dev.md`、`media.md`、`security.md`、`log.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 录音与音乐 | 系统唯一与麦克风相关的应用：录音、播放音乐、音乐编辑、语音识别 |
| 音乐播放器 | 播放本地音乐文件的组件 |
| 音乐编辑器 | 音乐编辑：裁剪、拼接、音量、混音等 |
| 音乐解码库 | 解码常见音频格式（MP3 / FLAC / WAV / AAC / OGG 等），位于 `/system/SystemApps/rec/codecs` |
| 语音识别（ASR） | 将音频转为文字；使用 1.5B 参数本地模型 |
| 麦克风对象 | 麦克风硬件 IO 流对象，仅供本应用访问 |

## 1 定位与功能

录音与音乐为**系统唯一能使用麦克风的应用**（`/system/SystemApps/rec/rec_app`），承担：

1. **录音**：经麦克风硬件对象采集音频，保存为音频文件。
2. **播放音乐**：播放本地音乐文件（解码复用音乐解码库）。
3. **音乐编辑**：裁剪、拼接、音量调整、混音等编辑，保存 / 导出。
4. **语音识别**：集成 1.5B 本地语音识别模型，把录音 / 音频文件转为文字。
5. **音乐解码库**：提供常见音频格式解码，供播放与编辑使用。

## 2 麦克风独占

1. 麦克风为硬件 IO 流对象（经 `hardware_svc.md` 绑定、`external_dev.md` 接入）。
2. 麦克风对象 ACL 仅授权本应用；**其它应用禁止打开麦克风对象**（唯一麦克风访问方）。
3. 应用退出 / 崩溃，内核回收麦克风句柄，麦克风复位（`audio.md`）。
4. 麦克风启用时有可见状态指示（隐私提示）。

## 3 录音流程

1. 应用打开麦克风对象，经 `audio.md` 配置采样率 / 声道 / 位深。
2. 音频数据经麦克风对象条目流入，写入音频文件（WAV / OGG 等，解码库编码）。
3. 停止录音：关闭采集流，保存文件到 `fs.md`。

## 4 播放与编辑音乐

1. **播放**：音乐播放器（`music_player`）打开音乐文件，经**音乐解码库**解码为 PCM 流，经 `audio.md` 输出播放；支持播放 / 暂停 / 停止 / 进度跳转。
2. **音乐编辑**：音乐编辑器（`music_editor`）支持裁剪、拼接、音量调整、混音等，编辑后经解码库编码保存。
3. 支持常见音频格式：MP3 / FLAC / WAV / AAC / OGG 等。

## 5 音乐解码库

1. 音乐解码库存放于**系统目录** `/system/SystemApps/rec/codecs`（本应用目录下）。
2. 解码库为录音与音乐的**关联组件**：本应用**卸载时连同音乐解码库一起卸载**（所有应用一致：应用卸载连同其关联库/组件一起卸载，见 `pkg.md`）。
3. 解码库只读使用，校验哈希防篡改（`security.md`）。

## 6 语音识别（1.5B 本地模型）

1. 识别引擎（`asr_engine`）加载 1.5B 本地模型（`asr_model`），离线推理，无需联网。
2. 识别来源：刚录制的音频、已有音频 / 音乐文件。
3. 输出文字结果（含时间戳分段）。
4. 模型运行于用户态，占用内存大，遵循 `memory.md` 约束。

## 7 与各子系统对接

1. **os.md**：录音与音乐对象为纯控制对象，复用对象‑条目模型。
2. **acl.md**：麦克风访问受 ACL 管控，仅授权本应用。
3. **fs.md**：录音 / 音乐文件读写遵循文件系统。
4. **process.md**：应用与识别引擎生命周期；崩溃回收。
5. **audio.md**：音频采集 / 播放经音频子系统。
6. **hardware_svc.md / external_dev.md**：麦克风硬件服务与接入。
7. **media.md**：音频格式编解码库复用（非音乐格式场景）。
8. **security.md**：麦克风与录音隐私授权、审计。
9. **log.md**：录音、识别等敏感操作写入审计日志。
10. **.api.md**：录音与音乐对外接口经 `rec.api` 契约与桥接层。

## 8 录音与音乐 `.api` 契约文件（rec.api 全文）

录音与音乐对外接口统一由以下契约文件声明，文件存放于 `/system/api/rec.api`、`/system/api/rec.music.api` 与 `/system/api/rec.asr.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# rec.app.api 录音对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = rec.app
env = python
exec = /system/SystemApps/rec/rec_app
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 20次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/rec/app rec_app

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
rec_app: state { } /internal/rec/state
rec_app: current_file { } /internal/rec/file
rec_app: record_seconds { } /internal/rec/seconds

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
rec_app: record_start { return self.record_start() } /internal/rec/start
rec_app: record_stop { return self.record_stop() } /internal/rec/stop
rec_app: save { return self.save(path) } /internal/rec/save

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    record_started() ｜ /internal/event/rec_start
    record_stopped(path: str) ｜ /internal/event/rec_stop
    record_failed(reason: str) ｜ /internal/event/rec_fail

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    record_start: 5次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    record_start: user_foreground
    save: user_foreground
```

```text
# rec.music.api 音乐播放编辑对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = rec.music
env = python
exec = /system/SystemApps/rec/rec_app
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 30次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/rec/music rec_music

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
rec_music: current_file { } /internal/music/file
rec_music: play_state { } /internal/music/state
rec_music: position { } /internal/music/position

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
rec_music: open { return self.open(path) } /internal/music/open
rec_music: play { return self.play() } /internal/music/play
rec_music: pause { return self.pause() } /internal/music/pause
rec_music: stop { return self.stop() } /internal/music/stop
rec_music: seek { return self.seek(pos) } /internal/music/seek
rec_music: edit_crop { return self.edit_crop(start, end) } /internal/music/crop
rec_music: edit_volume { return self.edit_volume(vol) } /internal/music/volume
rec_music: edit_concat { return self.edit_concat(paths) } /internal/music/concat

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    music_started() ｜ /internal/event/music_start
    music_paused() ｜ /internal/event/music_pause
    music_stopped() ｜ /internal/event/music_stop
    music_edited() ｜ /internal/event/music_edit

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    seek: 30次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    play: user_foreground
```

```text
# rec.asr.api 语音识别对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = rec.asr
env = python
exec = /system/SystemApps/rec/rec_app
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 10次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/rec/asr rec_asr

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
rec_asr: model_state { } /internal/asr/model
rec_asr: last_result { } /internal/asr/result

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
rec_asr: recognize_file { return self.recognize_file(path) } /internal/asr/file
rec_asr: recognize_stream { return self.recognize_stream() } /internal/asr/stream

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    asr_done(text: str) ｜ /internal/event/asr_done
    asr_failed(reason: str) ｜ /internal/event/asr_fail

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    recognize_file: 5次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    recognize_file: user_foreground
```

契约要点：

1. 录音对象内部路径 `/object/rec`，音乐对象 `/object/rec/music`，识别对象 `/object/rec/asr`。
2. 录音写字段为开始 / 停止录音、保存；音乐写字段为播放 / 暂停 / 停止 / 跳转 / 编辑（裁剪 / 音量 / 拼接）；识别写字段为文件 / 流识别命令。
3. 麦克风录音与识别均审计留痕（audit=true，隐私敏感）；音乐播放编辑不强制审计。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 9 设计取舍

1. 麦克风对象 ACL 独占授权本应用，杜绝后台偷录。
2. 1.5B 语音识别模型本地离线推理，无需联网，隐私可控。
3. 音乐解码库位于 `/system/SystemApps/rec/codecs`，随应用卸载一并卸载（应用卸载连同关联库/组件，所有应用一致，见 `pkg.md`）。
4. 麦克风启用可见状态指示 + 审计，录音全程可追踪。
5. **系统核心应用**：独占麦克风权限，**不可卸载**（卸载会连带卸载相关权限与组件致系统损坏，见 `pkg.md`）。
6. **卸载连带影响**：本应用拥有**独特的 API、独特的能力、独特的库、独特的程序与能力**；若被卸载，将**连带卸载**其关联的 API 契约、库与组件（不止一个）。其它应用可能**依赖本应用的 API**——刚卸载一个，依赖它的下一个应用随即用不了。因此应用间因 API 依赖相互锁定，系统自带应用**不可卸载**（见 `pkg.md`）。
