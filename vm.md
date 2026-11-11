# vm.md 虚拟机子系统规范（x86‑64，VMX，基于现有 os.md/hardware_svc.md）

> 
> 前提说明
> 
> 
> 1. 仅面向 x86‑64 Intel VMX；AMD‑SVM 暂不纳入；Ring1/Ring2 保持闲置不使用。
> 2. 本系统虚拟化运行在 VMX‑Root 模式，属于 Ring0 内核基座模块；客户机运行在 VMX‑Non‑Root。
> 3. 复用全局内核对象管理器、ACL 权限、VT‑d IOMMU 硬件隔离、硬件绑定表；不破坏原有对象‑条目模型。
> 4. 用户态虚拟机管理程序不直接操作 VMX 硬件指令；全部通过读写、等待虚拟机对象的条目完成控制，无独立自定义系统调用。
> 5. 硬件直通逻辑复用 hardware_svc.md 的 IOMMU 分组机制。

## 术语

- VMCS：虚拟机控制块，内核内部数据结构，用户态完全不可见。
- EPT：扩展页表，二级地址翻译，GPA（客户机物理地址）→ HPA（宿主机真实物理地址）。
- VM‑Exit：客户机执行特权操作触发退出，切回宿主机 VMX‑Root Ring0 处理。
- VM‑Entry：重新进入 Non‑Root，继续运行客户机。

## 1 总体约束

1. VMX 初始化、VMCS 管理、EPT 页表、VM‑Exit 处理全部在内核 Ring0 完成，作为可加载内核模块。
2. 用户态虚拟机管理器为普通`PROC_TYPE_NORMAL`进程；**不能直接执行 VMX 指令**，只能操作虚拟机内核对象的条目。
3. 每一台虚拟机对应一个**纯控制对象**，路径示例：`/object/vm/vm<id>`，无 VFS 映射，仅原生对象通路可用。
4. 虚拟机对象遵循标准 ACL；条目具备独立`CREATE_ENTRY`、`DELETE_ENTRY`权限；只有授权进程可以创建 / 销毁虚拟机实例。
5. 真实物理硬件：宿主机第三方服务依旧运行于 Ring3 `PROC_TYPE_SERVICE`；虚拟机要做硬件直通，需要先解除宿主机服务对该设备的绑定。
6. 虚拟机崩溃、异常终止不会破坏宿主机内核；内核自动回收虚拟机全部资源。

## 2 虚拟机对象内部条目布局

对象路径：`/object/vm/vm<id>`（纯控制对象）

表格

| 条目名称 | 权限 | 作用说明 |
| --- | --- | --- |
| `ctl` | 可读可写 | 控制命令条目：下发创建 vCPU、启动、暂停、复位、销毁命令结构体 |
| `state` | 只读 | 虚拟机状态：`created/running/paused/stopped/error` |
| `vcpu_list` | 只读 | 当前虚拟机下所有 vCPU 编号列表 |
| `ept_config` | 只写 | EPT 内存布局配置：客户机内存大小、GPA 空间分配参数 |
| `dev_passthrough` | 可读可写 | 硬件直通配置条目；写入 DevID 执行直通，清空 DevID 解除直通 |
| `event` | 等待可读 | 虚拟机事件队列：崩溃、暂停完成、错误码等事件通知 |
| `snapshot_ctl` | 可读可写 | 【未来可选】快照控制：触发创建快照、加载快照、删除快照 |
| `migrate` | 可读可写 | 【未来可选】迁移控制：导出虚拟机状态流、接收迁入状态数据 |

> 
> 每个 vCPU 会作为该虚拟机对象下**动态子条目**，由内核收到创建 vCPU 命令后动态创建；子条目包含 vCPU 状态、寄存器快照、VM‑Exit 统计信息。

### 创建一台虚拟机流程

