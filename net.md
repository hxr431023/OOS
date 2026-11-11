# net.md 网络子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 基于 `os.md` 对象‑条目模型；网络硬件服务运行在内核 Ring0 可加载模块；协议栈主体同样位于内核。
> 2. Ring1/Ring2 保持闲置；复用 `acl.md`、`memory.md`、`ipc.md`、`fs.md`、`process.md`。
> 3. 用户态不直接操作网卡硬件；套接字抽象为**IO 流内核对象**，可挂载 VFS，供 Linux 兼容层使用。
> 4. 不新增独立网络专用系统调用；全部操作通过对象条目完成；Linux 兼容层将 POSIX socket 调用翻译为对象条目操作。

## 术语

- **网卡硬件对象**：IO 流对象，代表物理网卡，路径`/object/net/dev/<nic_id>`；
- **套接字对象**：IO 流对象，路径`/object/net/sock/<sock_id>`，可映射 VFS 为文件描述符；
- 协议栈：内核内部实现，ARP、IP、ICMP、TCP、UDP；
- 接收队列、发送队列：套接字对象内部条目；
- tap 虚拟网卡：纯软件虚拟网卡对象，可给虚拟机使用，对接`vm.md`虚拟机子系统。

## 1 总体约束

1. 物理网卡硬件服务、TCP/UDP/IP 协议栈运行在内核 Ring0 模块；服务 bug 会导致内核 panic。
2. 用户态进程不能直接操作网卡 MMIO / 寄存器；全部网络收发通过套接字对象条目完成。
3. 套接字属于 IO 流对象，可以映射到 VFS，在 Linux 兼容层中对应 socket 返回的 fd；支持`read/write`，同时支持原生对象 API 读写条目。
4. 支持物理网卡 + tap 虚拟网卡；tap 设备可直通给虚拟机，对接`vm.md`。
5. 套接字生命周期由内核对象管理器引用计数管理；进程关闭句柄、进程崩溃自动关闭套接字，内核执行四次挥手 / 资源回收。

## 2 网卡硬件对象 `/object/net/dev/<nic_id>`（IO 流对象）

表格

| 条目名称 | 权限 | 说明 |
| --- | --- | --- |
| `info` | 只读 | 网卡 MAC、MTU、链路状态、IP 配置信息 |
| `config` | 可读可写 | 配置 IP 地址、子网掩码、网关、DNS；需要管理员 ACL 权限 |
| `stat` | 只读 | 收发数据包计数、错误统计、丢包统计 |

> 
> 内核 PCI 枚举完成后，网卡服务模块创建网卡硬件对象。普通应用只拥有`ENTRY_READ`；网络管理进程拥有`ENTRY_WRITE`修改网络配置。

## 3 套接字对象 `/object/net/sock/<sock_id>`（IO 流对象）

> 
> 创建套接字：打开父对象`OpenObject("/object/net/sock")`，调用`ObjectCreateEntry()`创建套接字子对象，传入协议类型（TCP/UDP/RAW）；需要父对象`CREATE_ENTRY`权限。

表格

| 条目名称 | 权限 | 说明 |
| --- | --- | --- |
| `sock_ctl` | 可读可写 | 套接字控制条目：bind、listen、connect、accept、shutdown、setsockopt 参数写入 |
| `rx_queue` | 等待可读 | 接收数据包队列；读操作阻塞等待数据到达 |
| `tx_queue` | 只写 | 发送数据包队列；写入数据交由内核协议栈发送 |
| `sock_status` | 只读 | 套接字状态：closed/listening/connected；本地 / 远端端口、IP |

### 套接字基础工作流程

1. 用户态调用`ObjectCreateEntry()`，指定协议，生成套接字对象。
2. 向`sock_ctl`写入 bind/connect/listen 等命令结构体。
3. 发送：把数据写入`tx_queue`条目；内核协议栈取出数据包，经由网卡硬件对象发出。
4. 接收：对`rx_queue`做等待读；内核收到网络报文，写入`rx_queue`，唤醒阻塞的用户态进程。
5. shutdown/close：向`sock_ctl`写入关闭命令；或者直接`CloseHandle`；内核执行协议层面关闭，释放连接资源。

> 
> TCP 场景：accept 会在内部新建一个子套接字对象，代表新的已建立连接。

## 4 TAP 虚拟网卡对象（IO 流对象）

路径：`/object/net/tap/<tap_id>`

