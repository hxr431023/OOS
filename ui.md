# ui.md GUI 窗口与图形子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 对象‑条目模型；窗口、显示设备全部抽象为内核对象。
> 2. 遵循混合内核设计：底层显示硬件服务运行在内核 Ring0 模块；窗口合成器、窗口管理器全部运行在 Ring3 用户态进程，GUI 崩溃不会导致整机 panic。
> 3. Ring1/Ring2 闲置；复用 `acl.md`、`memory.md`、`ipc.md`。
> 4. 不新增独立专用系统调用；全部能力依靠操作对象条目完成。

## 术语

- **显示设备对象**：IO 流对象，代表物理显卡输出通道，路径`/object/display/<disp_id>`；可映射 VFS 到`/system/dev/display/*`。
- **窗口对象**：纯控制对象，路径`/object/window/<win_id>`，**不挂载 VFS**，仅原生对象 API 访问。
- **合成器进程**：特权用户态进程，负责帧缓冲合成、扫描输出到显示硬件；拥有显示设备对象的写入权限。
- **窗口管理器**：用户态进程，负责窗口创建、层级管理、拖拽、最大化最小化、焦点切换。
- **共享帧缓冲区**：由内核分配的 DMA 可访问内存，应用与合成器通过页表映射获得虚拟地址，用于渲染像素。

## 1 总体约束

1. 显卡底层硬件服务（模式设置、CRTC、扫描输出、硬件光标）作为**内核可加载模块运行 Ring0**；负责初始化显示设备对象。
2. 所有上层图形逻辑：窗口管理、应用渲染、窗口合成全部在 Ring3 用户态；GUI 应用崩溃只销毁自身窗口，不影响合成器与内核。
3. 每个窗口为独立纯控制对象；窗口的输入事件、渲染提交、属性修改全部读写窗口对象内部条目。
4. 普通 GUI 应用**不能直接写硬件帧缓冲**；必须向合成器提交渲染缓冲区，由合成器统一合成输出到物理屏幕。
5. 输入（鼠标、键盘）事件经由 IPC 事件条目分发给对应焦点窗口。

## 2 显示设备对象（IO 流对象 `/object/display/<disp_id>`）

表格

| 条目名称 | 权限 | 说明 |
| --- | --- | --- |
| `mode_ctl` | 可读可写 | 显示模式控制：分辨率、刷新率、色深设置 |
| `fb_info` | 只读 | 帧缓冲物理 / 虚拟信息、缓冲区大小、像素格式 |
| `vsync_event` | 等待可读 | 垂直同步 VSYNC 事件；合成器等待此条目做帧限流 |
| `hw_cursor` | 可读可写 | 硬件光标位图、光标坐标控制 |

> 
> 内核显卡模块初始化完成后创建该对象；只有合成器进程拥有`mode_ctl`、`hw_cursor`的`ENTRY_WRITE`权限。
> 普通应用只有`ENTRY_READ`，只能读取显示模式信息，不能修改硬件显示参数。

## 3 窗口对象（纯控制对象 `/object/window/<win_id>`）

> 
> 父对象：`/object/window`；创建窗口需要父对象的`CREATE_ENTRY`权限。

表格

| 条目名称 | 权限 | 说明 |
| --- | --- | --- |
| `prop` | 可读可写 | 窗口属性：位置、宽高、z 层级、标题、是否可见、透明度 |
| `submit_buf` | 只写 | 提交渲染帧缓冲区句柄；应用把绘制好的缓冲句柄写入此条目，交给合成器 |
| `input_event` | 等待可读 | 输入事件队列：鼠标移动、点击、键盘按键事件；应用阻塞读取此条目获取输入 |
| `state` | 只读 | 窗口状态：normal/minimized/maximized/hidden；所属 PID |

### 窗口生命周期

1. GUI 应用调用`OpenObject("/object/window")`，使用`ObjectCreateEntry()`创建新窗口子对象，得到窗口句柄。
2. 向`prop`条目写入窗口初始大小、标题。
3. 向内核申请共享帧缓冲内存，映射到本进程虚拟地址，应用在该缓冲区完成像素渲染。
4. 将帧缓冲对应的对象句柄写入`submit_buf`条目，提交给合成器。
5. 合成器收到缓冲区，按照 Z 层级完成画面合成，等待`vsync_event`后输出到显示设备。
6. 鼠标键盘输入由内核输入服务捕获，窗口管理器判定焦点窗口，向该窗口`input_event`条目写入事件。
7. 应用销毁窗口：调用`ObjectDeleteEntry()`删除窗口对象；应用进程崩溃，内核自动销毁该进程全部关联窗口对象。