1. 用户态管理程序打开父对象 `OpenObject("/object/vm")`。
2. 使用 `ObjectCreateEntry()` 创建新的虚拟机实例子对象，ACL 校验调用方拥有父对象的`CREATE_ENTRY`权限。
3. 得到新虚拟机对象句柄；向`ept_config`条目写入客户机内存大小等参数。
4. 向`ctl`条目写入创建 vCPU 的命令，内核动态生成 vCPU 子条目、分配 VMCS、初始化 EPT 页表。
5. 向`ctl`写入启动命令；内核执行 VMLAUNCH 进入 Non‑Root 模式，客户机开始运行。
6. 用户态对`event`条目做等待读，接收虚拟机状态变更、异常事件。

## 3 VM‑Exit 处理逻辑（内核内部）

客户机在 Non‑Root 发生 VM‑Exit，宿主机 Ring0 执行：

1. 保存客户机上下文。
2. 判断退出原因：
   - IO 访问、MMIO 访问：若是虚拟设备，内核模拟设备行为；若是直通硬件，交由 IOMMU 硬件处理。
   - 页表相关：处理 EPT 缺页。
   - 中断、异常：分发或者注入虚拟中断给客户机。
3. 处理完毕，执行 VMRESUME 回到客户机 Non‑Root。

> 
> 用户态永远看不到 VMCS，不参与 VM‑Exit 直接处理；虚拟磁盘、虚拟网卡、虚拟显卡的设备模拟逻辑：
> 
> 
> - 方案 A：内核内实现极简模拟；
> - 方案 B：内核仅做 VM‑Exit 捕获，通过 IPC 将模拟事件转发到用户态虚拟机管理器进程处理。
> 
> 
> 【未来可选扩展】虚拟磁盘、虚拟网卡实现为**IO 流对象**，客户机 VM‑Exit 产生的 IO 请求重定向到此 IO 流对象；宿主机侧可通过原生对象条目或者 VFS 通路读写该对象，完成宿主机与客户机之间的数据交互。

## 4 硬件直通（复用 hardware_svc.md IOMMU）

> 
> 前提：目标硬件已经从宿主机解绑：向对应硬件对象控制条目写入解绑命令，宿主机`PROC_TYPE_SERVICE`服务释放该硬件。

1. 向虚拟机对象的`dev_passthrough`条目写入要直通的`DevID`。
2. 内核校验：
   1. 硬件当前处于空闲未绑定状态。
   2. VT‑d/IOMMU 分组可用，设备可以隔离。
3. 内核动作：
   1. 将该 PCI 设备从宿主机环境剥离。
   2. 修改 IOMMU 页表，把设备 DMA 重定向到该虚拟机 EPT 的 GPA 空间。
   3. 把硬件 BAR（物理 MMIO）映射进客户机 GPA 地址空间。
4. 客户机内部 OS 就可以发现并服务该硬件。

> 
> 解除直通：向`dev_passthrough`写入空 DevID；内核回收 IOMMU 配置，硬件回到宿主机空闲状态，可以再次被宿主机用户态服务绑定。

> 
> 安全保证：IOMMU 阻止该设备 DMA 访问宿主机真实物理内存；虚拟机内部即使服务 bug，不能破坏宿主机内存。

## 5 销毁虚拟机、资源回收

### 主动销毁

向虚拟机对象`ctl`条目下发销毁命令；内核执行：

1. 如果存在直通硬件，自动解除硬件直通，恢复硬件到宿主机空闲。
2. 停止所有 vCPU，释放 VMCS 结构。
3. 释放 EPT 二级页表、客户机占用的宿主机物理内存。
4. 删除虚拟机对象所有子条目。
5. 删除虚拟机对象本身。

### 异常情况：客户机崩溃 / 用户态管理进程死亡

内核对象管理器检测关联进程消亡，自动执行完整销毁回收流程，不会泄露资源。

## 6 和现有文档的对接关系

1. **对接 os.md**
   - 虚拟机是纯控制对象，完全使用对象、条目、ACL（含`CREATE_ENTRY`/`DELETE_ENTRY`）。
   - 不新增独立 API；全部操作是打开对象、读 / 写 / 等待条目。
