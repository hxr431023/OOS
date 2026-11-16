# audio.md 声音标准（音频子系统规范，x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；音频设备统一抽象为 **IO 流对象**，可挂载 VFS，同时支持原生对象通路访问。
> 2. Ring1/Ring2 闲置；音频设备对象受 `acl.md` 两级 ACL 管控；音频流数据缓冲遵循 `memory.md`。
> 3. **条目不在此文档硬编码**：音频子系统对外条目的集合、命名、权限由 `audio.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义声音子系统的行为语义，契约全文见第 8 节。
> 4. 分层：声卡硬件服务运行于 Ring0 可加载模块；混音、路由、音量控制等复杂逻辑运行于 Ring3 用户态音频服务进程。
> 5. 依赖：`os.md`、`acl.md`、`memory.md`、`ipc.md`、`hardware_svc.md`、`ui.md`、`log.md`、`.api.md`、`boot.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 音频设备对象 | 每个音频硬件（声卡输出/输入通道）对应的 IO 流对象 `/object/audio/<dev_id>` |
| 音频流 | 连续的 PCM 采样数据流；播放流（输出）、录音流（输入） |
| 混音 | 多路音频流按权重混合为一路输出的过程，由用户态音频服务完成 |
| 采样率 / 位深 | 音频采样参数：如 48000 Hz / 16 bit；决定音频质量与数据量 |
| 音频服务 | Ring3 用户态进程，负责混音、路由、音量、格式转换 |
| 环形缓冲 | 音频设备与音频服务之间的数据缓冲，降低延迟与卡顿 |

## 1 音频架构分层

1. **内核层（Ring0）**：声卡硬件服务（可加载模块），负责硬件寄存器、DMA 传输、中断；初始化音频设备对象。
2. **用户态服务层（Ring3）**：音频服务进程，负责混音、音量控制、采样率转换、音频路由、应用接入管理。
3. **应用层**：GUI 应用、媒体播放器通过音频服务接入，提交/接收音频流。

## 2 音频设备对象

音频设备对象 `/object/audio/<dev_id>`（IO 流对象）内部持有以下条目（**布局由 `audio.api` 契约声明**）：

1. `info`：只读，设备名称、支持的采样率/位深/声道、链路状态。
2. `play`：可读可写，播放数据流入口；应用/音频服务写入 PCM 数据。
3. `record`：等待可读，录音数据流出口；读取即获得捕获的 PCM 数据。
4. `mixer`：可读可写，混音与音量控制：主音量、各声道增益、静音。
5. `format`：可读可写，采样率、位深、声道数配置。
6. `event`：等待可读，设备事件：音频设备接入/移除、硬件错误。

## 3 音频数据通路

1. **播放通路**：音频服务（或经混音后的单路流）将 PCM 数据写入 `play` 条目；内核服务经 DMA 搬运至声卡硬件环形缓冲，中断触发输出。
2. **录音通路**：声卡硬件 DMA 采集 PCM 数据写入环形缓冲，内核触发事件唤醒读取方；读取方从 `record` 条目等待读取出数据。
3. **环形缓冲**：内核为每个音频设备维护环形缓冲，降低传输延迟；缓冲区受 `memory.md` 配额管控。
4. 数据载荷走 DMA 裸缓冲区，控制命令（音量、格式）走条目（`os.md` 性能取舍）。

## 4 混音与音量控制

1. **混音**：由用户态音频服务完成；多路应用音频流按权重混合为一路输出。
2. 音频服务对每路输入流维护独立增益，支持单应用静音、独立音量。
3. 主音量/各声道增益经 `mixer` 条目下发至内核与硬件。
4. 采样率/位深不同的流由音频服务统一转换后混音，保证输出格式一致。

## 5 应用接入流程

1. 应用打开音频服务接口，创建一路音频流（播放或录音）。
2. 播放：应用持续向音频服务提交 PCM 缓冲；音频服务混音后写入目标设备 `play` 条目。
3. 录音：应用从音频服务读取混音或单路录音流；音频服务从设备 `record` 条目取数。
4. 应用退出/崩溃：音频服务自动释放其音频流资源；引用计数归零后回收缓冲。

## 6 崩溃与资源回收

1. 声卡服务崩溃：Ring0 模块 bug 触发内核 panic（与其他内核模块一致）；音频服务可由 init 重启。
2. 音频服务进程崩溃：init 重启音频服务，重新接管设备对象与混音状态。
3. 应用进程崩溃：音频服务自动释放该应用音频流，不影响其他应用播放。
4. 音频设备移除：内核向 `event` 写入移除事件，音频服务释放对应设备句柄。

## 7 与各子系统对接

1. **os.md**：音频设备为 IO 流对象，支持原生对象通路与 VFS 两套访问。
2. **acl.md**：音频设备与条目受两级 ACL；音量/格式配置需授权。
3. **memory.md**：环形缓冲、音频数据缓冲遵循内存配额与映射约束。
4. **ipc.md**：`record` 等待读、`event` 事件复用事件/等待读机制。
5. **hardware_svc.md**：声卡服务为内核模块或用户态服务，遵循服务绑定规范。
6. **ui.md**：GUI 应用经音频服务播放声音；系统提示音、通知音由 UI 层触发。
7. **log.md**：音频设备接入/移除、硬件错误写入审计日志。
8. **.api.md**：音频子系统对外接口经 `audio.api` 契约与桥接层。
9. **boot.md**：基座阶段加载声卡服务，创建音频设备对象；init 拉起音频服务。
10. **media.md**：系统音效资源供音频子系统播放。
11. **rec.md**：录音机（rec.md）经音频子系统采集麦克风音频，为系统唯一麦克风使用方。

## 8 音频子系统 `.api` 契约文件（audio.api 全文）

音频子系统对外接口统一由以下契约文件声明，文件存放于 `/system/api/audio.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# audio.api 音频子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = audio
env = python
exec = /system/userland/services/daemons
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 1000次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/audio audio

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
audio: info { } /internal/audio/info
audio: record { } /internal/audio/record

# ---- 可写控制条目：转换代码为Python，实现音频控制 ----
audio: play {
    self.play(dev_id, pcm_data)
} /internal/audio/play
audio: set_volume {
    if not 0.0 <= volume <= 1.0:
        raise ValueError("音量越界")
    self.set_volume(dev_id, volume)
} /internal/audio/ctl/volume
audio: set_format {
    self.set_format(dev_id, rate, depth, channels)
} /internal/audio/ctl/format
audio: set_mute { self.set_mute(dev_id, muted) } /internal/audio/ctl/mute

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    audio_dev_attached(dev_id: u64) ｜ /internal/event/audio_attach
    audio_dev_detached(dev_id: u64) ｜ /internal/event/audio_detach
    audio_error(dev_id: u64, code: i32) ｜ /internal/event/audio_error

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    play: 5000次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    set_format: aux_system
    set_volume: aux_system
```

契约要点：

1. 外部程序仅能访问 `/system/api/audio` 标准虚拟对象，禁止直连音频设备对象内部条目。
2. `set_volume` 接口经 Python 转换代码校验音量区间（0.0~1.0），越界拒绝。
3. 事件仅封装契约声明的标准参数；设备接入/移除/错误事件供音频服务监听。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 9 设计取舍

1. 声卡硬件服务放内核保证时序稳定；混音、路由等复杂逻辑放用户态隔离故障。
2. 音频流复用 IO 流对象模型，应用经标准接口接入，无独立音频系统调用。
3. 环形缓冲 + DMA 裸数据通路，兼顾低延迟与吞吐。
4. 混音集中到单一音频服务，保证多应用声音协调一致。