## 4 角色分工（用户态进程）

1. **窗口管理器进程**
   - 拥有`/object/window`的`CREATE_ENTRY`/`DELETE_ENTRY`；管控窗口层级、拖拽、焦点、窗口装饰。
   - 接收输入原始事件，计算焦点，分发事件到对应窗口对象`input_event`条目。
2. **合成器进程**
   - 持有显示设备对象句柄；等待 VSYNC 事件；收集各个窗口提交的渲染缓冲，执行画面合成，输出到硬件帧缓冲。
   - 回收已经完成显示的旧缓冲区。
3. **普通 GUI 应用进程**
   - 创建自己的窗口对象；分配共享缓冲；绘制画面，提交缓冲；阻塞读取`input_event`处理交互。
   - 无权限直接操作显示硬件对象的控制条目。

## 5 共享渲染缓冲区对象（IO 流对象 `/object/gfxbuf/<buf_id>`）

> 
> 图形缓冲区，可被多个进程映射（应用、合成器），引用计数管理。

1. 应用向图形缓冲父对象创建缓冲对象，内核分配物理内存。
2. 内核将缓冲映射到应用、合成器各自的虚拟地址空间。
3. 应用完成渲染，把 gfxbuf 句柄写入窗口`submit_buf`提交。
4. 缓冲区引用计数：窗口提交后，合成器持有引用；画面扫描输出完成，合成器释放引用；引用计数归零自动释放内存。

> 
> 防止应用销毁之后，合成器仍然在读取已经释放的像素内存。

## 6 输入子系统对接

键盘、鼠标属于硬件 IO 流对象 `/object/input/<inp_id>`，内部`event`事件条目；
内核输入服务把原始输入事件写入硬件对象事件条目；
窗口管理器监听全部输入硬件事件，做坐标转换、命中测试，把事件转发给对应窗口对象的`input_event`条目。

## 7 崩溃自动回收

1. GUI 应用进程崩溃：内核遍历该进程持有的全部窗口对象与 gfxbuf 对象；删除窗口、递减缓冲区引用计数。合成器不会引用已经消亡进程的窗口。
2. 合成器进程崩溃：内核不会崩溃；内核重置显示硬件；init 进程重新拉起合成器进程，重建显示会话。
3. 窗口管理器崩溃：init 重启窗口管理器，窗口对象保留，只丢失焦点与窗口装饰逻辑。

## 8 和其它文档对接

1. **os.md**：窗口、显示设备、图形缓冲区均为内核对象；区分 IO 流对象 / 纯控制对象；两套访问入口。
2. **acl.md**：使用 ACL 控制谁可以创建窗口、修改显示硬件参数；普通应用权限被严格限制。
3. **memory.md**：图形缓冲区物理页分配、进程虚拟地址映射；遵循内存子系统约束。
4. **ipc.md**：`vsync_event`、`input_event`底层复用 IPC 等待‑通知条目。
5. **process.md**：进程消亡自动清理关联窗口、图形缓冲区对象。
6. **boot.md**：内核启动加载显卡、输入内核模块，创建 display、input 对象；init 拉起窗口管理器与合成器用户态进程。
7. **fs.md**：显示设备 IO 流对象挂载到 VFS `/system/dev/display`，兼容简单 Linux 图形程序。
8. **Linux.md**：兼容层可以模拟 fb 设备，供 Linux X11 应用运行。
9. **objmgr.md**：窗口、显示、图形缓冲对象由对象管理器统一创建、登记、回收。
10. **audio.md**：GUI 应用经音频服务播放系统提示音、通知音。
11. **media.md**：字体、光标、系统图标资源供 GUI 渲染使用。
12. **font.md**：GUI 文本渲染经字体引擎栅格化，兼容所有公开字体格式。
13. **lerf.md**：界面文本本地化与弹窗布局经语言资源文件（.lerf）加载。
14. **note.md**：便签（note.md）显示在桌面，置顶常驻；窗口渲染支持文档编辑器。
15. **video.md**：视频应用（video.md）视频帧渲染到窗口，全屏经合成器。
16. **config.md**：设置应用与控制面板（config.md）界面经窗口渲染。17. **input.md**：窗口焦点由合成器维护，输入事件（input.md）按焦点分发。
18. **clipboard.md**：剪贴板所有者绑定前台窗口；粘贴经输入快捷键。
19. **session.md**：锁屏图层与会话状态（session.md）联动。
20. **uio.md**：控件库与主题资源经图形界面专用动态链接库对象（uio.md）加载，进程内直接链接。

