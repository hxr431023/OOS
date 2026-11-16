# ipc.md 对象式 IPC 与事件通知子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于`os.md`内核对象‑条目模型构建，不设计独立的 IPC 系统调用；全部能力依托对象、条目、ACL 权限实现。
> 2. Ring1/Ring2 保持闲置；内核对象管理器为 IPC 的核心载体。
> 3. 兼容原有：服务中断通知、虚拟机事件通知、普通进程事件等待全部复用这套 IPC 事件机制。
> 4. 支持：消息队列、事件唤醒与信号、条件等待、共享内存、信号量、命名管道、域套接字、定期镜像同步、双向映射、对象句柄传递；统一由 IO 流对象 / 纯控制对象条目承载，不引入独立内核 IPC 子系统，并对外兼容其他系统（POSIX / Sys‑V / Windows）的 IPC 语义。

## 术语

- **等待读条目**：条目支持阻塞语义，进程读操作时若无数据则进程休眠；有数据写入后自动唤醒等待的进程。
- **句柄传递**：在消息载荷中携带内核对象句柄，接收方进程可获得该对象的访问句柄，内核自动增加对象引用计数。
- **事件信号**：轻量无数据通知，仅做唤醒，不携带消息体。
- **消息载荷**：结构化二进制数据，写入条目；最大长度受内核配置限制。

## 1 总体约束

1. IPC 载体分为两类对象：
   1. IO 流对象：用于字节流式 IPC，可映射到 VFS 虚拟文件，供 Linux 兼容程序使用。
   2. 纯控制对象：用于消息队列、事件信号，**无 VFS 映射，仅原生对象通路访问**。
2. 每个 IPC 对象内部包含 IPC 专用条目；条目同样遵循独立 ACL 权限：读权限、写权限、`CREATE_ENTRY`、`DELETE_ENTRY`。
3. 进程对条目执行`ObjectReadEntry()`，若为等待可读条目，无消息时内核将进程阻塞休眠；当其它进程执行`ObjectWriteEntry()`写入消息，休眠进程被唤醒返回数据。
4. 支持在消息内传递对象句柄；内核做权限校验、引用计数增减，防止句柄泄露与越权传递。
5. 内核对象管理器负责 IPC 对象的生命周期、引用计数；当所有句柄关闭，自动回收 IPC 对象资源。
6. 用户态服务、虚拟机管理程序、普通应用全部使用同一套 IPC 条目机制。

## 2 IPC 对象条目布局

### 类型 A：消息队列纯控制对象，路径示例 `/object/ipc/mq/<mq_id>`（纯控制对象，无 VFS）

表格

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `msg_queue` | 等待可读 / 只写 | 消息队列主体；写写入消息，读阻塞取出消息；FIFO 先进先出 |
| `config` | 可读可写 | 队列配置：最大消息数量、单条消息最大字节、是否允许句柄传递 |
| `stat` | 只读 | 状态条目：当前排队消息数、等待读的进程数量、总入队 / 出队计数 |

> 
> 创建消息队列：打开父对象`OpenObject("/object/ipc/mq")`，调用`ObjectCreateEntry()`创建子对象，需要父对象`CREATE_ENTRY`权限。

### 类型 B：事件信号对象（轻量通知，无消息体），路径 `/object/ipc/event/<ev_id>`（纯控制对象）

表格

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `signal` | 等待可读 / 只写 | 写：发送事件信号（仅计数，不带数据）；读：阻塞等待信号，读取后自动清零 |
| `config` | 可读可写 | 配置：是否支持多生产者、多等待者模式 |
| `stat` | 只读 | 待处理信号计数、等待进程数 |

> 
> 使用场景：硬件服务中断事件、虚拟机事件通知，例如`hardware_svc.md`硬件对象的 event 条目、`vm.md`虚拟机对象 event 条目底层就是复用该事件条目能力。

### 类型 C：字节流 IO‑IPC 对象（可 VFS 映射，兼容 Linux 程序）

路径示例：`/system/dev/ipc/streamxxx`，属于 IO 流对象，会在 VFS 生成虚拟文件。

