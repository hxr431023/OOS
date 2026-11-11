# acl.md 对象 ACL 权限子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 的内核对象‑条目模型，ACL 挂载于**对象以及每一条独立条目**；对象 ACL 为默认，条目可以覆写自己独立 ACL。
> 2. 所有子系统：服务、虚拟机、IPC、VFS 兼容层全部复用这套 ACL，不存在独立权限体系。
> 3. Ring1/Ring2 闲置，权限校验全部在内核对象管理器内部完成；用户态无法绕过。
> 4. VFS 通路权限由对应条目 ACL 自动翻译为 r/w 权限，实现两套访问入口权限一致。

## 术语

- **对象 ACL**：对象级默认权限；对象内部新建条目默认继承此 ACL。
- **条目覆写 ACL**：单条条目拥有独立 ACL，优先级高于对象 ACL；可以把某一条条目设置成和对象不一样的权限。
- **权限位**：每一个权限位是独立开关，互不绑定。
- **主体**：进行访问的主体为进程 PID；内核内部组件（内核模块）拥有特权，绕过 ACL 校验。

## 1 全部 ACL 权限位定义

表格

| 权限位 | 说明 |
| --- | --- |
| `ENTRY_READ` | 读取条目数据、等待读（阻塞读） |
| `ENTRY_WRITE` | 写入 / 修改条目数据，下发命令 |
| `CREATE_ENTRY` | 在此对象之下**新建子条目** |
| `DELETE_ENTRY` | 删除本对象下的子条目 |

> 
> 注意：`CREATE_ENTRY` / `DELETE_ENTRY` 是作用于**父对象**的权限，不是单条条目的权限；控制在对象内增删子条目。
> 例子：`/object/service_mgr` 设置 `CREATE_ENTRY=允许`，`DELETE_ENTRY=拒绝`；服务可以新建自己注册条目，不能删除别人的条目。

## 2 ACL 生效规则

1. 访问一条条目时，优先读取**条目自身覆写 ACL**；如果条目没有自定义 ACL，则继承所属对象的对象 ACL。
2. `CREATE_ENTRY`、`DELETE_ENTRY` 永远读取**父对象 ACL**，不看子条目。
3. 内核模块（Ring0）执行对象操作，跳过 ACL 校验；所有用户态进程（无论`PROC_TYPE_SERVICE`/`PROC_TYPE_NORMAL`）都必须经过 ACL 校验。
4. `PROC_TYPE_SERVICE` 只是 PCB 标记，**不等于自动获得 ACL 权限**；标记仅用于服务绑定逻辑校验，权限依旧由 ACL 管控。

## 3 操作行为‑权限对照表

表格

| 用户态操作 | 需要的权限位 |
| --- | --- |
| `ObjectReadEntry()` | 目标条目：`ENTRY_READ` |
| `ObjectWriteEntry()` | 目标条目：`ENTRY_WRITE` |
| `ObjectCreateEntry()` | 父对象：`CREATE_ENTRY` |
| `ObjectDeleteEntry()` | 父对象：`DELETE_ENTRY` |
| VFS `open() read()` | 对应条目：`ENTRY_READ` |
| VFS `open() write()` | 对应条目：`ENTRY_WRITE` |

> 
> VFS 兼容层自动映射：
> 
> 
> - `ENTRY_READ` → 文件 r 位
> - `ENTRY_WRITE` → 文件 w 位
> - `CREATE_ENTRY`/`DELETE_ENTRY` 无 POSIX 对应，VFS 通路不可操作，只能原生对象 API。

## 4 ACL 的修改接口（通过条目完成）

每个对象自带内置只读可写条目 `acl_config`。

1. 打开对象句柄，向 `acl_config` 写入 ACL 配置结构体，修改**对象默认 ACL**。
2. 修改单条条目覆写 ACL：向该条目附属的配置子项写入覆写 ACL。
3. 修改 ACL 本身需要目标对象条目的 `ENTRY_WRITE` 权限；普通进程不能随意篡改权限。