## 9 窗口 `.api` 契约文件（ui.window.api 全文）

窗口对外接口统一由以下契约文件声明，文件存放于 `/system/api/ui.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# ui.api 窗口子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = ui
env = python
exec = /system/desktop/compositor
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 120次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/ui ui

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
ui: title { } /internal/ui/title
ui: x { } /internal/ui/x
ui: y { } /internal/ui/y
ui: width { } /internal/ui/width
ui: height { } /internal/ui/height
ui: state { } /internal/ui/state
ui: z_order { } /internal/ui/z_order
ui: round_radius { } /internal/ui/round

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
ui: submit_buffer { return self.submit_buffer(buf) } /internal/ui/submit
ui: resize { return self.resize(w, h) } /internal/ui/resize
ui: move { return self.move(x, y) } /internal/ui/move
ui: minimize { return self.minimize() } /internal/ui/minimize
ui: maximize { return self.maximize() } /internal/ui/maximize
ui: close { return self.close() } /internal/ui/close

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    window_resized(win_id: u64) ｜ /internal/event/ui_resized
    window_state_changed(win_id: u64) ｜ /internal/event/ui_state
    window_closed(win_id: u64) ｜ /internal/event/ui_closed

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    submit_buffer: 120次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    submit_buffer: user_foreground
```

契约要点：

1. 契约对象内部路径 `/object/ui/window`，与本文窗口对象模型一致。
2. 只读字段透传 `props`/`rect` 下窗口状态与几何；写字段为 `ctl` 下控制命令（submit_buffer / resize / move / minimize / maximize / close）。
3. 全局限流 120 次/秒；窗口控制不强制审计（audit=false）。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 10 设计取舍

1. 显卡硬件模式设置放在内核保证时序稳定；复杂窗口合成全部放用户态，隔离故障。
2. 不设计特殊图形 syscall；全部使用对象条目；事件通知复用 IPC 子系统。
3. 渲染缓冲独立对象，引用计数安全管理，防止 use‑after‑free 画面错误。
4. 普通应用禁止直接访问硬件帧缓冲；由合成器统一合成，实现窗口叠加、透明度

## 11 悬浮窗口、通知与图层表

### 11.1 悬浮窗口（floating）

1. 悬浮球、悬浮窗等**常驻桌面顶层**的轻量窗口，存放于 `/system/desktop/floating`。
2. 悬浮窗口始终置顶（最高图层，但位于全屏层之下），不抢占焦点，可拖动、可收起。
3. 典型实现：悬浮球（快捷操作入口）。

### 11.2 通知（notifications）

1. 与 Windows 一致的通知：应用发通知，右下角弹出气泡；点击可展开操作。
2. 通知中心收纳历史通知，存放于 `/system/desktop/notifications`。
3. 通知对象为普通窗口级，图层位于普通窗口之上、悬浮层之下。

### 11.3 图层表（layer_table）

1. 一张**图层表**，记录**哪个对象、哪个空间在哪个图层**，存放于 `/system/desktop/layer_table`。
2. 图层由低到高：桌面壁纸 → 普通窗口 → 消息弹窗 → 通知 → 悬浮窗口 → 全屏层 → 加载框（最高）。
3. 合成器按图层表执行合成；图层表由合成器维护，可读查询（`ui.layer`）。

## 12 悬浮 / 通知 / 图层 `.api` 契约文件（ui.float / ui.notify / ui.layer 全文）

```text
# ui.float.api 悬浮窗口对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = ui.float
env = python
exec = /system/desktop/compositor
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 60次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/ui/float ui_float

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
ui_float: title { } /internal/float/title
ui_float: x { } /internal/float/x
ui_float: y { } /internal/float/y
ui_float: width { } /internal/float/width
ui_float: height { } /internal/float/height
ui_float: always_on_top { } /internal/float/top

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
ui_float: open_float { return self.open() } /internal/float/open
ui_float: close_float { return self.close() } /internal/float/close
ui_float: move { return self.move(x, y) } /internal/float/move

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    float_opened() ｜ /internal/event/float_open
    float_closed() ｜ /internal/event/float_close

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    move: 60次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    open_float: user_foreground
```