2. **对接 hardware_svc.md**
   - 硬件直通复用 VT‑d IOMMU、硬件对象绑定逻辑；硬件必须先解除宿主机服务绑定，才可直通虚拟机。
3. **对接 Linux.md**
   - 客户机内部可以运行 Linux ELF 系统，和宿主机 Linux 兼容层无关；宿主机 VFS 不受虚拟机影响。

## 7 未来可选扩展说明

1. **快照 / 迁移**
借助对象内新增的 `snapshot_ctl`、`migrate` 条目。

> 
> 外部对象‑条目接口保持不变，快照存储、内存压缩全部由内核内部实现。
   - 快照：向`snapshot_ctl`写入快照创建命令，内核把 vCPU 寄存器、EPT 内存状态导出为对象内部快照子条目；后续可下发加载命令恢复虚拟机现场。
   - 迁移：通过`migrate`条目导出完整虚拟机状态二进制流；目标主机接收状态流，重建虚拟机对象、vCPU、EPT，实现热迁移。
2. **AMD‑SVM 支持**
仅修改内核底层实现（替换 VMX 相关逻辑为 SVM，VMCB 替代 VMCS）；**上层虚拟机对象、所有条目、命令结构体、ACL 权限完全不用改动**；用户态虚拟机管理程序无需修改，做到硬件架构透明。
3. **虚拟设备（虚拟磁盘、虚拟网卡）**
将虚拟磁盘、虚拟网卡实现为**IO 流对象**，不属于虚拟机对象的子条目，为独立全局对象。
   - 客户机 IO VM‑Exit 事件重定向到此 IO 流对象；
   - 宿主机可以使用原生对象 API，或者 VFS 虚拟文件通路访问该 IO 流对象；
   - 实现客户机与宿主机双向数据读写，虚拟磁盘后端可以对接宿主机磁盘文件

## 8 虚拟机实例 `.api` 契约文件（vm.instance.api 全文）

虚拟机对外接口统一由以下契约文件声明，文件存放于 `/system/api/vm.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# vm.api 虚拟机子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = vm
env = python
exec = /system/vm/vm_manager
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 100次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/vm vm

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
vm: name { } /internal/vm/name
vm: state { } /internal/vm/state
vm: vcpu_count { } /internal/vm/vcpu_count
vm: memory_mb { } /internal/vm/memory_mb
vm: vcpu_weight { } /internal/vm/vcpu_weight
vm: passthrough_devices { } /internal/vm/passthrough
vm: tap_interface { } /internal/vm/tap

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
vm: start { return self.start() } /internal/vm/start
vm: pause { return self.pause() } /internal/vm/pause
vm: stop { return self.stop() } /internal/vm/stop
vm: reset { return self.reset() } /internal/vm/reset
vm: snapshot { return self.snapshot(name) } /internal/vm/snapshot
vm: add_passthrough { return self.add_passthrough(dev_id) } /internal/vm/add_passthrough
vm: attach_tap { return self.attach_tap(ifname) } /internal/vm/attach_tap

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    vm_started(name: str) ｜ /internal/event/vm_start
    vm_paused(name: str) ｜ /internal/event/vm_pause
    vm_stopped(name: str) ｜ /internal/event/vm_stop
    vm_crashed(name: str) ｜ /internal/event/vm_crash

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    snapshot: 10次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    start: aux_system
    stop: aux_system
```

契约要点：

1. 契约对象内部路径 `/object/vm`（父对象），与本文第 2 节虚拟机对象一致；实例子对象路径 `/object/vm/vm<id>`。
2. 只读字段透传 `props`/`config` 下配置；写字段为 `ctl` 下控制命令（start / pause / stop / reset / snapshot / add_passthrough / attach_tap）。
3. 全局限流 100 次/秒；全部操作审计留痕（audit=true）。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。