## 5 典型示例（对接已有文档场景）

### 示例 1：服务管理器对象 `/object/service_mgr`

对象 ACL：

- `ENTRY_READ`：允许全部进程（所有人可以读取服务状态）
- `ENTRY_WRITE`：拒绝普通进程
- `CREATE_ENTRY`：允许授权进程（服务用来新建注册条目）
- `DELETE_ENTRY`：全部拒绝（不能删除别人服务注册条目）

### 示例 2：硬件对象 `/object/hw/pci/<DevID>`

- 状态条目：`ENTRY_READ`全部允许；`ENTRY_WRITE`拒绝普通进程
- 绑定控制条目：仅授权`PROC_TYPE_SERVICE`拥有`ENTRY_WRITE`，普通应用直接拒绝。

### 示例 3：虚拟机父对象 `/object/vm`

- `CREATE_ENTRY`：仅授权管理进程，用来创建虚拟机实例
- `DELETE_ENTRY`：仅授权管理进程，销毁虚拟机

## 6 权限继承与新建条目默认行为

调用`ObjectCreateEntry()`新建条目：

1. 默认继承父对象的对象 ACL 作为初始 ACL；
2. 创建的同时可以**附带传入自定义覆写 ACL**，直接给新条目设置独立权限。

## 7 错误返回

权限不足时：

- 原生对象 API 返回 `-EACCES`
- VFS/POSIX 接口返回 `-EACCES`，和 Linux 语义保持对齐。

## 8 和其它文档对接

1. **os.md**：ACL 是内核对象管理器内置组件，支持对象、条目两级权限；区分`CREATE_ENTRY`、`DELETE_ENTRY`独立权限。
2. **hardware_svc.md**：服务注册依赖父对象`CREATE_ENTRY`关闭`DELETE_ENTRY`；硬件绑定依赖控制条目`ENTRY_WRITE`权限。
3. **vm.md**：虚拟机实例创建依赖父对象`CREATE_ENTRY`。
4. **ipc.md**：消息队列、事件对象依靠 ACL 控制谁可以读、写、新建队列。
5. **Linux.md**：VFS 层将 ACL 映射为 POSIX r/w 权限；`CREATE_ENTRY`/`DELETE_ENTRY`不向 POSIX 暴露。
6. **objmgr.md**：ACL 校验由内核对象管理器统一执行；对象、条目两级权限由对象管理器承载。

## 9 ACL `.api` 契约文件（acl.api 全文）

ACL 对外接口统一由以下契约文件声明，文件存放于 `/system/api/acl.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# acl.api 对象 ACL 权限子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = acl
env = python
exec = /system/kernel/core
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = kernel
# 全局默认调用限流规则
global_limit = 500次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/acl acl

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
acl: object_acl { } /internal/acl/object_acl
acl: entry_acl { } /internal/acl/entry_acl

# ---- 可写控制条目：转换代码为Python，实现 ACL 查询与修改 ----
acl: get_object_acl {
    return self.get_object_acl(path)
} /internal/acl/object_acl_get
acl: get_entry_acl {
    return self.get_entry_acl(path, entry)
} /internal/acl/entry_acl_get
acl: set_object_acl {
    return self.set_object_acl(path, acl)
} /internal/acl/object_acl_set
acl: set_entry_acl {
    return self.set_entry_acl(path, entry, acl)
} /internal/acl/entry_acl_set

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    acl_changed(path: str) ｜ /internal/event/acl_change

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    set_object_acl: 10次/秒
    set_entry_acl: 10次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    set_object_acl: kernel
    set_entry_acl: kernel
```

契约要点：

1. ACL 修改（set_*）仅授权 `kernel` 分类进程（内核 / 管理员）；普通进程只读查询。
2. get / set 覆盖对象 ACL 与条目覆写 ACL 两级。
3. ACL 变更事件供安全审计（`log.md`）监听。

