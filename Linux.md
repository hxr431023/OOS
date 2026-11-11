# Linux 兼容层规范（POSIX + VFS + 文件系统）

> 
> 前提说明
> 
> 
> 1. 本系统 Linux 兼容层实现三大块：**POSIX 系统调用（含 POSIX 扩展）**、**VFS 虚拟文件系统**、**真实文件系统**。这三块已能支撑大量 Linux 程序、命令行工具与脚本运行，不是简易版，而是刻意裁剪的实现范围。
> 2. 本规范只定义这三块；其余 Linux 全接口体系（ABI 细节、伪文件系统、glibc 运行时、网络语义、系统层级约定等）不在此范围，按需扩展。
> 3. **提供自研 `.api` 契约**：Linux 兼容层除 Linux / POSIX 接口外，经 `linux.api` 契约对外提供交互能力，支撑**普通应用与 Linux 应用通讯**、**开启 / 关闭应用**、**结束进程**、外部交互与**程序自动化**。
> 4. 依赖：`os.md`（对象‑条目模型）、`fs.md`（文件子系统）、`objmgr.md`。

## 1 POSIX 系统调用层（接口层）

兼容层的核心接口层，提供标准 POSIX 系统调用，是 Linux 程序与自研内核之间的翻译落点。覆盖 **POSIX 标准核心接口** 与 **POSIX 扩展接口** 两部分；`clone`、`futex`、`epoll`、`io_uring` 等 Linux 专属扩展接口不在此范围。

### 1.1 POSIX 标准核心接口

| 功能分类 | POSIX 标准核心接口 |
| --- | --- |
| 文件 I/O 基础 | `open/close/read/write/lseek/stat/fstat/access/chmod/chown/dup/dup2/fsync/fcntl/ioctl/truncate` |
| 进程管理 | `fork/execve/exit/_exit/wait/waitpid/getpid/getppid/setuid/getuid/nice/sched_yield` |
| 内存管理 | `mmap/munmap/mprotect/msync/brk/mlock/munlock` |
| 信号机制 | `kill/sigaction/sigprocmask/sigsuspend/alarm/pause` |
| 时间定时器 | `time/gettimeofday/clock_gettime` |
| 网络套接字 | `socket/bind/listen/accept/connect/send/recv/shutdown/getsockopt/setsockopt` |
| 进程间通信 | `pipe/mkfifo/shmget/shmat/semget/semop/msgget/msgsnd` |
| 文件系统操作 | `mkdir/rmdir/rename/link/unlink/symlink/readlink/chdir/getcwd/mount/umount` |
| 系统信息控制 | `uname/sysinfo/getrlimit/setrlimit/umask` |

### 1.2 POSIX 扩展接口

| 功能分类 | POSIX 扩展接口 |
| --- | --- |
| 进程创建扩展 | `posix_spawn/posix_spawnp/setpgid/getpgrp/setsid/getlogin` |
| 线程（POSIX threads） | `pthread_create/pthread_join/pthread_detach/pthread_mutex_*/pthread_cond_*/pthread_attr_*/pthread_rwlock_*` |
| 消息队列 | `mq_open/mq_close/mq_send/mq_receive/mq_notify/mq_unlink` |
| 共享内存 | `shm_open/shm_unlink/shmat/shmdt` |
| 信号量 | `sem_init/sem_wait/sem_post/sem_trywait/sem_open/sem_unlink` |
| 定时器与时钟扩展 | `nanosleep/clock_nanosleep/timer_create/timer_settime/timer_gettime` |
| 信号扩展 | `sigqueue/sigtimedwait/sigwaitinfo/sigwait` |
| 异步 I/O | `aio_read/aio_write/aio_error/aio_suspend/aio_cancel` |
| 文件与同步扩展 | `fdatasync/sync/pread/pwrite/ftruncate` |
| 目录遍历 | `opendir/readdir/rewinddir/closedir/scandir` |