- 条目`stream_data`可读可写；行为类似管道；支持`open/read/write`POSIX 接口；
- 供 Linux ELF 程序使用；底层内核实现和原生对象 API 共享同一套缓冲区。
### 与其他系统 IPC 方式的对应承载（总览）

| 其他系统方式 | 源系统 | 本系统承载 |
| --- | --- | --- |
| 管道 pipe / 匿名管道 | POSIX | 类型 C 字节流 IO‑IPC 对象（已有） |
| 命名管道 FIFO | POSIX / Windows | 类型 F 命名管道对象（IO 流 + VFS） |
| 信号 signal / kill | POSIX | 进程信号表 + 事件条目（信号 = 带编号的事件） |
| 共享内存 shm / mmap | Sys‑V / POSIX | 类型 D 共享内存对象（VFS 映射） |
| 信号量 semaphore | Sys‑V / POSIX | 类型 E 信号量对象（计数 + 等待队列） |
| 消息队列 mq | Sys‑V / POSIX | 类型 A 消息队列对象（已有） |
| Unix 域套接字 socketpair | POSIX | 类型 G 域套接字对象 |
| 事件 EVENT | Windows | 类型 B 事件信号对象（已有） |
| 邮件槽 mailslot | Windows | 类型 A 消息队列承载（单向广播） |
| 剪贴板 / COM / RPC | Windows | 对象调用 + 消息队列承载（对象式 RPC） || 定期镜像同步（复制式，本系统增强） | — | 类型 H 镜像同步对象（副本 + 版本号 + 冲突规则） |
| 双向映射（本系统增强） | — | 类型 I 双向映射对象（双区交叉映射，实时共享） |

### 类型 D：共享内存对象，路径示例 `/object/ipc/shm/<shm_id>`（纯控制对象 + VFS 映射）

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `data` | 可读可写 | 共享内存映射区主体；进程经 `mmap` 映射后直接读写，写后对其他映射进程可见 |
| `config` | 可读可写 | 映射大小、是否可扩展、VFS 映射开关 |
| `stat` | 只读 | 当前映射进程数、引用计数 |

> 
> VFS 映射 `/system/dev/ipc/shm/<shm_id>`；供 Linux 兼容层 `shm_open` / `mmap` 翻译，Windows 内存映射文件（Memory‑Mapped File）语义兼容。

### 类型 E：信号量对象，路径示例 `/object/ipc/sem/<sem_id>`（纯控制对象）

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `wait` | 等待可读 | 原子减一；计数为 0 时阻塞，被 `post` 唤醒后返回 |
| `post` | 只写 | 原子加一；唤醒一个等待进程 |
| `config` | 可读可写 | 初始计数、最大值、是否支持多进程互斥 |
| `stat` | 只读 | 当前计数、等待进程数 |

> 
> 供 POSIX `sem_wait` / `sem_post`、Sys‑V `semop` 翻译。

### 类型 F：命名管道对象，路径示例 `/object/ipc/fifo/<name>`（IO 流对象，VFS 映射）

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `data` | 可读可写 | 字节流主体；阻塞语义：写满阻塞写，读空阻塞读 |
| `config` | 可读可写 | 缓冲区大小、阻塞 / 非阻塞模式 |
| `stat` | 只读 | 缓冲占用、读写进程数 |

> 
> VFS 映射 `/system/dev/ipc/fifo/<name>`；供 POSIX `mkfifo` / `open` / `read` / `write` 翻译，Windows 命名管道语义兼容。

### 类型 G：域套接字对象，路径示例 `/object/ipc/sock/<name>`（IO 流对象，VFS 映射）

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `listen` | 等待可读 | 监听连接请求，接受后建立对端 |
| `data` | 可读可写 | 流（SOCK_STREAM）或报文（SOCK_DGRAM）数据 |
| `peer` | 只读 | 对端信息（进程 / 对象路径） |
| `config` | 可读可写 | 套接字类型、缓冲区、是否允许句柄传递 |
| `stat` | 只读 | 连接状态、缓冲占用 |

> 
> VFS 映射 `/system/dev/ipc/sock/<name>`；供 POSIX `socket` / `bind` / `listen` / `accept` / `connect` / `send` / `recv` 翻译。

### 进程信号（signal）机制

