# kernel.md 内核镜像与模块规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 混合内核架构：**微内核即原生内核**，**内核扩展文件即其他系统的驱动**（混合内核中除微内核之外的全部部分），用户态服务是本系统的驱动；本文档定义内核镜像组成与内核扩展文件的标准。
> 2. Ring0 内核模块与内核共享地址空间，模块 bug 会导致整机 panic；Ring1/Ring2 闲置。
> 3. **条目不在此文档硬编码**：内核对外条目的集合、命名、权限由 `kernel.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义内核镜像与模块的行为语义，契约全文见第 7 节。
> 4. 对应目录树 `/system/kernel/`：`core`（内核主镜像、内核扩展二进制）、`runtime`（开机画面、内核参数模板）、`modules`（Ring0 内核硬件服务模块）、`aux_modules`（内核附属功能模块，含嵌入内核态的第一方服务）。
> 5. 依赖：`os.md`、`objmgr.md`、`boot.md`、`memory.md`、`hardware_svc.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 内核主镜像 | 内核基座可执行镜像，含最小微内核核心，位于 `kernel/core` |
| 内核模块 | 可加载内核扩展（内核扩展文件），对标 Linux `.ko`；与内核共享地址空间 |
| 微内核 | 本系统的原生内核，核心常驻、不可卸载 |
| 内核扩展文件 | 其他系统所称的驱动；本系统中作为混合内核除微内核之外的全部部分 |
| 用户态服务 | 本系统的驱动，Ring3 服务进程；非性能敏感或易出错的功能放这里 |
| 硬件服务模块 | 串口、显卡、输入、块设备等硬件服务，位于 `kernel/modules` |
| 附属模块 | 内核附属功能模块（VFS 适配、网络协议栈、VMX 等）与嵌入内核态的第一方服务，位于 `kernel/aux_modules` |
| 内核参数 | 启动参数模板，位于 `kernel/runtime` |

## 1 定位与组成

1. **内核主镜像（`core`）**：内核基座可执行文件 + 内核扩展二进制；包含最小微内核核心（调度、物理内存、中断、基础 IPC、对象管理器）。
2. **运行时资源（`runtime`）**：开机画面、内核参数模板等非代码资源。
3. **硬件服务模块（`modules`）**：Ring0 硬件服务模块，动态加载/卸载（`boot.md` 阶段 5）。
4. **附属功能模块（`aux_modules`）**：VFS 适配、网络协议栈、VMX 虚拟化等可选内核模块，以及**嵌入内核态的第一方服务**（随内核装载）。
5. **归属判据**：**能提高速度、且出错率较低**的功能 → 内核扩展文件（`modules` / `aux_modules`）；性能收益不足或出错概率高 → 用户态服务（`hardware_svc.md` / `external_dev.md`）。

## 2 可加载模块机制

1. 模块为标准内核扩展格式，加载后与内核共享地址空间，直接函数调用无 IPC 开销。
2. **加载流程**：向内核模块接口提交模块路径 → 校验签名/完整性 → 映射入内核空间 → 执行模块初始化 → 注册模块至内核模块表。
3. **卸载流程**：提交卸载 → 停止模块服务 → 注销 → 释放模块占用资源。
4. 模块依赖管理：硬件不支持（如无 VMX）则对应模块不加载，不影响内核基座运行（`boot.md` 第 3 节）。
5. 模块 bug 造成内核 panic：由内核紧急控制台处理（`boot.md` 第 2 节）。
6. **模块准入**：只接受性能收益明确且错误率低的模块；不满足者退回用户态服务实现，不因「驱动就该在内核」而强行入内核。

## 3 与服务子系统协同

1. `kernel/modules` 中的硬件服务模块为 Ring0 服务（内核态）；用户态第三方服务见 `hardware_svc.md`、`external_dev.md`。
2. 磁盘文件系统、VFS 适配等模块在启动阶段加载（`boot.md` 阶段 5）。
3. 模块向对象管理器注册对应内核对象（硬件对象、IO 流对象等）。

## 4 内核参数

1. 内核参数模板存放于 `kernel/runtime`，bootloader 传递（`boot.md` 阶段 2）。
2. 参数影响内核早期初始化行为（内存布局、调试开关等）。
3. 运行期参数经内核接口只读暴露，修改需重启。

## 5 与各子系统对接

1. **os.md**：内核主镜像 + 可加载模块是混合内核的内核态组成。
2. **boot.md**：模块加载发生在启动阶段 5；基座初始化在阶段 4。
3. **objmgr.md**：内核模块创建/登记内核对象经对象管理器。
4. **memory.md**：模块加载占用内核地址空间，遵循内存规范。
5. **hardware_svc.md**：Ring0 硬件服务模块与用户态服务协同绑定硬件。
6. **.api.md**：内核对外接口经 `kernel.api` 契约与桥接层。7. **rtc.md**：内核时钟与定时器由时间子系统（rtc.md）维护。

## 6 容错

1. 模块加载失败：拒绝加载，报告原因，不影响已加载模块。
2. 模块 panic：整机崩溃（内核模块固有特性），进入内核紧急控制台。
3. 核心模块（基座）常驻不可卸载；仅可加载模块支持动态加载/卸载。

## 7 内核 `.api` 契约文件（kernel.api 全文）

内核对外接口统一由以下契约文件声明，文件存放于 `/system/api/kernel.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# kernel.api 内核镜像与模块子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = kernel
env = python
exec = /system/kernel/core
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = core_system
# 全局默认调用限流规则
global_limit = 50次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/kernel kernel

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
kernel: version { } /internal/kernel/version
kernel: modules { } /internal/kernel/modules
kernel: params { } /internal/kernel/params

# ---- 可写控制条目：转换代码为Python，实现模块管理 ----
kernel: load_module {
    if not self.verify(path):
        raise PermissionError("模块校验失败")
    self.load_module(path)
} /internal/kernel/module/load
kernel: unload_module { self.unload_module(name) } /internal/kernel/module/unload

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    module_loaded(name: str) ｜ /internal/event/module_load
    module_unloaded(name: str) ｜ /internal/event/module_unload
    kernel_panic(reason: str) ｜ /internal/event/kernel_panic

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    load_module: 10次/秒
    unload_module: 10次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    load_module: core_system
    unload_module: core_system
```

契约要点：

1. 外部进程仅能访问 `/system/api/kernel` 标准虚拟对象；内核模块加载/卸载仅授权 `core_system`。
2. `load_module` 经 Python 转换代码先做模块校验，失败拒绝。
3. 事件仅封装契约声明的标准参数；模块加载/卸载、内核 panic 供系统管理监听。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 8 设计取舍

1. 内核拆分为常驻基座 + 可加载模块，硬件无关功能可裁剪，保持最小基座。
2. 模块与内核共享地址空间换取性能，代价是模块 bug 导致 panic（混合内核固有取舍）。
3. 模块加载校验前置在桥接层，杜绝未验证模块进入内核。
4. 对外只暴露 `/system/api/kernel` 标准接口，内核镜像与模块内部完全隐藏。