```text
# ui.notify.api 通知对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = ui.notify
env = python
exec = /system/desktop/compositor
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 100次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/ui/notify ui_notify

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
ui_notify: history { } /internal/notify/history
ui_notify: unread { } /internal/notify/unread

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
ui_notify: notify { return self.push(title, text) } /internal/notify/push
ui_notify: dismiss { return self.dismiss(note_id) } /internal/notify/dismiss
ui_notify: open_center { return self.open_center() } /internal/notify/center

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    notification_pushed(note_id: str) ｜ /internal/event/notify_pushed
    notification_dismissed(note_id: str) ｜ /internal/event/notify_dismissed

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    notify: 30次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    notify: user_foreground
```

```text
# ui.layer.api 图层表对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = ui.layer
env = python
exec = /system/desktop/compositor
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 30次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/ui/layer ui_layer

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
ui_layer: layer_table { } /internal/layer/table
ui_layer: layer_count { } /internal/layer/count

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
ui_layer: assign_layer { return self.assign(obj, space, layer) } /internal/layer/assign
ui_layer: rebuild { return self.rebuild() } /internal/layer/rebuild

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    layer_changed(layer: u32) ｜ /internal/event/layer_changed

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    assign_layer: 30次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    assign_layer: aux_system
```

契约要点：

1. 悬浮窗口对象 `/object/ui/float`：写字段 open / close / move；始终置顶（`always_on_top`）。
2. 通知对象 `/object/ui/notify`：写字段 push / dismiss / open_center；历史与未读只读透传。
3. 图层表对象 `/object/ui/layer`：`layer_table` 记录每个对象 / 空间所在图层；`assign_layer` 分配图层（仅合成器授权，audit=true）。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 13 加载框（splash）

1. **加载框**为安装程序 / 应用启动加载时的临时加载窗口，存放于 `/system/desktop/splash`。
2. **触发**：安装程序（`pkg.md`）或应用启动加载时显示；加载完成自动关闭。
3. **形态**：屏幕**正中央**的**长方形窗口**，位于**最高图层**（覆盖全部窗口，含全屏层）。
4. **内容**：**图标、程序名称、加载信息**（进度 / 状态文字）。
5. **模态**：加载框显示期间为模态，用户不可操作被加载程序；可手动点击关闭（取消加载）。
6. **超时 / 失败**：加载超时或失败时，加载框显示失败信息并允许关闭，不阻塞系统。

## 14 加载框 `.api` 契约文件（ui.splash 全文）

```text
# ui.splash.api 加载框对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = ui.splash
env = python
exec = /system/desktop/compositor
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 30次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/ui/splash ui_splash

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
ui_splash: title { } /internal/splash/title
ui_splash: icon { } /internal/splash/icon
ui_splash: message { } /internal/splash/message
ui_splash: progress { } /internal/splash/progress
ui_splash: visible { } /internal/splash/visible

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
ui_splash: show { return self.show(app, title) } /internal/splash/show
ui_splash: update { return self.update(message, progress) } /internal/splash/update
ui_splash: dismiss { return self.dismiss() } /internal/splash/dismiss

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    splash_shown(app: str) ｜ /internal/event/splash_shown
    splash_hidden(app: str) ｜ /internal/event/splash_hidden
    splash_done(app: str) ｜ /internal/event/splash_done

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    update: 30次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    show: aux_system
```

契约要点：

1. 加载框对象 `/object/ui/splash`：正中央、最高图层、长方形窗口。
2. 写字段 show / update / dismiss：显示、更新加载信息与进度、关闭。
3. `show` / `dismiss` 仅授权 `aux_system`（安装 / 启动引导调用）。
4. 事件供程序自动化感知加载显示与完成（`splash_shown` / `splash_done`）。

## 15 设置框（settings panel）

1. **设置框**为设置 / 开机引导时的交互面板，存放于 `/system/desktop/settings_panel`。
2. **位置**：占据屏幕**左侧**区域（竖长条面板）。
3. **结构**：自上而下分为**标题**、**解释**、**选择按钮**三段。
4. **用途**：开机设置向导、应用 / 系统设置引导、安装选项选择等（配合 `config.md` 设置应用）。
5. **交互**：设置框为模态交互，用户选择完成后关闭，结果反馈给调用方。

