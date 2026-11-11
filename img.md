# img.md 截屏与图片子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；截屏与图片为**系统唯一自带的图片应用**，运行于 Ring3，功能：截屏、打开图片、编辑图片。
> 2. **摄像头拍照 / 录像已分离为独立摄像头应用（`cam.md`）**，本应用不再使用摄像头硬件。
> 3. Ring1/Ring2 闲置；图片读写受 `acl.md` 权限管控；截屏内容涉敏感信息，访问经 `security.md` 授权与审计。
> 4. **条目不在此文档硬编码**：截屏与图片对外条目的集合、命名、权限由 `img.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义截屏与图片的行为语义，契约全文见第 7 节。
> 5. 对应目录树 `/system/SystemApps/img/`：截屏与图片应用主程序、截图组件、滤镜/处理库、配置。
> 6. 截图经 `ui.md` 合成器统一抓取（普通应用不能直接访问硬件帧缓冲）；图片格式解码/编码复用 `media.md` 解码库。
> 7. 依赖：`os.md`、`acl.md`、`fs.md`、`process.md`、`ui.md`、`media.md`、`filemgr.md`、`security.md`、`log.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 截屏 | 抓取全屏 / 区域 / 指定窗口画面为图片 |
| 截屏与图片 | 系统唯一自带图片应用，集截屏、打开图片、编辑图片于一体 |
| 图片格式 | PNG、JPEG、BMP、GIF、WebP 等公开图片格式 |
| 滤镜 | 图片处理效果（亮度、对比度、模糊、锐化等） |

## 1 定位与功能

截屏与图片为**系统唯一自带的图片应用**（`/system/SystemApps/img/img_app`），承担三类功能：

1. **截屏**：全屏截图、区域截图、指定窗口截图，保存为图片。
2. **打开图片**：打开常见图片格式（PNG / JPEG / BMP / GIF / WebP 等）查看。
3. **编辑图片**：裁剪、缩放、旋转、画笔、文字、滤镜等编辑，保存 / 导出。

## 2 截屏

1. 截屏经 `ui.md` 合成器统一抓取：应用向合成器截图对象请求抓取，合成器从合成缓冲区导出画面（普通应用不能直接访问硬件帧缓冲）。
2. 截图模式：
   - **全屏**：抓取当前合成输出全幅；
   - **区域**：用户框选屏幕区域；
   - **窗口**：抓取指定窗口内容（经窗口对象）。
3. 截图结果进入编辑界面，可立即编辑 / 保存。

## 3 打开与编辑图片

1. **打开图片**：经 `fs.md` 读取图片文件，`media.md` 解码库解码为像素数据，渲染到窗口（`ui.md`）。
2. **编辑**：
   - 基本变换：裁剪、缩放、旋转、翻转；
   - 绘制：画笔、形状、文字；
   - 滤镜：亮度 / 对比度 / 模糊 / 锐化 / 反色等；
   - 历史：撤销 / 重做（undo/redo 栈）。
3. **保存 / 导出**：编码为 PNG / JPEG 等格式写回 `fs.md`。

## 4 与各子系统对接

1. **os.md**：截屏与图片对象为纯控制对象，复用对象‑条目模型。
2. **acl.md**：图片读写、截屏访问受 ACL 管控。
3. **fs.md**：图片文件读写遵循文件系统。
4. **process.md**：图片应用生命周期；崩溃回收。
5. **ui.md**：画面渲染；截屏经合成器抓取。
6. **media.md**：图片格式解码库、系统图标资源复用。
7. **filemgr.md**：文件管理器可调用图片应用打开图片。
8. **security.md**：截屏涉敏感信息，授权与审计。
9. **log.md**：截屏等敏感操作写入审计日志。
10. **.api.md**：截屏与图片对外接口经 `img.api` 契约与桥接层。

## 5 图片格式支持

1. 系统唯一图片应用必须支持常见公开图片格式：PNG、JPEG、BMP、GIF、WebP（解码复用 `media/decoders`）。
2. 保存默认 PNG；可导出 JPEG 等。
3. 格式解码库只读复用 `media.md`，图片应用不重复内置。

## 6 容错

1. 图片应用崩溃：init 重启，不影响已保存图片。
2. 打开损坏图片：报错提示，不崩溃。
3. 编辑过程未保存崩溃：丢失本次编辑，原文件不受影响。

## 7 截屏与图片 `.api` 契约文件（img.api 全文）

截屏与图片对外接口统一由以下契约文件声明，文件存放于 `/system/api/img.api` 与 `/system/api/img.app.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# img.capture.api 截屏对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = img.capture
env = python
exec = /system/SystemApps/img/img_app
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 10次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/img/capture img_capture

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
img_capture: mode { } /internal/capture/mode
img_capture: last_shot { } /internal/capture/last
img_capture: status { } /internal/capture/state

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
img_capture: capture_full { return self.capture_full() } /internal/capture/full
img_capture: capture_region { return self.capture_region(rect) } /internal/capture/region
img_capture: capture_window { return self.capture_window(win_id) } /internal/capture/window

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    shot_taken(path: str) ｜ /internal/event/shot_done
    shot_failed(reason: str) ｜ /internal/event/shot_fail

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    capture_region: 20次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    capture_window: user_foreground
```

```text
# img.app.api 图片应用对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = img.app
env = python
exec = /system/SystemApps/img/img_app
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 30次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/img/app img_app

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
img_app: open_file { } /internal/app/file
img_app: status { } /internal/app/state

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
img_app: open { return self.open(path) } /internal/app/open
img_app: save { return self.save(path) } /internal/app/save
img_app: undo { return self.undo() } /internal/app/undo
img_app: apply_filter { return self.apply_filter(filter) } /internal/app/filter
img_app: crop { return self.crop(rect) } /internal/app/crop
img_app: scale { return self.scale(w, h) } /internal/app/scale

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    image_opened(path: str) ｜ /internal/event/img_opened
    image_saved(path: str) ｜ /internal/event/img_saved

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    save: 20次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    open: user_foreground
```

契约要点：

1. 截图对象内部路径 `/object/img/capture`，图片应用对象 `/object/img`。
2. 截图写字段为全屏 / 区域 / 窗口抓取命令；图片应用写字段为打开 / 保存 / 撤销 / 滤镜 / 裁剪 / 缩放命令。
3. 截图访问审计留痕（audit=true，涉屏敏感）；图片编辑不强制审计。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 8 设计取舍

1. 截屏统一经合成器抓取，禁止普通应用直接访问硬件帧缓冲，保安全。
2. 摄像头拍照 / 录像分离到独立摄像头应用（`cam.md`），职责单一。
3. 图片格式解码复用 `media.md` 解码库，图片应用不重复内置，避免资源冗余。
4. 截屏敏感操作经 ACL + 审计双重管控。
5. **系统核心应用**：承担系统唯一截屏与图片编辑，**不可卸载**（卸载会连带卸载相关权限与组件致系统损坏，见 `pkg.md`）。
6. **卸载连带影响**：本应用拥有**独特的 API、独特的能力、独特的库、独特的程序与能力**；若被卸载，将**连带卸载**其关联的 API 契约、库与组件（不止一个）。其它应用可能**依赖本应用的 API**——刚卸载一个，依赖它的下一个应用随即用不了。因此应用间因 API 依赖相互锁定，系统自带应用**不可卸载**（见 `pkg.md`）。
