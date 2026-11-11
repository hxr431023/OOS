# objmgr.md 内核对象管理器规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 内核对象管理器为**整个系统的底层根基**（`os.md` 二、）；运行于 Ring0 内核基座，常驻不可卸载。
> 2. 全局对象命名空间、对象生命周期、句柄、引用计数、条目增删、ACL 校验全部由本管理器统一承载。
> 3. **条目不在此文档硬编码**：对象管理器对外条目的集合、命名、权限由 `objmgr.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义对象管理器的行为语义，契约全文见第 7 节。
> 4. 对应目录树 `/system/objmgr/`，其中 `policies` 存放对象管理器 ACL、初始化策略配置。
> 5. 依赖：`os.md`、`acl.md`、`boot.md`、`process.md`、`.api.md`；所有子系统均复用本管理器。

## 术语

| 术语 | 说明 |
| --- | --- |
| 对象 | 内核资源的统一封装载体，带全局唯一路径名；分 IO 流对象与纯控制对象两大类 |
| 条目 | 对象内部独立状态/指令通道；每条条目独立配置 ACL |
| 命名空间 | 全局对象路径树，根为 `/object`；对象路径全局唯一 |
| 句柄 | 进程持有的对象访问凭证，由对象管理器分配与校验 |
| 引用计数 | 对象被持有的句柄数量；归零后对象自动回收销毁 |
| 对象目录 | 对象在命名空间中的路径，如 `/object/proc/<pid>`、`/object/hw/pci/<DevID>` |

## 1 定位与职责

内核对象管理器承担以下核心职责：

1. **命名空间管理**：维护全局对象路径树，保证路径唯一；提供按路径查找对象的能力。
2. **对象生命周期**：对象创建、初始化、销毁的统一入口；对象销毁时回收全部条目与资源。
3. **句柄管理**：句柄分配、校验、传递、关闭；句柄与进程绑定，防止跨进程越权访问。
4. **引用计数**：统一维护对象引用计数；句柄关闭、句柄传递时正确增减。
5. **条目管理**：条目创建、删除（受父对象 `CREATE_ENTRY`/`DELETE_ENTRY` ACL 管控）、条目数据读写入口。
6. **ACL 校验**：所有对象/条目访问必经 ACL 校验（`acl.md`）；内核模块特权绕过，用户态全部校验。
7. **崩溃回收**：进程消亡时，自动关闭其全部句柄、回收其持有对象（联动 `process.md` 第 7 节）。

## 2 对象命名空间

1. 命名空间为单根树，根节点 `/object`，在基座初始化阶段创建（`boot.md` 阶段 4）。
2. 路径语法：`/object/<分类>/<实例标识>`；分类包括 `proc`、`hw`、`ipc`、`net`、`window`、`service_mgr`、`vm`、`cmd`、`display`、`gfxbuf`、`blk`、`log`、`mem`、`cpu`、`api` 等。
3. 路径唯一性由对象管理器强制保证；重复路径创建返回冲突错误。
4. 对象按类别分为 **IO 流对象**（可挂载 VFS 生成虚拟文件）与**纯控制对象**（仅原生对象通路访问）两类（`os.md` 二、）。

## 3 对象生命周期

1. **创建**：调用方向对象管理器提交创建请求（含路径、类别、初始 ACL）；校验父目录 `CREATE_ENTRY` 权限后创建对象，分配引用计数为 1。
2. **查找**：按路径解析对象，返回句柄；无权限或路径不存在返回错误。
3. **销毁**：调用方提交销毁请求；校验 `DELETE_ENTRY` 权限；引用计数归零时对象管理器执行销毁，回收全部条目与底层资源。
4. **动态条目**：支持在对象内动态创建/删除条目，受对象 ACL 管控（`os.md`、`acl.md`）。
5. 内核对象管理器自身为常驻基座，不随进程消亡销毁。

## 4 句柄管理与引用计数

1. **句柄分配**：进程打开对象时，对象管理器在进程句柄表中登记句柄，递增对象引用计数。
2. **句柄校验**：所有对象操作先校验句柄归属与权限；无效句柄直接拒绝。
3. **句柄传递**：支持在 IPC 消息载荷中传递句柄（`ipc.md` 第 4 节）；传递时内核校验发送方权限并递增引用计数，接收方无 ACL 权限则拒绝传递。
4. **句柄关闭**：进程关闭句柄时递减引用计数；全部句柄关闭后对象销毁回收（引用计数归零）。
5. **进程消亡**：进程退出/崩溃时，对象管理器遍历并关闭该进程全部句柄（`process.md` 第 7 节）。

## 5 条目管理

1. 条目为对象内部独立状态/指令通道；每条条目独立配置 ACL 权限位（`ENTRY_READ`/`ENTRY_WRITE`/`CREATE_ENTRY`/`DELETE_ENTRY`）。
2. 新建条目默认继承父对象 ACL，创建时可附带自定义覆写 ACL（`acl.md` 第 6 节）。
3. 条目创建/删除作用于父对象，受父对象 `CREATE_ENTRY`/`DELETE_ENTRY` 权限管控。
4. 条目数据读写提供等待语义：等待可读条目无数据时阻塞调用方，有数据写入后自动唤醒（`ipc.md`）。

## 6 崩溃与异常回收

1. 内核对象管理器本身运行于 Ring0 基座，模块 bug 才可能导致内核 panic；对象管理器核心逻辑保持最小、稳健。
2. 进程消亡触发对象管理器统一回收：关闭句柄 → 递减引用计数 → 引用计数归零的对象执行销毁。
3. 对象管理器维护全部对象的注册表，支撑系统管理工具查询对象状态、统计资源使用。

## 7 对象管理器 `.api` 契约文件（objmgr.api 全文）

对象管理器对外接口统一由以下契约文件声明，文件存放于 `/system/api/objmgr.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# objmgr.api 内核对象管理器对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = objmgr
env = python
exec = /system/kernel/core
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = core_system
# 全局默认调用限流规则
global_limit = 1000次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/objmgr objmgr

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
objmgr: list_objects { } /internal/objmgr/list
objmgr: ref_count { } /internal/objmgr/ref_count
objmgr: stat.object_total { } /internal/objmgr/stat/object_total
objmgr: stat.handle_total { } /internal/objmgr/stat/handle_total