## 16 设置框 `.api` 契约文件（ui.panel 全文）

```text
# ui.panel.api 设置框对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = ui.panel
env = python
exec = /system/desktop/compositor
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 30次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/ui/panel ui_panel

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
ui_panel: title { } /internal/panel/title
ui_panel: description { } /internal/panel/description
ui_panel: choices { } /internal/panel/choices
ui_panel: selected { } /internal/panel/selected
ui_panel: visible { } /internal/panel/visible

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
ui_panel: show { return self.show(app, title) } /internal/panel/show
ui_panel: set_title { return self.set_title(title) } /internal/panel/set_title
ui_panel: set_description { return self.set_description(text) } /internal/panel/set_description
ui_panel: set_choices { return self.set_choices(list) } /internal/panel/set_choices
ui_panel: select { return self.select(choice) } /internal/panel/select
ui_panel: dismiss { return self.dismiss() } /internal/panel/dismiss

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    panel_shown(app: str) ｜ /internal/event/panel_shown
    panel_closed(app: str) ｜ /internal/event/panel_closed
    choice_selected(app: str, choice: str) ｜ /internal/event/panel_choice

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    select: 30次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    show: aux_system
```

契约要点：

1. 设置框对象 `/object/ui/panel`：左侧面板，三段式（标题 / 解释 / 选择按钮）。
2. 写字段 show / set_title / set_description / set_choices / select / dismiss：显示、填充三段内容、选择、关闭。
3. `show` 仅授权 `aux_system`（开机设置向导 / 系统设置引导调用）。
4. 事件供程序自动化感知面板显示、关闭与选择结果（`panel_shown` / `choice_selected`）。
## 17 消息弹窗（msgbox）

1. **消息弹窗**为类 Windows 7 的模态消息框，存放于 `/system/desktop/msgbox`。
2. **形态**：屏幕居中窗口，标题栏 + 图标 + 消息文本 + 按钮行。
3. **图标**：`info`（信息）/ `warning`（警告）/ `error`（错误）/ `question`（疑问）。
4. **按钮组合**：`ok` / `ok_cancel` / `yes_no` / `yes_no_cancel` / `retry_cancel` / `abort_retry_ignore`；默认按钮高亮。
5. **键盘**：Enter 触发默认按钮，Esc 触发取消按钮。
6. **模态**：显示期间阻塞调用方，直到用户响应；返回所选按钮（OK / CANCEL / YES / NO / RETRY / IGNORE / ABORT）。
7. **图层**：位于普通窗口之上、通知之下（模态覆盖父窗口）。

## 18 消息弹窗 `.api` 契约文件（ui.msgbox 全文）

```text
# ui.msgbox.api 消息弹窗对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = ui.msgbox
env = python
exec = /system/desktop/compositor
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 60次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/ui/msgbox ui_msgbox

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
ui_msgbox: title { } /internal/msgbox/title
ui_msgbox: message { } /internal/msgbox/message
ui_msgbox: icon { } /internal/msgbox/icon
ui_msgbox: buttons { } /internal/msgbox/buttons
ui_msgbox: result { } /internal/msgbox/result
ui_msgbox: visible { } /internal/msgbox/visible

# ---- 可写控制条目：转换代码为Python，实现弹窗控制 ----
ui_msgbox: show { return self.show(title, message, icon, buttons, default_btn) } /internal/msgbox/show
ui_msgbox: dismiss { return self.dismiss() } /internal/msgbox/dismiss

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    msgbox_shown(app: str) ｜ /internal/event/msgbox_shown
    msgbox_closed(app: str) ｜ /internal/event/msgbox_closed
    msgbox_responded(app: str, btn: str) ｜ /internal/event/msgbox_respond

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    show: 30次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    show: user_foreground
```

契约要点：

1. 消息弹窗对象 `/object/ui/msgbox`：居中模态，标题 / 图标 / 消息 / 按钮组合。
2. 写字段 show / dismiss：弹窗与关闭；`show` 模态阻塞，返回用户所选按钮（`result`）。
3. `show` 授权 `user_foreground`：普通应用均可弹消息框（区别于 splash / panel 的 aux_system）。
4. 事件供程序自动化感知弹窗显示、关闭与用户响应（`msgbox_shown` / `msgbox_responded`）。
## 19 开始菜单（start menu）