1. 每个进程一张信号表，路径 `/object/ipc/sig/<pid>`；条目 `signal`：**写 = 向该进程发送信号（携带信号号），读 = 阻塞等待信号**。
2. 支持 POSIX `kill` / `signal` / `sigaction` 翻译；服务中断、虚拟机事件也可经该机制投递。
3. 与类型 B 事件的区别：信号**绑定目标进程并携带信号号**，事件是匿名广播通知；信号未处理时内核按默认处置（终止 / 忽略 / 自定义处理函数）。
### 类型 H：定期镜像同步对象，路径示例 `/object/ipc/mirror/<mirror_id>`（复制式同步，非实时）

参与方各自持有本地副本，按触发 / 周期双向覆盖同步；同步携带版本号，按冲突规则解决双方同时修改。

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `data` | 可读可写 | 本地副本区；参与方只读写自己的副本 |
| `sync` | 只写 | 触发一次双向同步：推送本地副本并拉取对端副本，按冲突规则合并 |
| `config` | 可读可写 | 同步模式（手动 / 周期）、冲突规则（newest_wins / region_lock / manual）、参与方列表 |
| `stat` | 只读 | 各参与方版本号、上次同步时间、待同步变更数 |

冲突规则：

1. `newest_wins`（默认）：按版本号 / 时间戳，较新者覆盖较旧者；同版本按参与方优先级。
2. `region_lock`：按区域加锁，仅持锁方可在该区域修改；同步时跳过被锁区域，防止互相覆盖。
3. `manual`：同步遇到双方都修改的区域，不自动覆盖，返回冲突列表由调用方决定。

> 
> 语义对应分布式副本同步 / 网盘同步 / 存档云同步；无 POSIX 直接对应，供原生应用使用。

### 类型 I：双向映射对象，路径示例 `/object/ipc/dual/<dual_id>`（实时共享，方向隔离）

开两个方向映射区：`a_to_b` 由 A 写、B 实时可见；`b_to_a` 由 B 写、A 实时可见。映射共享同一物理内存，写侧修改直接生效于读侧，零拷贝实时可见。

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `a_to_b` | 可读可写 | A → B 方向数据区：A 写，B 实时可见 |
| `b_to_a` | 可读可写 | B → A 方向数据区：B 写，A 实时可见 |
| `config` | 可读可写 | 各方向映射大小、是否允许句柄传递 |
| `stat` | 只读 | 各方向映射进程数、最近写入时间 |

> 
> 与类型 D 的区别：类型 D 是单块共享区多进程共用；类型 I 是两块区按方向交叉映射，**方向隔离**，各方向可独立配置权限，适合双向但需区分数据流向的场景。VFS 映射 `/system/dev/ipc/dual/<dual_id>/a2b` 与 `/b2a`。

## 3 核心操作流程示例

### 示例 1：消息队列发送‑接收

1. 创建方打开父对象`/object/ipc/mq`，`ObjectCreateEntry`创建消息队列子对象，配置`config`条目参数。
2. 发送进程：获取队列句柄，`ObjectWriteEntry`向`msg_queue`写入结构化消息（可选携带对象句柄）。
3. 接收进程：对`msg_queue`执行`ObjectReadEntry()`，队列无消息则进程阻塞休眠。
4. 消息入队后内核唤醒接收进程，调用返回消息载荷；内核自动处理句柄引用计数。

### 示例 2：事件等待（服务中断场景，参考 hardware_svc.md）

1. 硬件对象内置一条等待可读的`event`事件条目（底层复用 ipc 事件机制）。
2. 用户态服务对该条目执行`ObjectReadEntry()`，无中断事件则阻塞。
3. 内核 Ring0 收到硬件中断，向该`event`条目写入事件信号，唤醒服务进程。
4. 服务读取事件数据，继续处理硬件逻辑。

## 4 句柄传递规则（重要安全规则）

1. 消息载荷中可以携带对象句柄；写入时内核校验：当前进程是否拥有该句柄访问权限。
2. 发送时内核**增加对象引用计数**；接收方读取消息后得到有效的句柄副本。
3. 如果接收进程没有对应对象的 ACL 访问权限，句柄传递直接拒绝，返回错误；不会把权限私自授予接收进程。
4. 进程关闭句柄，内核减少引用计数；全部句柄关闭后对象销毁回收。

