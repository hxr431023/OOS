# fs.md 文件子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 基于 `os.md` 对象‑条目模型；分为两大部分：**真实磁盘文件系统** + **VFS 兼容适配层**。
> 2. 磁盘文件系统运行在内核可加载模块；VFS 作为适配层，把磁盘文件、内核 IO 流对象统一对外提供 POSIX 接口。
> 3. Ring1/Ring2 保持闲置；复用 `acl.md`、`memory.md`。
> 4. 区分：**持久磁盘文件**（存于块设备）、**内存虚拟节点**（内核 IO 流对象映射，无磁盘存储）。

## 术语

- 块设备对象：IO 流对象，代表磁盘 / U 盘，路径`/object/blk/<blk_id>`；
- 挂载点：VFS 目录树上的路径，将后端（磁盘文件系统 / 内存 IO 流对象）绑定到目录；
- inode：磁盘文件系统内部索引，代表文件 / 目录本体；
- dentry：目录项，文件名到 inode 的映射；
- 内存虚拟节点：VFS 把 IO 流对象的条目伪装成文件，没有磁盘实体，读写直接透传到内核条目。

## 1 总体约束

1. 真实磁盘文件系统属于内核模块，运行 Ring0；故障会造成内核 panic。
2. VFS 兼容层是翻译适配层，本身**不存储任何数据**，只分发操作到两类后端：
   - 后端 A：磁盘文件系统（持久化，块设备存储）
   - 后端 B：内核 IO 流对象（内存虚拟节点，映射对象条目）
3. VFS 向外提供标准 POSIX：`open/read/write/stat/ioctl`；
   - 访问**内存虚拟节点**：操作会翻译为原生对象 API 的`ObjectReadEntry / ObjectWriteEntry`；
   - 访问**磁盘文件**：调用磁盘文件系统模块读写块设备。
4. VFS 通路权限和底层条目 ACL 双向同步；磁盘文件拥有自己的 POSIX 权限模型。
5. 纯控制对象**不能被 VFS 挂载**，只能使用原生对象句柄访问。

## 2 VFS 全局目录树（沿用 os.md 定义）

```
/system/dev      # IO流对象映射虚拟设备节点
/system/proc     # 进程对象映射虚拟节点
/system/sys      # 硬件对象映射设备树
/disk/*          # 磁盘、U盘挂载点
/usr /home       # 持久磁盘文件目录
/tmp             # 临时内存文件目录
```

## 3 块设备对象（磁盘硬件）

每一个磁盘块设备为 **IO 流对象** `/object/blk/<blk_id>`

表格

| 条目名称 | 权限 | 说明 |
| --- | --- | --- |
| `io_data` | 可读可写 | 块读写入口，块号为单位读写磁盘 |
| `info` | 只读 | 磁盘容量、块大小、序列号 |
| `ctl` | 可读可写 | 控制：刷新缓存、弹出介质 |

> 
> 磁盘文件系统内核模块打开块设备对象句柄，通过条目读写磁盘块。

## 4 挂载操作逻辑

挂载不是独立系统调用，操作 VFS 对应的控制条目完成。

1. VFS 根对象存在挂载控制条目`mount_ctl`。
2. 写入挂载命令结构体：指定挂载目标目录、后端来源（块设备 ID / IO 流对象路径）、挂载参数。
3. 内核校验权限，执行挂载：
   - 如果后端是块设备：加载对应磁盘文件系统，解析分区、超级块；
   - 如果后端是 IO 流对象：把该对象全部条目映射为 VFS 下虚拟文件，形成内存虚拟节点。
4. 卸载：向`mount_ctl`写入卸载命令。

> 
> 内存虚拟节点挂载示例：将`/object/hw`硬件 IO 流对象挂载到`/system/sys`。

## 5 两类后端行为差异

### 后端 1：磁盘文件系统（持久化）

- 文件、目录保存在块设备；断电数据保留；
- 使用传统 inode/dentry；支持 uid/gid、rwx 权限；
- read/write 操作落到磁盘块缓存，回写到物理介质。

### 后端 2：IO 流对象 → 内存虚拟节点

