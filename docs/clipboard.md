# clipboard.md 剪贴板规范

> 
> 前提说明
> 
> 1. 提供系统级剪贴板：文本、图片、文件引用等多格式数据的**写入、读取与变更通知**。
> 2. 剪贴板所有权模型：当前持有者（前台窗口所属进程）独占写权；持有者退出或注销时剪贴板内容失效。
> 3. 大数据（图片 / 文件）采用**延迟渲染（lazy rendering）**：复制时只登记格式与持有者，粘贴时才向持有者取数据。
> 4. 剪贴板内容承载于 IPC 对象（复用 `ipc.md` 对象条目模型），不引入独立内核机制。
> 5. 敏感数据（密码等）不应进剪贴板；确需复制时由应用自行处理（安全建议见 `secure_island.md`）。

## 1 总体约束

1. 剪贴板对象路径 `/object/clipboard/main`，条目见下。
2. 写权限：仅当前前台窗口所属进程；读权限：前台会话任意进程（粘贴即读取）。
3. 变更即广播事件：粘贴板监听者（如剪贴板历史）实时感知。
4. 延迟渲染：登记格式列表；读取时若持有者存活则回调取数，否则返回格式失效。
5. 会话切换 / 注销：剪贴板随会话隔离，注销后内容销毁。

## 2 剪贴板对象条目布局

### 剪贴板对象，路径 `/object/clipboard/main`

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `formats` | 只读 | 当前可用格式列表（文本 / 图片 / 文件引用 / 自定义） |
| `data` | 可读可写 | 立即数据（小数据直存）；读取触发延迟渲染回调 |
| `owner` | 只读 | 当前持有者（进程 / 窗口） |
| `claim` | 只写 | 声明持有剪贴板（前台进程自动获得） |
| `clear` | 只写 | 清空剪贴板 |

## 3 核心操作流程示例

### 示例 1：复制文本

1. 前台进程写入 `data`，声明 `claim`；
2. 剪贴板登记格式（text/plain），广播 `clip_changed`；
3. 其他进程读取 `data` 获得文本；持有者退出后内容自动失效。

### 示例 2：复制文件引用（延迟渲染）

1. 文件管理器登记格式（file/ref）+ 持有者；
2. 粘贴方读取时，剪贴板向持有者回调取文件路径；
3. 持有者已退出 → 返回格式失效，粘贴方提示。

## 4 和其它文档对接关系

1. **对接 ui.md**：前台窗口与焦点决定剪贴板持有者；粘贴快捷键由输入子系统分发。
2. **对接 ipc.md**：剪贴板对象复用 IPC 对象条目模型与 ACL。
3. **对接 note.md / img.md / filemgr.md**：文本 / 图片 / 文件引用的实际读写方。
4. **对接 session.md**：剪贴板随会话隔离与销毁。
5. **对接 input.md**：粘贴动作（Ctrl+C / Ctrl+V）经输入事件链触发。

## 5 设计取舍

1. 延迟渲染避免复制大文件时立即搬运数据，粘贴时才取数，省内存。
2. 所有权归前台：剪贴板内容不会被后台进程偷偷覆盖。
3. 随会话隔离：多用户快速切换互不可见，兼顾隐私。

## 6 剪贴板 `.api` 契约文件（clipboard.api 全文）

剪贴板对外接口统一由以下契约文件声明，文件存放于 `/system/api/clipboard.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# clipboard.api 剪贴板对外接口契约
version = 1.0
target = clipboard
env = python
exec = /system/desktop/clipboard/clip_svc
internal_loc = map + symbol + register
min_os_version = 1.0
global_acl = user_foreground
global_limit = 200次/秒

object /system/api/clipboard clipboard

# ---- 只读条目 ----
clipboard: clip_formats { } /internal/clipboard/formats
clipboard: clip_owner { } /internal/clipboard/owner

# ---- 可写控制条目 ----
clipboard: clip_set {
    return self.clip_set(fmt, data)
} /internal/clipboard/set
clipboard: clip_get {
    return self.clip_get(fmt)
} /internal/clipboard/get
clipboard: clip_claim {
    return self.clip_claim()
} /internal/clipboard/claim
clipboard: clip_clear {
    return self.clip_clear()
} /internal/clipboard/clear

# ---- 对外可监听标准事件 ----
EVENT:
    clip_changed(formats: str) ｜ /internal/event/clipboard

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    clip_set: user_foreground
    clip_claim: user_foreground
```

契约要点：

1. 写与声明仅限前台进程（`user_foreground`），防止后台覆盖剪贴板。
2. `clip_get` 触发延迟渲染回调，持有者失效时返回错误。
3. `clip_changed` 事件供剪贴板历史 / 同步工具监听。
