# exeo.md 可执行对象（.exeo）规范

> 
> 前提说明
> 
> 1. **可执行对象**：`.exeo` 为系统可执行文件对象；**开机时**对每个可执行文件分配**固定对象路径** `/object/exeo/<名称>`，对象身份不随实例变化。
> 2. **需要 `.api` 契约**：可执行对象的修改（默认启动参数、环境）与启动调用（传入开机参数）经 `exeo.api` 契约与桥接层完成。
> 3. 与 `dllo.md` 的边界：DLLO 是进程内直接链接（不需 `.api`）；`.exeo` 是可执行对象，需经 `.api` 传参 / 修改 / 启动。
> 4. Ring1/Ring2 闲置；启动与修改受 `acl.md` 权限管控；可执行文件签名 / 哈希校验（`security.md`）。
> 5. 依赖：`os.md`、`objmgr.md`、`process.md`、`asm.md`、`acl.md`、`security.md`、`fs.md`、`dllo.md`、`Linux.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 可执行对象 | 以 `.exeo` 登记的可执行文件对象，开机分配固定对象路径 |
| 固定对象 | 开机登记、不随实例变化的对象身份 `/object/exeo/<名称>` |
| 开机参数 | 启动可执行对象时传入的参数（`args`）与环境变量（`env`） |
| 实例 | 同一可执行对象的多次运行，经 `instances` 条目区分 |

## 1 总体约束

1. 开机时扫描可执行文件并登记为**固定对象**；对象路径 `/object/exeo/<名称>` 一经分配不再变化。
2. 同一可执行文件只有一个固定对象；多次启动以**实例**区分（见 `process.md`）。
3. 启动调用、参数修改、环境修改均经 `exeo.api` 契约（跨进程操作，走桥接层）。
4. 可执行文件发布时签名 / 哈希校验前置（`security.md`）；敏感可执行对象仅 `aux_system` 可启动。

## 2 可执行对象条目布局

### 可执行对象，路径 `/object/exeo/<名称>`

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `path` | 只读 | 可执行文件路径 |
| `entry` | 只读 | 入口地址 / 入口符号 |
| `args` | 可读可写 | 默认启动参数（经 `.api` 修改） |
| `env` | 可读可写 | 默认环境变量（经 `.api` 修改） |
| `status` | 只读 | 已登记 / 已加载 / 运行中 |
| `instances` | 只读 | 当前运行实例列表 |

## 3 启动调用（经 exeo.api）

1. 调用方按名称请求启动，可传**开机参数**（`args`）与环境变量（`env`）；
2. 校验：签名 / 哈希 + ACL；通过后创建进程实例（`process.md`）；
3. 进程加载 DLLO 依赖（`dllo.md`）与可执行对象入口，实例启动；
4. 广播 `exeo_started`（含实例 PID）。

## 4 修改（经 exeo.api）

1. 修改默认启动参数（`exeo_set_args`）与环境（`exeo_set_env`）；
2. 修改对**后续启动**生效，不影响已运行实例；
3. 修改记录审计日志（`log.md`）。

## 5 与各子系统对接

1. **os.md**：复用对象‑条目模型。
2. **objmgr.md**：可执行对象开机登记、回收。
3. **process.md**：启动调用创建进程实例。
4. **dllo.md**：进程加载动态链接库对象（进程内直接链接，不经 `.api`）。
5. **asm.md**：入口与符号解析约定。
6. **acl.md / security.md**：启动权限与签名校验。
7. **fs.md**：可执行文件存放与读取。
8. **Linux.md**：ELF 可执行文件经翻译按 `.exeo` 登记与启动。
9. **.api.md**：启动与修改经 `exeo.api` 契约与桥接层。

## 6 设计取舍

1. 开机固定对象：可执行文件身份稳定，程序自动化可按名引用。
2. 参数 / 环境走 `.api`：跨进程修改受 ACL 与审计管控。
3. 与 DLLO 分工：进程内链接走符号地址（零开销），进程外操作走 `.api` 契约。

## 7 可执行对象 `.api` 契约文件（exeo.api 全文）

可执行对象对外接口统一由以下契约文件声明，文件存放于 `/system/api/exeo.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# exeo.api 可执行对象对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = exeo
env = python
exec = /system/exeo/exeo_mgr
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 100次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/exeo exeo

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
exeo: exeo_list { } /internal/exeo/list
exeo: exeo_info { } /internal/exeo/info
exeo: exeo_instances { } /internal/exeo/instances

# ---- 可写控制条目：转换代码为Python，实现启动与参数修改 ----
exeo: exeo_exec {
    return self.exec(name, args, env)
} /internal/exeo/exec
exeo: exeo_set_args {
    return self.set_args(name, args)
} /internal/exeo/args
exeo: exeo_set_env {
    return self.set_env(name, env)
} /internal/exeo/env

# ---- 对外可监听标准事件 ----
EVENT:
    exeo_registered(name: str) ｜ /internal/event/exeo_reg
    exeo_started(name: str, pid: int) ｜ /internal/event/exeo_start
    exeo_exited(name: str, pid: int) ｜ /internal/event/exeo_exit

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    exeo_exec: 50次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    exeo_exec: user_foreground
    exeo_set_args: user_foreground
```

契约要点：

1. 外部程序经 `/system/api/exeo` 标准虚拟对象启动与修改可执行对象，禁止直连文件内部。
2. `exeo_exec` 传开机参数与环境，创建进程实例（`process.md`）。
3. `exeo_set_args` / `exeo_set_env` 修改默认参数，对后续启动生效。
4. 启动 / 退出事件供程序自动化与系统监视监听。