## 5 生命周期与资源回收

1. IPC 对象引用计数由内核对象管理器统一维护。
2. 主动销毁：拥有`DELETE_ENTRY`权限的进程删除 IPC 对象条目 / 子对象；所有等待该对象的进程全部被唤醒，返回对象已销毁错误。
3. 进程异常崩溃：内核自动关闭该进程持有的全部 IPC 句柄，减少引用计数；不影响其它进程打开的 IPC 对象。
4. 当引用计数降为 0，内核释放缓冲区、条目资源，彻底销毁 IPC 对象。

## 6 和其它文档对接关系

1. **对接 os.md**
   - IPC 全部构建在对象‑条目模型；复用 ACL：读、写、`CREATE_ENTRY`、`DELETE_ENTRY`；分为 IO 流对象、纯控制对象；两套访问入口（原生对象 API / VFS）。
2. **对接 hardware_svc.md**
   - 硬件对象的中断 event 条目，底层复用本 IPC 事件对象能力，服务不需要额外特殊接口。
3. **对接 vm.md**
   - 虚拟机对象`event`事件条目复用本 IPC 事件机制，虚拟机管理程序等待虚拟机事件。
4. **对接 Linux.md**
   - 字节流 IPC IO 流对象映射 VFS 虚拟文件，供 POSIX `read/write`调用，模拟管道行为，用于 Linux 兼容层。
   - 共享内存 / 信号量 / 命名管道 / 域套接字 / 信号对象经「VFS 映射 + 对象 API」双通路，翻译 POSIX `shm_open` / `sem_wait` / `mkfifo` / `socket` / `kill` 等调用（详见 Linux 兼容层 POSIX 扩展接口）。
5. **对接 storage.md**
   - 共享内存对象可选的持久化 / 文件映射由存储子系统承载；文件锁语义经对象条目级互斥实现。

## 7 设计取舍

1. 不单独实现传统 Sys‑V IPC、POSIX IPC 的独立内核逻辑；全部复用对象条目统一承载，对外兼容其语义（共享内存 / 信号量 / 消息队列 / 命名管道 / 域套接字 / 信号），由 Linux 兼容层翻译。
2. 复杂 IPC 优先使用原生对象通路；VFS 映射仅作为兼容层给 Linux 程序。
3. 阻塞 / 唤醒逻辑统一由内核对象管理器事件等待框架处理。

## 8 IPC `.api` 契约文件（9 个契约，与 /system/ipc/ 子目录一一对应）

IPC 对外接口由 **9 个契约文件**声明，存放于 `/system/api/ipc/`，由桥接层加载解析；每个契约对应 `/system/ipc/` 下一个子目录的宿主可执行文件（exec 一一对应）。**本文档内置全部契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

### 8.1 ipc_core.api（IPC 核心与事件信号）

```text
# ipc_core.api IPC 核心与事件信号契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = ipc.core
# 转换代码运行环境
env = python
# 所属可执行文件命名空间（IPC 核心：对象-条目承载与事件等待框架）
exec = /system/ipc/core
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 200次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/ipc/core ipc_core

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
ipc_core: event_stat { } /internal/ipc/event/stat

# ---- 可写控制条目：转换代码为Python，实现事件信号与等待 ----
ipc_core: event_signal {
    return self.event_signal(ev_id)
} /internal/ipc/event/signal
ipc_core: event_wait {
    return self.event_wait(ev_id)
} /internal/ipc/event/wait

# ---- 对外可监听标准事件（IPC 对象生命周期） ----
EVENT:
    ipc_created(kind: str) ｜ /internal/event/ipc_create
    ipc_destroyed(kind: str) ｜ /internal/event/ipc_destroy

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    event_signal: 1000次/秒
```

### 8.2 ipc_mq.api（消息队列）

```text
# ipc_mq.api 消息队列契约
version = 1.0
target = ipc.mq
env = python
exec = /system/ipc/mq
internal_loc = map + symbol + register
min_os_version = 1.0
global_acl = user_foreground
global_limit = 200次/秒

object /system/api/ipc/mq ipc_mq

ipc_mq: mq_stat { } /internal/ipc/mq/stat

ipc_mq: mq_create {
    return self.mq_create(cfg)
} /internal/ipc/mq/create
ipc_mq: mq_send {
    return self.mq_send(mq_id, msg)
} /internal/ipc/mq/send
ipc_mq: mq_recv {
    return self.mq_recv(mq_id)
} /internal/ipc/mq/recv

LIMIT_OVERRIDE:
    mq_send: 1000次/秒

ACL_OVERRIDE:
    mq_create: user_foreground
```