> 
> 覆盖以上核心 + 扩展接口，脚本、命令行工具、shell、解释器与多数传统程序均可稳定运行；程序自动化（如经 `ioctl` 下发控制、经脚本与命令接口调用）同样经上述 Linux / POSIX 接口实现。

### 1.3 接口映射表（TMMF）

1. **接口映射表**以 TMMF 格式（`tmmf.md`）表达：Linux / POSIX 接口名到自研本地接口（对象条目 / 转换代码 / 多接口组合）的映射。
2. **行格式**：数据行 `<\>标签组=内容组</>`——等号**左侧为搜索词**（Linux 接口名 / 别名，多个用**逗号**分隔），**右侧为映射结果**（本地接口 / 转换目标，多个用**分号**分隔）。
3. **多搜索词 → 同一结果**：同一接口的多个别名（如 `lstat` / `fstatat`、`dup` / `dup2`）并列为多个搜索词，命中任一搜索词即映射到同一组结果。
4. **使用位置**：兼容层按接口名查表，命中即按结果执行转换（调用本地对象条目 / Python 转换代码 / 组合接口）；未命中的接口按默认路径翻译。
5. 映射表文件存放于 Linux 兼容层目录（`/system/linux/`），随兼容层加载。

示例数据行：

```text
<\>lstat,fstatat=fs.stat;fs.statx</>
<\>dup,dup2=process.dup_fd</>
<\>mq_open,mq_send=ipc.mq.open;ipc.mq.send</>
```

## 2 VFS 虚拟文件系统层（适配层）

1. **定位**：不单兼容 Linux 二进制程序，同时适配所有习惯 Linux 交互逻辑的自研工具、shell 脚本，提供标准 POSIX `open/read/write/ioctl`。
2. **目录设计**：单根统一目录树（无 Windows 多盘符）
   - `/system/dev`：IO 流对象虚拟设备 / 配置节点
   - `/system/proc`：进程、硬件状态虚拟节点
   - `/system/sys`：硬件设备树
   - `/disk/*`：磁盘、U 盘挂载点
   - `/usr /home`：持久化磁盘文件目录
3. **两大存储后端共用一套 VFS 调度**
   ① 持久磁盘文件系统（系统刚需，不可关闭）：存储程序、用户文件，断电保留数据。
   ② 内存虚拟节点（兼容附加层，可裁剪关闭）：映射 IO 流对象条目，不占磁盘，关机清空；读写直接穿透到内核条目，无独立数据副本。
4. **特性**：VFS 只做适配转发，本身不存储任何数据；详细语义见 `fs.md`。

## 3 文件系统层（持久存储）

1. 真实磁盘文件系统属于内核模块，运行 Ring0；保留传统 inode 模型，保证兼容 POSIX 文件语义。
2. 提供持久化存储：程序、用户文件、配置，断电保留数据。
3. 通过块设备对象 `/object/blk/<blk_id>` 读写磁盘块；挂载到 VFS 目录树 `/disk/*`、`/usr`、`/home`。
4. 与 VFS 的关系：磁盘文件系统是 VFS 的后端 A；VFS 把磁盘文件与内核 IO 流对象统一对外提供 POSIX 接口（详见 `fs.md`）。

## 与自研子系统对接

- **cmd.md**：shell 脚本、命令工具兼容依赖命令子系统（命令执行对象 + `/system/api/cmd`）。
- **fs.md / net.md / process.md**：Linux 系统调用与各子系统的翻译对接见各自文档。
- **objmgr.md**：Linux 兼容层所需对象基础由对象管理器承载。
- **tmmf.md**：接口映射表（第 1.3 节）以 TMMF 格式表达，存放于 /system/linux/。
- **linux.api 契约**：Linux 兼容层经 `.api` 契约对外提供应用 / 进程管理与跨侧通讯接口（全文见下节）。

## Linux 兼容层 `.api` 契约文件（linux.api 全文）

