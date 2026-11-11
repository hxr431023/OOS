# hardware_svc.md 用户态第三方服务规范（x86‑64）

> 
> 前提：内核对象管理器为系统底层根基；Ring1/Ring2 闲置不使用；第三方服务全部运行于 Ring3 用户进程；第三方服务文件可存放于**系统任意路径**，不固定目录。本系统的驱动分两类：**性能高且出错率低**的放内核扩展文件（`kernel.md`），其余为用户态服务。
> x86‑64开启分页，所有CPU内存访问都经过MMU地址翻译；用户态进程**无法直接使用物理地址**访问MMIO，必须由内核建立虚拟地址映射。
> 重要：服务不使用一套独立自定义API，全部操作**完全基于内核对象、对象条目完成**，没有额外独立系统调用集合。

## 术语说明：物理MMIO

PCIe设备的BAR资源给出一段**物理地址区间**，读写该物理地址不会访问DRAM内存，而是访问硬件的寄存器。
用户态不能直接传入这个物理数字进行访问：CPU会把该数值当作虚拟地址查询当前进程页表；无映射则触发缺页异常，进程终止。
需要内核校验权限后，将物理MMIO页映射到服务进程的虚拟地址空间，服务只操作返回的**虚拟MMIO地址**；同时页表标记UC（不可缓存），避免CPU缓存干扰硬件寄存器读写。

## 1 总体约束

1. 本系统的驱动分两类：**性能收益大且出错率低**的硬件功能放内核扩展文件（`kernel.md`），其余一律为**用户态服务**。所有第三方硬件服务为 Ring3 用户进程，PCB 标记为 `PROC_TYPE_SERVICE`；普通应用默认为 `PROC_TYPE_NORMAL`，无硬件独占能力。
2. 服务禁止直接执行`in/out`指令，禁止直接使用MMIO物理地址；MMIO物理页由内核有条件映射进服务页表，服务仅使用映射后的虚拟MMIO地址。
3. 服务崩溃仅终止自身进程；内核自动完成全套资源回收，不会整机蓝屏/panic。
4. 服务注册、硬件绑定、条目增删、DMA、中断等待，**全部复用内核对象‑条目模型以及ACL权限，不引入独立自定义API/系统调用**。
5. **本规范实体统一称为用户态第三方服务**：注册流程（第 2 节 `/object/service_mgr`）、硬件独占绑定、中断与 DMA、崩溃自动回收、虚拟设备等机制见各节。

## 2 服务注册流程（利用对象条目 `CREATE_ENTRY` 权限）

全局纯控制对象：`/object/service_mgr`，无VFS映射，仅原生对象通路可访问。

- 对象ACL：开启`CREATE_ENTRY`，关闭`DELETE_ENTRY`；服务仅允许创建自身注册条目，不允许删除其他服务的条目。

注册步骤（全部使用对象条目原语：`OpenObject`、`ObjectCreateEntry`、`ObjectWriteEntry`）：

1. 服务进程调用 `OpenObject("/object/service_mgr")` 获取服务管理器对象句柄。
2. 在该对象上调用 `ObjectCreateEntry`，新建一条服务注册条目。
3. 使用 `ObjectWriteEntry` 向新建条目写入结构化内容：`服务PID、硬件DevID、服务版本、状态(ready/busy/error)`。
4. 内核在校验条目创建权限同时校验：进程是否允许标记为`PROC_TYPE_SERVICE`。校验通过，自动将本进程PCB标记为`PROC_TYPE_SERVICE`。
5. 没有完成该注册条目创建的进程，后续操作硬件对象条目会被ACL拒绝；系统管理工具可`ObjectReadEntry`读取条目查看服务状态。

> 
> 说明：普通应用、其他服务没有`DELETE_ENTRY`权限，不能删除他人的服务注册条目。

## 3 硬件独占绑定（单硬件绑定单一服务PID）

每一个PCI/总线硬件，对应一个独立硬件内核对象，例如 `/object/hw/pci/<DevID>`。

> 
> 绑定不是特殊函数调用：**向硬件对象内的专用控制条目写入绑定请求结构体完成绑定动作**。

绑定前置校验（内核在条目写入时自动执行）：

1. 调用进程已经注册，身份为`PROC_TYPE_SERVICE`。
2. 硬件对象内部状态条目标记为空闲，未被其他PID占用。
3. 当前进程ACL具备该硬件对象对应控制条目的写权限。

向硬件对象的控制条目写入绑定请求，校验成功后内核执行内部动作：