### 8.3 ipc_shm.api（共享内存）

```text
# ipc_shm.api 共享内存契约
version = 1.0
target = ipc.shm
env = python
exec = /system/ipc/shm
internal_loc = map + symbol + register
min_os_version = 1.0
global_acl = user_foreground
global_limit = 200次/秒

object /system/api/ipc/shm ipc_shm

ipc_shm: shm_stat { } /internal/ipc/shm/stat

ipc_shm: shm_create {
    return self.shm_create(cfg)
} /internal/ipc/shm/create
ipc_shm: shm_map {
    return self.shm_map(shm_id, addr_hint)
} /internal/ipc/shm/map
ipc_shm: shm_unmap {
    return self.shm_unmap(shm_id)
} /internal/ipc/shm/unmap
ipc_shm: shm_destroy {
    return self.shm_destroy(shm_id)
} /internal/ipc/shm/destroy

EVENT:
    shm_created(shm_id: str) ｜ /internal/event/ipc_shm_create
    shm_destroyed(shm_id: str) ｜ /internal/event/ipc_shm_destroy
```

### 8.4 ipc_sem.api（信号量）

```text
# ipc_sem.api 信号量契约
version = 1.0
target = ipc.sem
env = python
exec = /system/ipc/sem
internal_loc = map + symbol + register
min_os_version = 1.0
global_acl = user_foreground
global_limit = 200次/秒

object /system/api/ipc/sem ipc_sem

ipc_sem: sem_stat { } /internal/ipc/sem/stat

ipc_sem: sem_create {
    return self.sem_create(cfg)
} /internal/ipc/sem/create
ipc_sem: sem_wait {
    return self.sem_wait(sem_id)
} /internal/ipc/sem/wait
ipc_sem: sem_post {
    return self.sem_post(sem_id)
} /internal/ipc/sem/post
ipc_sem: sem_destroy {
    return self.sem_destroy(sem_id)
} /internal/ipc/sem/destroy

EVENT:
    sem_created(sem_id: str) ｜ /internal/event/ipc_sem_create
    sem_destroyed(sem_id: str) ｜ /internal/event/ipc_sem_destroy

LIMIT_OVERRIDE:
    sem_post: 1000次/秒
```

### 8.5 ipc_fifo.api（命名管道）

```text
# ipc_fifo.api 命名管道契约
version = 1.0
target = ipc.fifo
env = python
exec = /system/ipc/fifo
internal_loc = map + symbol + register
min_os_version = 1.0
global_acl = user_foreground
global_limit = 200次/秒

object /system/api/ipc/fifo ipc_fifo

ipc_fifo: fifo_open {
    return self.fifo_open(name, mode)
} /internal/ipc/fifo/open
ipc_fifo: fifo_write {
    return self.fifo_write(fd, data)
} /internal/ipc/fifo/write
ipc_fifo: fifo_read {
    return self.fifo_read(fd)
} /internal/ipc/fifo/read
```

### 8.6 ipc_sock.api（域套接字）

```text
# ipc_sock.api 域套接字契约
version = 1.0
target = ipc.sock
env = python
exec = /system/ipc/sock
internal_loc = map + symbol + register
min_os_version = 1.0
global_acl = user_foreground
global_limit = 200次/秒

object /system/api/ipc/sock ipc_sock

ipc_sock: sock_stat { } /internal/ipc/sock/stat

ipc_sock: sock_create {
    return self.sock_create(cfg)
} /internal/ipc/sock/create
ipc_sock: sock_connect {
    return self.sock_connect(name)
} /internal/ipc/sock/connect
ipc_sock: sock_send {
    return self.sock_send(fd, data)
} /internal/ipc/sock/send
ipc_sock: sock_recv {
    return self.sock_recv(fd)
} /internal/ipc/sock/recv
ipc_sock: sock_close {
    return self.sock_close(fd)
} /internal/ipc/sock/close

EVENT:
    sock_connected(name: str) ｜ /internal/event/ipc_sock_conn
    sock_closed(name: str) ｜ /internal/event/ipc_sock_close

LIMIT_OVERRIDE:
    sock_send: 1000次/秒
```