1. **开始菜单**为类 Windows 10 的桌面启动入口，存放于 `/system/desktop/start_menu`。
2. **位置**：点击任务栏开始按钮后，屏幕**左下角**弹出面板。
3. **结构**：应用列表（字母分组）、固定项（磁贴 / 固定应用）、最近使用、**搜索框**、电源按钮（关机 / 重启 / 注销 / 锁定）。
4. **交互**：点击外部区域或按 Esc 关闭；搜索实时过滤应用列表；启动应用后菜单关闭。
5. **联动**：应用列表来自已登记可执行对象（`exeo.md`）；启动经可执行对象契约（exeo.api）。
6. **电源按钮**：关机 / 重启 / 注销为敏感操作，仅 `aux_system` 可触发（配合 `power.md` / `session.md`）。

## 20 开始菜单 `.api` 契约文件（ui.startmenu 全文）

```text
# ui.startmenu.api 开始菜单对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = ui.startmenu
env = python
exec = /system/desktop/compositor
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 60次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/ui/startmenu ui_startmenu

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
ui_startmenu: apps { } /internal/startmenu/apps
ui_startmenu: pinned { } /internal/startmenu/pinned
ui_startmenu: recent { } /internal/startmenu/recent
ui_startmenu: visible { } /internal/startmenu/visible

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
ui_startmenu: open { return self.open() } /internal/startmenu/open
ui_startmenu: close { return self.close() } /internal/startmenu/close
ui_startmenu: search { return self.search(query) } /internal/startmenu/search
ui_startmenu: launch { return self.launch(app) } /internal/startmenu/launch
ui_startmenu: power { return self.power(action) } /internal/startmenu/power

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    startmenu_opened() ｜ /internal/event/startmenu_open
    startmenu_closed() ｜ /internal/event/startmenu_close
    app_launched(app: str) ｜ /internal/event/startmenu_launch
    power_requested(action: str) ｜ /internal/event/startmenu_power

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    search: 30次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    launch: user_foreground
    power: aux_system
```

契约要点：

1. 开始菜单对象 `/object/ui/startmenu`：左下角弹出面板，应用列表 / 固定项 / 最近使用 / 搜索 / 电源。
2. 写字段 open / close / search / launch / power：打开、关闭、搜索过滤、启动应用、电源操作。
3. `launch` 授权 `user_foreground`；`power`（关机 / 重启 / 注销）仅 `aux_system`。
4. 事件供程序自动化感知菜单开关、应用启动与电源请求（`startmenu_opened` / `power_requested`）。

## 21 视觉风格（类 Windows 11 圆角）

1. **默认视觉风格**为类 Windows 11 圆角风格（主题资源存放于 `/system/desktop/themes`）。
2. **圆角半径**：
   - 主窗口：8px；
   - 弹窗 / 菜单 / 开始菜单：6px；
   - 控件（按钮 / 输入框 / 列表）：4px。
3. **窗口对象 `prop` 增加 `round_radius` 条目**（像素，0=直角）；`ui.window.api` 透传 `round_radius`。
4. **阴影**：窗口带层级阴影（层级越高阴影越深、越柔和），配合图层表（第 11.3 节）。
5. **透明度**：任务栏 / 开始菜单 / 通知面板采用半透明亚克力效果。
6. **动效**：窗口打开 / 关闭为平滑缩放 + 淡入淡出（约 150ms）；开始菜单弹出为缩放展开。
7. **主题切换**：`themes` 提供 Win11 圆角（默认）与 Win10 直角两套主题，可全局切换；切换经主题配置（`config.md`）。
## 22 文本输出窗口（控制台渲染）

1. 图形模式下，进程的文字输出经控制台渲染为**该进程自己的窗口**（非全屏字符模式；全屏文字模式仅存在于系统启动阶段，`boot.md`）。
2. **渲染**：字符栅格 + 滚动 + 光标；字体经 `font.md`，控件 / 渲染代码经图形界面专用动态链接库对象（`uio.md`）。
3. **窗口属性**：等宽字体、固定行宽 / 行高、可滚动、文本可选中复制（`clipboard.md`）。
4. **终端仿真**：支持标准转义序列（光标移动 / 清屏 / 颜色），由合成器文本渲染器解释。
5. **设备化**：文本输出可连接服务实现的**虚拟终端设备**（`hardware_svc.md` 第 7 节），如虚拟终端 / 虚拟串口。
6. **联动**：进程控制台对象 `/object/process/<名称>/console`（`process.md` 第 8 节），写入经 `process.api` 的 console_write。