1. 修改硬件对象内部状态条目：记录绑定关系 `DevID ↔ 服务PID`，状态修改为独占占用；其它进程读写该硬件对象业务条目时返回业务状态 `-EBUSY`。
2. 配置VT‑d IOMMU分组：该硬件DMA仅允许访问本服务后续申请的DMA缓冲区，禁止访问内核、其它进程内存。
3. 将硬件物理MMIO区间映射至当前服务进程页表，页表属性设置UC不可缓存；**硬件对象输出条目填充返回虚拟MMIO起始地址**。
4. 服务拿到输出条目中的虚拟MMIO地址，直接读写该虚拟地址操作硬件寄存器，无需IPC转发。

## 4 中断与DMA缓冲区，同样基于条目

1. **中断等待**：硬件对象存在一条事件等待条目。服务对该条目执行等待读；硬件发生中断，内核向该条目写入中断事件数据，唤醒等待的服务进程。
2. **DMA缓冲区分配**：向硬件对象的DMA分配控制条目写入申请参数（size）；内核分配物理页，配置IOMMU权限；DMA分配输出条目返回服务可使用的用户虚拟地址。> 
> 服务永远不会拿到底层物理地址；禁止把任意普通进程内存直接交给硬件做DMA。

## 5 解绑与崩溃自动回收

1. **主动解绑**：向硬件对象控制条目写入解绑命令结构体，触发解绑流程。
2. 服务进程崩溃/被杀死，内核对象管理器检测对应PID消亡，自动执行内部回收逻辑：
   1. 撤销该进程页表里MMIO虚拟映射；
   2. 清除硬件IOMMU分组配置；
   3. 向硬件下发复位；
   4. 释放全部DMA缓冲区；
   5. 修改硬件对象内部状态条目：清空PID绑定记录，硬件状态置回空闲；
   6. 删除 `/object/service_mgr` 下该服务对应的注册条目。

## 6 权限逻辑总结

> 
> 不存在一套专门给服务的独立API，所有动作都是：打开对象 → 创建/读/写/等待条目。
> 不是把MMIO物理地址交给用户态服务直接访问；
> 内核在对象条目读写路径完成身份、ACL、独占状态全部校验之后，建立页表映射，由对象输出条目给出虚拟MMIO地址；
> 即便服务知晓硬件物理MMIO数值，没有内核通过对象条目体系建立映射，也完全无法访问硬件寄存器。
>
> 对接：对象‑条目、ACL 基础见 `objmgr.md`、`acl.md`；外部设备接入见 `external_dev.md`；服务生命周期见 `process.md`。

## 7 服务创建虚拟设备

1. **虚拟设备**：由服务创建的**无真实硬件**的设备节点，纯软件模拟——如虚拟终端 / 虚拟串口 / 虚拟网卡 / 虚拟磁盘。
2. **注册**：经设备管理器（`devmgr.md`）登记为设备节点 `/object/dev/<名称>`；无 MMIO、无 DMA、无中断（不占硬件资源）。
3. **数据通路**：与物理设备一致——设备节点 + 对象条目读写（read / write / ioctl 语义），对使用方透明。
4. **生命周期**：随服务进程；服务解绑 / 崩溃时（第 5 节回收）虚拟设备连同节点一并移除。
5. **文字模式联动**：显卡文字模式控制台可虚拟化为**虚拟终端设备**；进程文字输出连接虚拟终端（`process.md` 第 8 节、`ui.md` 第 22 节）。
6. **典型用途**：虚拟终端（进程控制台）、测试桩、外设模拟。

## 8 服务管理器 `.api` 契约文件（service.manager.api 全文）

服务管理器对外接口统一由以下契约文件声明，文件存放于 `/system/api/service.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# service.api 服务管理子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = service
env = python
exec = /system/devmgr/manager
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 50次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/service service

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
service: service_name { } /internal/service/name
service: pid { } /internal/service/pid
service: state { } /internal/service/state
service: bound_device { } /internal/service/bound
service: supported_devices { } /internal/service/supported

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
service: register { return self.register(name) } /internal/service/register
service: unregister { return self.unregister() } /internal/service/unregister
service: bind_device { return self.bind(dev_id) } /internal/service/bind
service: unbind_device { return self.unbind(dev_id) } /internal/service/unbind
service: reset_device { return self.reset(dev_id) } /internal/service/reset

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    service_registered(name: str) ｜ /internal/event/drv_reg
    service_unregistered(name: str) ｜ /internal/event/drv_unreg
    device_bound(dev_id: str) ｜ /internal/event/dev_bound
    device_unbound(dev_id: str) ｜ /internal/event/dev_unbound

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    reset_device: 5次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    register: aux_system
    bind_device: aux_system
```

契约要点：

1. 契约对象内部路径 `/object/service_mgr`，与本文第 2 节服务注册对象一致。
2. 只读字段透传 `info` 下服务状态；写字段为 `ctl` 下控制命令（register / unregister / bind_device / unbind_device / reset_device）。
3. 全局限流 50 次/秒；全部操作审计留痕（audit=true）。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。