# external_dev.md 外部设备标准（I/O 接口外部设备接入规范，x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；外部设备统一抽象为**IO 流对象**，可挂载 VFS 生成虚拟节点，同时支持原生对象通路访问。
> 2. Ring1/Ring2 闲置；外部设备对象受 `acl.md` 两级 ACL 管控；服务绑定复用 `hardware_svc.md` 用户态服务规范；本系统驱动分内核扩展文件与用户态服务两类，按「性能高且错误率低」判据归属。
> 3. **条目不在此文档硬编码**：外部设备子系统对外条目的集合、命名、权限由 `external_dev.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义外部设备接入的行为语义，契约全文见第 7 节。
> 4. 覆盖范围：通过 I/O 接口连接的外部设备——USB、串口（UART）、PS/2、显示输出、通用输入设备等；存储类设备见 `fs.md` 块设备对象，音频设备见 `audio.md`。
> 5. 依赖：`os.md`、`acl.md`、`hardware_svc.md`、`memory.md`、`ipc.md`、`boot.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 外部设备 | 通过 I/O 接口与主机连接、可插拔的外设：USB、串口、PS/2 键鼠、显示器等 |
| I/O 接口 | 设备与主机之间的连接总线/端口：USB 控制器、UART、PS/2、显示链路等 |
| 设备对象 | 每个外部设备对应一个 IO 流对象 `/object/dev/<dev_id>`，承载设备状态与数据通路 |
| 热插拔 | 设备在系统运行期间接入/移除，系统自动枚举与回收 |
| 设备枚举 | 系统发现总线上的设备、读取其描述信息并创建对应设备对象的过程 |

## 1 外部设备分类

1. **USB 设备**：键盘、鼠标、U 盘、音频、摄像头、打印设备等；经 USB 控制器挂接，支持热插拔。
2. **串口设备（UART）**：调试终端、工业控制设备、嵌入式外设；面向字节流通信。
3. **PS/2 设备**：键盘、鼠标；传统接口，无热插拔或热插拔受限。
4. **显示输出设备**：经 DisplayPort / HDMI 等链路连接显示器；对接 `ui.md` 显示设备对象。
5. **其它通用外设**：打印机、扫描仪等；经对应 I/O 接口接入。

## 2 设备接入与枚举

1. 内核基座初始化阶段完成 PCIe 总线枚举（`boot.md` 阶段 3），注册各 I/O 控制器（USB 控制器、UART 控制器等）。
2. **接入检测**：I/O 控制器硬件中断或轮询检测到设备接入/移除，上报内核。
3. **设备枚举**：内核读取设备描述信息（厂商 ID、产品 ID、设备类别、接口描述），在 `/object/dev` 命名空间下创建对应设备对象。
4. **设备对象创建**：设备对象为 IO 流对象，路径 `/object/dev/<dev_id>`；创建时填充设备类别、描述信息、接口类型、状态（未绑定/已绑定）。
5. **热插拔移除**：检测到设备移除，内核通知已绑定服务并触发回收流程（见第 6 节）。

## 3 设备对象模型

设备对象 `/object/dev/<dev_id>` 内部持有以下条目（**布局由 `external_dev.api` 契约声明**）：

1. `info`：只读，设备类别、厂商/产品 ID、序列号、接口类型、链路状态。
2. `data`：可读可写，面向字节流/数据报的设备数据通路（串口、USB 数据端点）。
3. `ctl`：可读可写，设备控制命令：配置接口参数、复位、收发控制。
4. `event`：等待可读，设备事件队列：接入、移除、数据到达、错误。
5. `state`：只读，设备状态：未绑定 / 已绑定 / 忙 / 错误。

## 4 设备服务绑定（复用 hardware_svc.md）

1. 外部设备服务为 Ring3 用户态进程，`PROC_TYPE_SERVICE` 标记；通过 `hardware_svc.md` 注册流程向 `/object/service_mgr` 创建注册条目。
2. **绑定**：服务向设备对象 `ctl` 条目写入绑定请求结构体；内核校验服务身份、设备空闲状态、ACL 权限后完成绑定，`state` 置为已绑定，记录 `DevID ↔ 服务PID`。
3. **数据通路**：绑定后服务通过 `data` 条目收发设备数据；数据链路复用 `memory.md` DMA 缓冲与 `ipc.md` 等待读机制。
4. **多设备**：一个服务可绑定多个同类设备；每个设备独立对象，独立绑定关系。
5. 同一设备同一时刻仅允许单一服务绑定（独占）。