1. tap 虚拟网卡不绑定物理硬件；数据包条目：`tx`、`rx`。
2. 用户态或虚拟机可以读写 tap 条目；写入的数据包交给内核协议栈；内核要发往 tap 的数据包写入 rx 条目。
3. **对接 vm.md 虚拟机**：虚拟机的虚拟网卡后端直接绑定 tap 对象；客户机收发网络报文全部流过 tap 条目。

## 5 权限规则（acl.md 复用）

1. 创建套接字：需要`/object/net/sock`父对象`CREATE_ENTRY`权限。
2. 修改网卡 IP、MTU：需要网卡对象`config`条目`ENTRY_WRITE`权限。
3. 普通进程：可以创建普通 TCP/UDP 套接字；禁止修改网卡硬件配置；RAW 原始套接字需要额外 ACL 授权。
4. 套接字对象映射 VFS 之后，r/w 权限对应底层条目 ACL；`CREATE_ENTRY`依旧只能原生对象 API。

## 6 资源回收

1. 用户态主动关闭：`CloseHandle`关闭套接字句柄；引用计数递减；引用计数归零，内核关闭连接，释放缓冲区，销毁套接字对象。
2. 进程崩溃：内核自动关闭该进程全部套接字句柄；TCP 执行断开；释放 rx/tx 缓冲区。
3. tap 虚拟网卡：所有句柄关闭后自动销毁 tap 对象。

## 7 和其它文档对接

1. **os.md**：网卡、套接字、tap 全部为 IO 流对象；支持原生对象 API + VFS 两套通路。
2. **acl.md**：所有对象条目遵循两级 ACL；网络配置修改需要写权限。
3. **memory.md**：内核协议栈分配网络数据包内存；DMA 由网卡服务使用内存子系统接口。
4. **ipc.md**：`rx_queue`底层复用 IPC 等待‑读机制，无数据时阻塞进程。
5. **process.md**：进程句柄表保存套接字对象句柄；进程崩溃自动回收套接字资源。
6. **fs.md**：套接字对象映射 VFS，成为 fd；dup/dup2 复制句柄，增加对象引用计数。
7. **vm.md**：tap 对象作为虚拟机虚拟网卡后端，虚拟机对象条目读写转发到 tap 条目。
8. **boot.md**：内核启动加载网卡服务模块，创建网卡硬件对象；init 进程做初始网络配置。
9. **Linux.md**：POSIX socket 系统调用全部翻译成对套接字对象条目的操作；模拟 socket、bind、listen、accept、send、recv 语义；支持 SO_* 套接字选项。
10. **map.md**：WiFi / 基站信号经网络子系统上报，供定位服务（map.md）网络定位使用。

## 8 套接字 `.api` 契约文件（net.socket.api 全文）

套接字对外接口统一由以下契约文件声明，文件存放于 `/system/api/net.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# net.api 网络子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = net
env = python
exec = /system/network/net_daemon
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 10000次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/net net

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
net: local_port { } /internal/net/local_port
net: remote_port { } /internal/net/remote_port
net: local_addr { } /internal/net/local_addr
net: remote_addr { } /internal/net/remote_addr
net: state { } /internal/net/state
net: rx_bytes { } /internal/net/rx_bytes
net: tx_bytes { } /internal/net/tx_bytes

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
net: connect { return self.connect(addr, port) } /internal/net/connect
net: listen { return self.listen(port) } /internal/net/listen
net: close { return self.close() } /internal/net/close
net: bind { return self.bind(addr, port) } /internal/net/bind

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    connected(peer: str) ｜ /internal/event/net_connected
    disconnected(reason: str) ｜ /internal/event/net_disconnected
    data_ready(rx_bytes: u64) ｜ /internal/event/net_data_ready

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    connect: 100次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    bind: aux_system
```

契约要点：

1. 契约对象内部路径 `/object/net/sock`（父对象），与本文第 3 节套接字对象一致；实例路径 `/object/net/sock/<sock_id>`。
2. 只读字段透传 `status`/`stats` 下套接字状态与统计；写字段为 `ctl` 下控制命令（connect / listen / close / bind）。
3. 全局限流 10000 次/秒（高频收发场景）；全部操作审计留痕（audit=true）。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 9 设计取舍

1. 网络协议栈放在内核，保证吞吐性能；不把 TCP/UDP 放到用户态。
2. 套接字复用 IO 流对象模型，不需要单独发明一套网络内核抽象。
3. tap 虚拟网卡作为普通 IO 流对象，统一物理网卡、虚拟网卡接口，虚拟机网络接入逻辑简洁。
4. VFS 映射保证 Linux 兼容层可以直接跑依赖 socket 的程序。