- 一条条目对应 VFS 下一个文件；
- `read()` → `ObjectReadEntry()`
- `write()` → `ObjectWriteEntry()`
- cat、echo 脚本可以直接操作内核状态；
- 关机全部消失，不写磁盘；
- 权限取自底层对象条目的 ACL，自动转换 r/w。

> 
> 限制：**不支持 creat/unlink**；不能通过 VFS 创建删除条目；`CREATE_ENTRY`/`DELETE_ENTRY`只能走原生对象 API。

## 6 文件描述符与对象句柄关系

1. Linux 兼容层进程的 fd（文件描述符）分为两类：
   - 磁盘文件 fd：指向 VFS 内部文件上下文；
   - 内存虚拟节点 fd：内部持有对应内核对象句柄。
2. `close(fd)`：磁盘文件关闭 VFS 上下文；虚拟节点会调用`CloseHandle`释放对象句柄，引用计数递减。
3. dup/dup2：复制 fd；虚拟节点场景复制对象句柄，增加引用计数。

## 7 和其它文档对接

1. **os.md**：VFS 是两套访问入口之一；IO 流对象可挂载 VFS，纯控制对象不可挂载。
2. **acl.md**：内存虚拟节点的 r/w 权限来自条目 ACL；CREATE/DELETE_ENTRY 不暴露 POSIX。
3. **hardware_svc.md**：`/system/dev`来自硬件 IO 流对象挂载。
4. **process.md**：进程 fd 表，虚拟节点 fd 内部保存内核对象句柄。
5. **Linux.md**：完整提供 POSIX 文件系统 syscall；proc/sys/dev 全部是内存虚拟节点后端。
6. **boot.md**：内核启动阶段挂载根磁盘；挂载`/system/proc` `/system/sys`虚拟节点。
7. **filemgr.md**：文件管理器作为 VFS 的用户态前端，浏览 `/usr`、`/home`、`/disk` 目录与虚拟节点。

## 8 设计取舍

1. 磁盘文件系统保留传统 inode 模型，保证兼容 POSIX 文件语义；
2. 虚拟节点不重新实现一套内存文件，直接复用已有的内核对象条目，消除重复存储；
3. 创建 / 删除条目禁止通过 open/unlink，封锁越权途径，强制走原生对象 API；
4. VFS 只做适配转发，业务逻辑分别下沉到磁盘模块 / 对象管理器。

## 9 系统文件后缀规范

1. **`.sys` 系统文件后缀**：系统二进制程序，以及非文本、非其他类型的系统文件，统一使用 `.sys` 后缀标识（系统级文件）。
2. 文本、配置、文档等已定义格式的文件，按其既有后缀 / 格式存放（见各子系统规范）；不属于任何已定义格式的系统二进制 / 杂项文件一律归入 `.sys`。
3. 运行位置（内核态 / 用户态）规则见 os.md 第 3 节：与内核相关→内核态；易崩溃 / 与内核无关→用户态。

## 10 系统目录树

完整 /system 目录树已从本文档分离，独立存放于 dirtree.md（唯一权威版本）；media/ 子目录展开见 dirtree.md。

## 11 文件 `.api` 契约文件（fs.api 全文）

文件子系统对外接口统一由以下契约文件声明，文件存放于 `/system/api/fs.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# fs.api 文件子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = fs
env = python
exec = /system/kernel/modules
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 1000次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/fs fs

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
fs: mounts { } /internal/fs/mounts
fs: block_info { } /internal/fs/block
fs: stat { } /internal/fs/stat

# ---- 可写控制条目：转换代码为Python，实现挂载与查询 ----
fs: mount {
    return self.mount(target, backend, opts)
} /internal/fs/mount
fs: umount {
    return self.umount(target)
} /internal/fs/umount

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    fs_mounted(path: str) ｜ /internal/event/fs_mount
    fs_umounted(path: str) ｜ /internal/event/fs_umount

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    mount: 10次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    mount: kernel
    umount: kernel
```

契约要点：

1. 挂载 / 卸载（`mount` / `umount`）仅授权 `kernel`（管理员）；文件读写经 VFS POSIX 或原生对象条目，权限由底层条目 ACL 自动翻译。
2. 挂载表、块设备信息、文件状态只读透传。
3. 挂载 / 卸载事件供审计（`log.md`）监听。