# ---- 可写控制条目：转换代码为Python，实现对象管理操作 ----
objmgr: lookup {
    return self.lookup(path)
} /internal/objmgr/lookup
objmgr: create {
    self.create(path, obj_type, acl)
} /internal/objmgr/create
objmgr: destroy {
    self.destroy(path)
} /internal/objmgr/destroy

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    obj_created(path: str, obj_type: str) ｜ /internal/event/obj_new
    obj_destroyed(path: str) ｜ /internal/event/obj_gone

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    create: 100次/秒
    destroy: 100次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    create: core_system
    destroy: core_system
```

契约要点：

1. 外部进程仅能访问 `/system/api/objmgr` 标准虚拟对象，禁止直连对象管理器内部实现。
2. `create`、`destroy` 等对象管理操作仅授权 `core_system` 分类进程调用。
3. 事件仅封装契约声明的标准参数；对象创建/销毁事件供系统管理工具监听。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 8 与各子系统对接

1. **os.md**：对象管理器是内核基座核心组件，是裸内核与完整系统的分水岭（`boot.md` 阶段 4）。
2. **acl.md**：ACL 挂载于对象及条目，由对象管理器统一校验；内核模块特权绕过。
3. **boot.md**：基座初始化阶段启动全局对象管理器，创建根命名空间与全局基础对象。
4. **process.md**：进程对象、句柄表、崩溃回收与对象管理器联动。
5. **ipc.md**：句柄传递、引用计数、事件等待由对象管理器统一承载。
6. **.api.md**：对象管理器对外接口经 `objmgr.api` 契约与桥接层。
7. **hardware_svc.md / vm.md / net.md / ui.md / fs.md / log.md**：全部子系统对象均经对象管理器创建、登记、回收，路径统一挂载于 `/object` 命名空间。

## 9 设计取舍

1. 单一对象管理器统一承载命名空间、句柄、引用计数、ACL，避免各子系统重复实现对象管理。
2. 句柄与进程绑定，配合 ACL 实现最小权限与越权隔离。
3. 引用计数自动回收，配合进程消亡统一回收，杜绝资源泄漏。
4. 对外只暴露 `/system/api/objmgr` 标准接口，内部命名空间、句柄表完全隐藏。