### 8.7 ipc_sig.api（进程信号）

```text
# ipc_sig.api 进程信号契约
version = 1.0
target = ipc.sig
env = python
exec = /system/ipc/sig
internal_loc = map + symbol + register
min_os_version = 1.0
global_acl = user_foreground
global_limit = 200次/秒

object /system/api/ipc/sig ipc_sig

ipc_sig: sig_stat { } /internal/ipc/sig/stat

ipc_sig: sig_send {
    return self.sig_send(pid, signo)
} /internal/ipc/sig/send
ipc_sig: sig_wait {
    return self.sig_wait(pid)
} /internal/ipc/sig/wait

EVENT:
    sig_received(pid: int, signo: int) ｜ /internal/event/ipc_sig

LIMIT_OVERRIDE:
    sig_send: 1000次/秒

ACL_OVERRIDE:
    sig_send: aux_system
```

### 8.8 ipc_mirror.api（定期镜像同步）

```text
# ipc_mirror.api 定期镜像同步契约
version = 1.0
target = ipc.mirror
env = python
exec = /system/ipc/mirror
internal_loc = map + symbol + register
min_os_version = 1.0
global_acl = user_foreground
global_limit = 200次/秒

object /system/api/ipc/mirror ipc_mirror

ipc_mirror: mirror_stat { } /internal/ipc/mirror/stat

ipc_mirror: mirror_create {
    return self.mirror_create(cfg)
} /internal/ipc/mirror/create
ipc_mirror: mirror_sync {
    return self.mirror_sync(mirror_id)
} /internal/ipc/mirror/sync
ipc_mirror: mirror_destroy {
    return self.mirror_destroy(mirror_id)
} /internal/ipc/mirror/destroy

EVENT:
    mirror_synced(mirror_id: str, conflict: int) ｜ /internal/event/ipc_mirror_sync
```

### 8.9 ipc_dual.api（双向映射）

```text
# ipc_dual.api 双向映射契约
version = 1.0
target = ipc.dual
env = python
exec = /system/ipc/dual
internal_loc = map + symbol + register
min_os_version = 1.0
global_acl = user_foreground
global_limit = 200次/秒

object /system/api/ipc/dual ipc_dual

ipc_dual: dual_stat { } /internal/ipc/dual/stat

ipc_dual: dual_create {
    return self.dual_create(cfg)
} /internal/ipc/dual/create
ipc_dual: dual_map {
    return self.dual_map(dual_id, dir)
} /internal/ipc/dual/map
ipc_dual: dual_unmap {
    return self.dual_unmap(dual_id, dir)
} /internal/ipc/dual/unmap
ipc_dual: dual_destroy {
    return self.dual_destroy(dual_id)
} /internal/ipc/dual/destroy

EVENT:
    dual_created(dual_id: str) ｜ /internal/event/ipc_dual_create
    dual_destroyed(dual_id: str) ｜ /internal/event/ipc_dual_destroy
```

契约要点：

1. IPC 对外接口按 `/system/ipc/` 下 9 个子目录拆分为 9 个契约：`core`（事件与等待框架）、`mq`、`shm`、`sem`、`fifo`、`sock`、`sig`、`mirror`、`dual`；每个契约声明各自的宿主可执行文件（exec），与目录一一对应，存放于 `/system/api/ipc/`。
2. 各契约条目复用对象条目 ACL 管控；读、写、`CREATE_ENTRY`、`DELETE_ENTRY` 权限一致。
3. 事件按归属分散在各契约：IPC 对象生命周期事件归 `core`，共享内存 / 信号量 / 域套接字 / 进程信号 / 镜像 / 双向映射各自携带自身事件。
4. `sig_send` 跨进程投递信号属敏感操作，ACL 覆写为 `aux_system`；`mq_send` / `event_signal` / `sem_post` / `sig_send` / `sock_send` 为高频调用，独立限流放宽。