## 5 数据传输与中断

1. 字节流设备（串口、USB 数据端点）：服务读写 `data` 条目；写入即发送，等待可读接收。
2. 数据量较大的外设（存储类、视频类）：数据经 DMA 缓冲直接搬运，控制命令走条目，数据载荷走裸缓冲区（`os.md` 性能取舍）。
3. 设备中断：内核将设备中断事件写入设备对象 `event` 条目，唤醒等待的服务进程（`ipc.md` 事件机制）。
4. 页表与缓存属性、DMA 缓冲区分配遵循 `memory.md`；设备 DMA 受 IOMMU 管控（`acl.md`/`hardware_svc.md`）。

## 6 设备移除与资源回收

1. **热插拔移除**：内核检测到设备移除，向 `event` 条目写入移除事件，通知已绑定服务。
2. 服务可主动解绑（向 `ctl` 写解绑命令）或随进程消亡自动解绑（`hardware_svc.md` 第 5 节）。
3. 解绑流程：撤销 MMIO/数据映射、释放 DMA 缓冲、清除 IOMMU 分组、设备状态置回未绑定、删除服务注册条目。
4. 设备对象在无引用且移除后由对象管理器回收销毁。

## 7 外部设备子系统 `.api` 契约文件（external_dev.api 全文）

外部设备子系统对外接口统一由以下契约文件声明，文件存放于 `/system/api/external_dev.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# external_dev.api 外部设备子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = external_dev
env = python
exec = /system/devmgr/manager
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 1000次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/external_dev external_dev

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
external_dev: list_devices { } /internal/dev/list
external_dev: state { } /internal/dev/state
external_dev: data { } /internal/dev/data

# ---- 可写控制条目：转换代码为Python，实现设备控制 ----
external_dev: bind {
    if not self.check_service(pid):
        raise PermissionError("非服务进程")
    self.bind(dev_id, pid)
} /internal/dev/ctl/bind
external_dev: unbind { self.unbind(dev_id) } /internal/dev/ctl/unbind
external_dev: send {
    self.send(dev_id, payload)
} /internal/dev/ctl/send

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    dev_attached(dev_id: u64, dev_type: str) ｜ /internal/event/dev_attach
    dev_detached(dev_id: u64) ｜ /internal/event/dev_detach
    dev_data_ready(dev_id: u64) ｜ /internal/event/dev_data_ready

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    send: 5000次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    bind: aux_system
    unbind: aux_system
```

契约要点：

1. 外部程序仅能访问 `/system/api/external_dev` 标准虚拟对象，禁止直连设备对象内部条目。
2. `bind`/`unbind` 接口经 Python 转换代码校验调用方服务身份，非服务进程直接拒绝。
3. 事件仅封装契约声明的标准参数；设备接入/移除事件供系统管理工具与服务监听。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 8 与各子系统对接

1. **os.md**：外部设备为 IO 流对象，支持原生对象通路与 VFS 两套访问。
2. **acl.md**：设备对象与条目受两级 ACL；绑定/配置需授权。
3. **hardware_svc.md**：设备服务绑定、解绑、崩溃回收完全复用用户态服务规范。
4. **memory.md**：数据通路 DMA 缓冲、页表映射、缓存属性遵循内存规范。
5. **ipc.md**：设备事件、数据到达等待复用事件/等待读机制。
6. **boot.md**：基座阶段枚举 I/O 控制器，建立设备枚举基础。
7. **fs.md**：设备对象可挂载 VFS 生成虚拟节点（如 `/system/dev`）。
8. **.api.md**：外部设备对外接口经 `external_dev.api` 契约与桥接层。
9. **rec.md**：麦克风等音频外设接入，供录音机（rec.md，系统唯一麦克风使用方）采集。
10. **cam.md**：摄像头外设接入，供系统唯一摄像头应用（cam.md）拍照 / 录像。

## 9 设计取舍

1. 所有外部设备统一抽象为 IO 流对象，与内核对象模型一致，无独立设备接口体系。
2. 设备服务复用用户态服务规范，崩溃隔离，不整机 panic。
3. 数据载荷走裸缓冲区、控制命令走条目，兼顾吞吐与可管理性。
4. 热插拔由内核自动枚举与回收，服务无感知接入细节。