Linux 兼容层对外接口统一由以下契约文件声明，文件存放于 `/system/api/linux.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# linux.api Linux 兼容层对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.1
# 绑定目标子系统标识
target = linux
# 转换代码运行环境
env = python
# 所属可执行文件命名空间（Linux 兼容层主可执行程序）
exec = /system/linux_compat/compat
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 500次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/linux linux

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
linux: elf_info { } /internal/linux/elf
linux: proc_nodes { } /internal/linux/proc
linux: syscall_table { } /internal/linux/syscall
linux: running_apps { } /internal/linux/apps
linux: process_list { } /internal/linux/process
linux: export_table { } /internal/linux/exports

# ---- 可写控制条目：转换代码为Python，实现 ELF 加载、翻译与交互管理 ----
linux: load_elf {
    return self.load_elf(path)
} /internal/linux/load
linux: translate {
    return self.translate(api)
} /internal/linux/translate
linux: register_syscall {
    return self.register_syscall(no, handler)
} /internal/linux/syscall_reg

# ---- 应用 / 进程交互管理：开启、关闭、结束进程 ----
linux: launch_app {
    return self.launch_app(path)
} /internal/linux/launch
linux: close_app {
    return self.close_app(name)
} /internal/linux/app_close
linux: close_all_apps {
    return self.close_all_apps()
} /internal/linux/app_close_all
linux: kill_process {
    return self.kill_process(pid)
} /internal/linux/process_kill

# ---- 跨侧通讯：普通应用 ↔ Linux 应用 ----
linux: ipc_send {
    return self.ipc_send(target, data)
} /internal/linux/ipc_send
linux: ipc_recv {
    return self.ipc_recv(chan)
} /internal/linux/ipc_recv

# ---- 程序自动化：脚本 / 命令执行 ----
linux: run_script {
    return self.run_script(script)
} /internal/linux/run
# ---- Linux 程序暴露接口：登记、注销、调用（途径开放给其他 Linux 应用与原生应用） ----
linux: export_register {
    return self.export_register(name, prog, sig, loc)
} /internal/linux/export_reg
linux: export_unregister {
    return self.export_unregister(name)
} /internal/linux/export_unreg
linux: export_call {
    return self.export_call(name, args)
} /internal/linux/export_call

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    elf_loaded(path: str) ｜ /internal/event/linux_load
    app_launched(name: str) ｜ /internal/event/linux_app_launch
    app_closed(name: str) ｜ /internal/event/linux_app_close
    process_exited(pid: int) ｜ /internal/event/linux_proc_exit
    ipc_received(from: str, data: str) ｜ /internal/event/linux_ipc
    export_registered(name: str, prog: str) ｜ /internal/event/linux_export_reg
    export_unregistered(name: str) ｜ /internal/event/linux_export_unreg

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    run_script: 10次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    kill_process: aux_system
    close_all_apps: aux_system
```

契约要点：

1. `load_elf` 加载 Linux ELF 二进制并启动；`translate` 将 Linux 系统调用 / POSIX 接口翻译为自研子系统对象条目调用；`register_syscall` 注册系统调用号翻译映射。
2. `launch_app` / `close_app` / `close_all_apps` / `kill_process`：开启应用、关闭单个 / 所有应用、结束一个进程（应用与进程级交互管理）。
3. `ipc_send` / `ipc_recv`：**普通应用与 Linux 应用之间的跨侧通讯**（外部交互）。
4. `run_script`：程序自动化——脚本 / 命令执行。
5. `export_register` / `export_unregister` / `export_call` / `export_table`：Linux 程序暴露的接口（ELF 动态符号 / 运行时注册）经 `export_register` 登记后，`export_call` 将调用途径开放给**其他 Linux 应用与原生应用**；接口内部定位遵循 `.api` 4.9 内部定位规则。
6. 事件供程序自动化感知应用启停、进程结束、通讯消息与接口登记变化（`app_launched` / `app_closed` / `process_exited` / `ipc_received` / `export_registered` / `export_unregistered`）。
