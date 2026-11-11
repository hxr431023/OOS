# note.md 便签及文档子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；便签及文档为**系统唯一自带的文档格式编辑器与便签应用**，运行于 Ring3，类似 Windows 记事本 + 便签。
> 2. Ring1/Ring2 闲置；文档读写受 `acl.md` 权限管控。
> 3. **条目不在此文档硬编码**：便签及文档对外条目的集合、命名、权限由 `note.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义便签及文档的行为语义，契约全文见第 7 节。
> 4. 对应目录树 `/system/SystemApps/note/`：便签与文档编辑器主程序、便签组件、文档编辑器、文档格式库、配置。
> 5. 依赖：`os.md`、`acl.md`、`fs.md`、`process.md`、`ui.md`、`filemgr.md`、`media.md`、`log.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 便签 | 桌面便签，快捷记录并可贴在桌面（类似 Windows 便签） |
| 文档编辑器 | 打开、编辑、保存文本及文档格式（类似 Windows 记事本） |
| 文档格式 | 纯文本（txt）、富文本（rtf/md）等文档格式 |
| 文档格式库 | 文档格式编解码组件 |

## 1 定位与功能

便签及文档为**系统唯一自带**的文档编辑与便签应用（`/system/SystemApps/note/`），承担：

1. **便签**：创建多张桌面便签，快捷记录，可贴在桌面常驻（类似 Windows 便签）。
2. **文档编辑**：打开、编辑、保存文本及文档格式（纯文本 / 富文本），类似 Windows 记事本。
3. **唯一性**：系统不提供其它自带文档编辑器或便签应用；文档格式编辑统一由本应用承担。

## 2 便签

1. 便签组件（`sticky_notes`）可创建多张便签，每张便签为独立内容对象。
2. 便签可贴在桌面（`ui.md` 桌面），置顶显示；可编辑文字、改颜色、置顶/隐藏、删除。
3. 便签内容自动保存（`fs.md`），重启后恢复。
4. 便签为轻量记录，不做格式排版。

## 3 文档编辑

1. 文档编辑器（`editor`）打开 / 编辑 / 保存文本及文档格式。
2. 支持格式：
   - 纯文本（txt）：普通文本编辑；
   - 富文本（rtf / md）：加粗、斜体、列表、标题等基础排版。
3. 编辑功能：输入、撤销/重做、查找替换、保存/另存为。
4. 文档格式编解码由文档格式库（`formats`）承担。
5. 打开文档经 `filemgr.md` 文件关联调用本应用。

## 4 与各子系统对接

1. **os.md**：便签与文档对象为纯控制对象，复用对象‑条目模型。
2. **acl.md**：文档读写受 ACL 管控。
3. **fs.md**：便签与文档文件读写遵循文件系统。
4. **process.md**：应用生命周期；崩溃回收。
5. **ui.md**：便签显示在桌面；文档渲染到窗口。
6. **filemgr.md**：文件管理器按文件关联调用本应用打开文档。
7. **media.md**：文档内嵌图片/资源经媒体资源库（可选）。
8. **log.md**：文档编辑敏感操作写入审计日志。
9. **.api.md**：便签及文档对外接口经 `note.api` 契约与桥接层。

## 5 容错

1. 应用崩溃：init 重启；已保存便签/文档不受影响。
2. 未保存编辑崩溃：丢失本次编辑，原文件不受影响。
3. 打开损坏文档：报错提示，不崩溃。

## 6 便签及文档 `.api` 契约文件（note.api 全文）

便签及文档对外接口统一由以下契约文件声明，文件存放于 `/system/api/note.api` 与 `/system/api/note.doc.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# note.sticky.api 便签对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = note.sticky
env = python
exec = /system/SystemApps/note/note_app
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 50次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/note/sticky note_sticky

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
note_sticky: note_list { } /internal/sticky/list
note_sticky: note_count { } /internal/sticky/count

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
note_sticky: create { return self.create(title) } /internal/sticky/create
note_sticky: delete { return self.delete(note_id) } /internal/sticky/delete
note_sticky: set_text { return self.set_text(note_id, text) } /internal/sticky/set_text
note_sticky: pin { return self.pin(note_id, on) } /internal/sticky/pin

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    note_created(note_id: str) ｜ /internal/event/sticky_created
    note_deleted(note_id: str) ｜ /internal/event/sticky_deleted
    note_updated(note_id: str) ｜ /internal/event/sticky_updated

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    set_text: 100次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    create: user_foreground
```

```text
# note.doc.api 文档编辑器对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = note.doc
env = python
exec = /system/SystemApps/note/note_app
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 50次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/note/doc note_doc

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
note_doc: open_file { } /internal/doc/file
note_doc: status { } /internal/doc/state

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
note_doc: open { return self.open(path) } /internal/doc/open
note_doc: save { return self.save(path) } /internal/doc/save
note_doc: undo { return self.undo() } /internal/doc/undo
note_doc: find_replace { return self.find_replace(find, rep) } /internal/doc/find_replace

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    doc_opened(path: str) ｜ /internal/event/doc_opened
    doc_saved(path: str) ｜ /internal/event/doc_saved

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    save: 30次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    open: user_foreground
```

契约要点：

1. 便签对象内部路径 `/object/note/sticky`，文档对象 `/object/note/doc`。
2. 便签写字段为创建 / 删除 / 写文本 / 置顶命令；文档写字段为打开 / 保存 / 撤销 / 查找替换命令。
3. 便签与文档编辑为普通编辑操作，不强制审计（audit=false）。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 7 设计取舍

1. 便签与文档合并为系统唯一编辑应用，避免多应用重复。
2. 便签独立组件，轻量常驻桌面；文档编辑器完整排版。
3. 文档格式库独立，纯文本 / 富文本统一编解码。
4. 文档打开经 `filemgr.md` 关联，交互统一。
5. **系统核心应用**：承担唯一文档格式编辑，**不可卸载**（卸载会连带卸载相关权限与组件致系统损坏，见 `pkg.md`）。
6. **卸载连带影响**：本应用拥有**独特的 API、独特的能力、独特的库、独特的程序与能力**；若被卸载，将**连带卸载**其关联的 API 契约、库与组件（不止一个）。其它应用可能**依赖本应用的 API**——刚卸载一个，依赖它的下一个应用随即用不了。因此应用间因 API 依赖相互锁定，系统自带应用**不可卸载**（见 `pkg.md`）。
