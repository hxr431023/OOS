# filemgr.md 文件管理器子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；文件管理器为**用户态应用/服务**，运行于 Ring3，底层文件操作全部经 `fs.md` VFS 与块设备对象完成。
> 2. Ring1/Ring2 闲置；文件操作受 `acl.md` 权限管控；删除、移动等敏感操作写入审计日志（`log.md`）。
> 3. **条目不在此文档硬编码**：文件管理器对外条目的集合、命名、权限由 `filemgr.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义文件管理器的行为语义，契约全文见第 7 节。
> 4. 对应目录树 `/system/filemgr/`：系统文件管理器主程序。
> 5. 依赖：`os.md`、`acl.md`、`fs.md`、`process.md`、`log.md`、`storage.md`、`security.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 文件管理器 | 面向用户的文件浏览与操作程序，管理 `/usr`、`/home`、`/disk` 等目录 |
| 文件操作 | 复制、移动、重命名、删除、新建目录等文件系统操作 |
| 文件关联 | 按文件类型关联默认打开程序 |
| 虚拟节点 | 内存虚拟节点（IO 流对象映射），文件管理器中以特殊视图展示 |

## 1 定位与职责

文件管理器（`/system/filemgr`）承担：

1. 目录浏览：遍历 `/usr`、`/home`、`/disk/*` 等目录，展示文件与目录；
2. 文件操作：复制、移动、重命名、删除、新建目录；
3. 文件属性查看与修改（权限、所有者、大小、时间戳）；
4. 文件关联：按扩展名/类型关联默认打开程序；
5. 与存储卷、磁盘加密、备份联动（展示可移动设备、加密卷、还原点）。

## 2 核心对象模型

文件管理器以**文件管理对象** `/object/filemgr/mgr`（纯控制对象）为核心载体，内部持有以下条目（**布局由 `filemgr.api` 契约声明**）：

1. `list`：只读，目录内容列表（名称、类型、大小、权限、修改时间）。
2. `copy`：可写，复制请求（源、目标）。
3. `move`：可写，移动请求（源、目标）。
4. `rename`：可写，重命名请求。
5. `delete`：可写，删除请求。
6. `mkdir`：可写，新建目录请求。
7. `stat`：只读，指定文件属性。
8. `event`：等待可读，文件操作完成/失败事件。

## 3 文件操作流程

1. 调用方向文件管理对象条目写入操作请求（复制/移动/重命名/删除/新建）。
2. 内核校验调用方 ACL 权限与目标路径权限（`acl.md`、`fs.md` POSIX 权限）。
3. 文件管理器经 VFS 接口执行底层文件系统操作（`fs.md` 后端 A 磁盘文件系统）。
4. 操作结果回写（`list` 刷新、`event` 事件通知），写入审计日志（`log.md`）。
5. 内存虚拟节点（`/system/dev`、`/system/proc` 等）在文件管理器中以特殊视图只读展示，不允许删除/重命名。

## 4 文件关联与打开

1. 文件管理器按扩展名/类型匹配关联默认程序（应用启动器、图像查看器、文本编辑器等）。
2. 打开文件经 `process.md` 派生目标程序进程，传入文件路径。
3. 未关联类型提示用户选择打开方式。

## 5 容错与安全

1. 删除/移动为破坏性操作：文件管理器二次确认 + ACL 校验 + 审计留痕。
2. 加密卷未解锁时显示锁定状态，不可浏览内容（`security.md` 磁盘加密）。
3. 文件管理器进程崩溃：init 重启，不丢失文件系统数据（数据由 `fs.md` 保证）。
4. 虚拟节点受 `fs.md` 约束：不支持 creat/unlink，文件管理器对虚拟节点不做增删操作。

## 6 与各子系统对接

1. **os.md**：文件管理对象为纯控制对象，复用对象‑条目模型。
2. **acl.md**：文件操作受 ACL 管控；删除等敏感操作需授权。
3. **fs.md**：底层文件浏览与操作全部经 VFS 完成。
4. **process.md**：打开文件派生子进程；文件管理器生命周期由进程子系统承载。
5. **log.md**：删除、移动等敏感操作写入审计日志。
6. **storage.md**：可移动设备、存储卷在文件管理器中展示。
7. **security.md**：加密卷解锁与文件访问联动。
8. **.api.md**：文件管理器对外接口经 `filemgr.api` 契约与桥接层。
9. **img.md**：文件管理器按文件关联调用系统唯一截屏与图片应用（img.md）打开图片。
10. **note.md**：文件管理器按文件关联调用系统唯一文档编辑器（note.md）打开文本/文档。
11. **video.md**：文件管理器按文件关联调用系统自带视频应用（video.md）打开视频。

## 7 文件管理器 `.api` 契约文件（filemgr.api 全文）

文件管理器对外接口统一由以下契约文件声明，文件存放于 `/system/api/filemgr.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# filemgr.api 文件管理器子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = filemgr
env = python
exec = /system/filemgr
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 100次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/filemgr filemgr

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
filemgr: list { } /internal/filemgr/list
filemgr: stat { } /internal/filemgr/stat

# ---- 可写控制条目：转换代码为Python，实现文件操作 ----
filemgr: copy {
    self.copy(src, dst)
} /internal/filemgr/copy
filemgr: move { self.move(src, dst) } /internal/filemgr/move
filemgr: rename { self.rename(path, new_name) } /internal/filemgr/rename
filemgr: delete {
    if self.is_virtnode(path):
        raise PermissionError("虚拟节点不可删除")
    self.delete(path)
} /internal/filemgr/delete
filemgr: mkdir { self.mkdir(path) } /internal/filemgr/mkdir

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    file_op_done(op: str, path: str) ｜ /internal/event/file_op_done
    file_op_failed(op: str, path: str, reason: str) ｜ /internal/event/file_op_fail

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    delete: 50次/秒
    move: 50次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    delete: user_foreground
    move: user_foreground
```

契约要点：

1. 外部程序仅能访问 `/system/api/filemgr` 标准虚拟对象，禁止直连文件管理对象内部条目。
2. `delete` 接口经 Python 转换代码校验目标是否为虚拟节点，虚拟节点拒绝删除。
3. 事件仅封装契约声明的标准参数；文件操作完成/失败事件供文件管理器与其它调用方监听。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 8 设计取舍

1. 文件管理器为独立用户态应用，崩溃不影响文件系统数据。
2. 文件操作统一走对象条目与 ACL，敏感操作可审计。
3. 虚拟节点以只读视图隔离，防止误操作破坏内核状态映射。
4. 对外只暴露 `/system/api/filemgr` 标准接口，内部浏览逻辑完全隐藏。
