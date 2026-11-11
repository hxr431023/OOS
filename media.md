# media.md 媒体资源子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；媒体资源为**只读资源库**，无独立可执行程序，运行于 Ring3 应用层。
> 2. Ring1/Ring2 闲置；资源读取受 `acl.md` 权限管控。
> 3. **条目不在此文档硬编码**：媒体资源对外条目的集合、命名、权限由 `media.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义媒体资源的行为语义，契约全文见第 7 节。
> 4. 对应目录树 `/system/media/`：字体、系统音效、光标、音视频解码库；无可执行程序。
> 5. 依赖：`os.md`、`acl.md`、`fs.md`、`ui.md`、`audio.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 媒体资源 | 字体、系统音效、光标、音视频解码库等只读资源文件 |
| 资源标识 | 按名称/类型唯一标识资源，供系统与应用加载 |
| 解码库 | 音视频解码库，供媒体播放、GUI 使用 |

## 1 定位与构成

`/system/media` 为系统级只读媒体资源库，按子文件夹组织（目录树见 `dirtree.md` / `fs.md`）：

1. **字体**（`fonts/`）：系统字体文件（按字体族组织），供字体引擎（`font.md`）与 GUI 文本渲染使用；`fonts/` 下含三组件——`font_loader`（字体加载器）、`font_resmap`（字体资源映射器，序号↔字体资源，记录位置与格式，映射 `.lerf` 字源及公开开源格式）、`char_mapper`（字符映射器）。
2. **系统音效**（`sounds/`）：提示音、通知音、事件音效，供音频子系统（`audio.md`）播放。
3. **光标**（`cursors/`）：系统光标位图资源，供窗口系统使用。
4. **音视频解码库**（`decoders/`）：音视频解码能力库，供媒体播放应用调用。

## 2 资源加载约定

1. 资源按 `名称.类型` 组织，存放于 `/system/media` 对应子目录。
2. 系统与应用经媒体资源接口按资源标识加载（只读）。
3. 资源文件权限只读，普通进程不可修改，防篡改。
4. 解码库以库形式被应用调用，遵循用户态库加载约定。

## 3 与各子系统对接

1. **os.md**：媒体资源为系统资源库，复用对象‑条目模型。
2. **acl.md**：资源读取受 ACL 管控；资源文件只读。
3. **fs.md**：资源存放遵循 `/system/media` 目录树。
4. **ui.md**：字体、光标、系统图标供 GUI 渲染使用。
5. **audio.md**：系统音效供音频子系统播放。
6. **font.md**：字体资源由字体引擎加载渲染（字体规范专项）。
7. **lerf.md**：语言资源文件的图标/字样经字体引擎加载。
8. **img.md**：图片格式解码库供系统唯一截屏与图片应用（img.md）打开与编辑图片。
9. **video.md**：系统共享音视频解码库补充视频应用（video.md）全格式解码。
10. **cam.md**：图像 / 视频编码库供系统唯一摄像头应用（cam.md）拍照录像。
11. **.api.md**：媒体资源对外接口经 `media.api` 契约与桥接层。

## 4 媒体资源 `.api` 契约文件（media.api 全文）

媒体资源对外接口统一由以下契约文件声明，文件存放于 `/system/api/media.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# media.api 媒体资源子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = media
env = python
exec = /system/media
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 100次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/media media

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
media: fonts { } /internal/media/fonts
media: sounds { } /internal/media/sounds
media: cursors { } /internal/media/cursors

# ---- 可写控制条目：转换代码为Python，实现资源加载 ----
media: load_resource {
    return self.load(name, res_type)
} /internal/media/load

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    media_loaded(name: str, res_type: str) ｜ /internal/event/media_load

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    load_resource: 200次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    load_resource: user_foreground
```

契约要点：

1. 外部程序仅能访问 `/system/api/media` 标准虚拟对象，禁止直连资源目录内部。
2. `load_resource` 经 Python 转换代码校验资源类型与存在性，按标识加载。
3. 事件仅封装契约声明的标准参数；资源加载事件供统计与审计。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 5 设计取舍

1. 媒体资源为只读资源库，无独立服务进程，轻量稳定。
2. 资源读取统一走对象条目与 ACL，防篡改、可审计。
3. 字体、音效、光标、解码库集中管理，避免各应用自带资源冗余。
4. 对外只暴露 `/system/api/media` 标准接口，内部资源布局完全隐藏。
