# account.md 账户控制子系统规范（x86‑64）

> 前提：
> 1. **账户控制**（account）对应 Windows 用户账户控制（UAC）与账户管理：账户模型、角色分级、令牌与完整性级别、提权同意 / 凭据提示；对应用户账户与系统账户两大类身份。
> 2. 完全基于 `os.md` 对象‑条目模型；账户以对象承载，权限判定最终由 `acl.md` 完成——本规范只定义「谁是什么角色、能提升到什么权限、提权后进程拿到什么令牌」。
> 3. 部署于 `/system/security/account/`：`account_svc.service`（后台常驻账户控制**服务文件** `.service`，由服务加载器加载、在服务运行打包容器中运行）与 `config/`（账户策略、提权策略）。
> 4. 账户分**两大部分**：**系统账户**（`system`、`update`）与**用户账户**（`user`、`admin`、`root`）。架构参考：`user` / `admin` / `update` / `system` 按 **Windows** 语义，`root` 按 **Linux** 语义。
> 5. 职责边界：`auth.md` 认证「你是谁」；本规范确定「你是什么角色、能提升到哪一级」；`session.md` 承载登录后的会话上下文；`acl.md` 判定「你能访问什么」。
> 6. 依赖：`auth.md`、`session.md`、`acl.md`、`process.md`、`security.md`、`secure_island.md`、`ui.md`、`log.md`、`update.md`、`install.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 账户 | 身份记录 + 角色 + 策略 + 凭据引用；以 `/object/account/<account_id>` 承载 |
| 角色 | `system` / `update` / `user` / `admin` / `root` 五类，决定令牌与可执行的管理动作 |
| 标准令牌 | 受限令牌（filtered token）：权限标签、完整性级别受限，日常运行默认使用 |
| 提权令牌 | 高权限令牌：经同意 / 凭据提示后创建，只授予单个进程 |
| 完整性级别 | `low` / `medium` / `high` / `system`，与令牌绑定，参与 ACL 判定 |
| 同意提示 | consent：管理员用户提权时仅确认，不输入密码 |
| 凭据提示 | credential：普通用户提权时必须输入管理员 / root 凭据 |
| 管理员批准模式 | Admin Approval Mode：管理员登录后默认以标准令牌运行，需要时再提权 |
| 安全桌面 | secure desktop：提权提示独占的最高图层，渲染与输入隔离，防伪造窗口与输入劫持 |
| 盾牌标记 | 程序清单声明「需要管理员权限」，界面显示盾牌图标 |
| 系统重要文件 | 内核镜像与内核扩展、系统核心服务、安全策略与账户库、更新区、固件与硬件配置；含 system 自身文件；仅 `update` 可写，`system` 不可读、不可写 |
| 硬件操作 | MMIO、DMA、中断、固件访问；仅 `system` 账户与内核服务可执行，其余账户须经硬件服务代理 |

## 1 账户分类

账户分为**系统账户**与**用户账户**两大部分，互不混用。

### 1.1 系统账户

1. **`system`（系统账户）**：Windows SYSTEM 语义的内置最高权限账户，用于内核组件、init（`boot.md`）、核心服务；**几乎可以操作系统中任何类型的文件与资源**——硬件（MMIO / DMA / 中断 / 固件）、系统服务与进程、普通系统文件与用户文件；但**不能读、也不能写自己的文件**：system 账户自身记录与凭据、正在运行的内核镜像、自身服务代码、账户库等系统重要文件一律不可访问；系统重要文件只由 `update` 在更新时写入；**不可交互登录**、无登录界面入口，只能由内核 / init 以服务方式使用；全部操作强制审计。
2. **`update`（更新权限账户）**：Windows 更新账户语义的**受限专用账户**——是**系统重要文件唯一的更新写入身份**：只允许读写更新区与系统部件、执行替换与回滚；**不能操作硬件**、不能管理账户、不能改 ACL、不能安装任意软件；更新服务（`update.md`）以该账户身份运行。独立于 `system`，使更新链路被攻破时不等于拿到全系统权限。

### 1.2 用户账户

1. **`user`（普通用户）**：Windows 标准用户语义，**权限很小**；默认角色，持标准令牌；只能访问自身配置视图与授权文件，不能安装系统级软件、不能改系统设置、**不能操作硬件、不能访问系统重要文件**；提权需**凭据提示**（输入管理员 / root 凭据）。
2. **`admin`（管理员用户）**：Windows 管理员语义，**够正常使用**——可安装软件、改系统设置、管理普通用户；启用**管理员批准模式**，登录后默认标准令牌运行，管理操作弹出**同意提示**（仅确认）；仍**不能直接读写系统重要文件、不能操作硬件**，需经对应系统服务代理。
3. **`root`（最高权限用户）**：**Linux root 语义**（架构参考 Linux，而非 Windows）；是**用户空间最高权限**——可管理全部用户、进程、用户文件与软件安装；但**不能访问或修改系统重要文件**、**不能操作硬件**（系统核心与硬件只归 `system`）；提权**不弹同意提示**（可按策略强制提示）；所有操作强制审计；默认禁用交互登录或要求强认证（策略可配）。

| 账户 | 类别 | 架构参考 | 硬件操作 | 系统重要文件 | 用户空间权限 | 提权方式 |
| --- | --- | --- | --- | --- | --- | --- |
| `system` | 系统 | Windows SYSTEM | **可**（MMIO / DMA / 中断 / 固件） | **否**（不读不写自身文件，仅 `update` 可写） | **系统内任何文件类型**（自身文件除外） | 不可提权（本身最高） |
| `update` | 系统 | Windows 更新账户 | 否 | **可读写**（仅更新区与系统部件，**唯一更新写入者**） | 无 | 不可交互提权 |
| `user` | 用户 | Windows 标准用户 | 否 | 否 | **很小**：仅自身配置与授权文件 | 凭据提示 |
| `admin` | 用户 | Windows 管理员 | 否（经服务代理） | 否（经服务代理） | 正常使用：装软件 / 改设置 / 管普通用户 | 同意提示 |
| `root` | 用户 | Linux root | **否** | **否**（不可访问 / 不可修改） | **最高**：全部用户、进程、用户文件、装软件 | 直接授予（可配提示） |

## 2 账户对象条目

账户以**纯控制对象** `/object/account/<account_id>` 承载，条目：

1. `name`：只读，账户名。
2. `type`：只读，`system` / `user`。
3. `role`：只读，`system` / `update` / `user` / `admin` / `root`。
4. `state`：只读，`active` / `disabled` / `locked`。
5. `groups`：只读，所属组（如 administrators）。
6. `token_level`：只读，`standard` / `elevated` / `system`。
7. `integrity`：只读，`low` / `medium` / `high` / `system`。
8. `home`：只读，该账户配置视图与主目录根。
9. `policy`：可读可写，登录与会话策略（是否允许交互登录、是否强制提权提示、会话并发上限）。
10. `last_login`：只读，最近成功登录时间。

## 3 令牌与完整性级别

1. 每个进程携带账户令牌：账户 ID + 角色 + 组 + 完整性级别 + 权限标签。
2. 进程派生**默认继承父令牌**；**提权不继承**——提权只为目标进程创建高令牌，父进程与会话默认令牌保持标准，防止提权沿进程树扩散。
3. 完整性级别：`low`（沙箱 / 不可信应用）、`medium`（普通用户标准）、`high`（管理员提权）、`system`（系统账户 / 内核服务）。
4. ACL 判定时按令牌映射权限标签（`acl.md`），不新增权限位；账户角色只决定可获得哪些标签，不绕过对象 ACL。
5. 服务进程按服务文件声明的运行身份分配令牌（`system` / `update` / 专用服务账户）；服务运行期间**不得自行提权**。
6. `system` 与 `update` 账户令牌只能由 init / 服务加载器在启动阶段分配，用户态 API 不可申请。
7. `system` 的「**不能读也不能写自身文件**」由内核强制：system 账户自身记录与凭据、正在运行的内核镜像、自身服务代码、账户库等系统重要文件，对 system 一律不可读、不可写；该限制不依赖 ACL 配置，任何提权都无法绕过。
8. 系统重要文件的**读写**只由 `update` 账户在更新路径执行；`system` 不读、不写系统重要文件，二者职责不重叠。

## 4 提权流程（UAC）

1. **触发**：程序清单声明需要高权限（盾牌标记）、用户主动「以管理员身份运行」，或系统操作（安装、更新、改系统配置）需要高权限。
2. **判定**：`account_svc.service` 读取账户角色与策略：
   - `root` → 直接授予**用户空间高令牌（high）**（策略可要求提示）；系统重要文件与硬件操作**不随提权授予**：系统重要文件只能交 `update` 更新路径，硬件只能交 `system` 或硬件服务代理；
   - `admin` → **同意提示**（consent），仅确认；
   - `user` → **凭据提示**（credential），输入管理员 / root 凭据；
   - `system` / `update` 账户进程 → 拒绝交互提权。
3. **提示**：切换到**安全桌面**（独立最高图层，渲染与输入隔离），显示程序名、发布者、请求权限与盾牌图标；普通桌面无法伪造或覆盖该提示。
4. **决策**：同意 → 创建提权令牌与高完整性新进程；拒绝 → 返回拒绝码，不创建进程。
5. **审计**：提权请求、同意、拒绝全部写 `log.md`（账户、程序、时间、结果）。
6. **作用范围**：提权只作用于该进程，不改变会话默认令牌，不持久化。

## 5 账户管理

1. 创建 / 删除 / 改角色 / 启用停用：`admin` 可管理普通用户，`root` 可管理全部账户；**账户库属系统重要文件，任何角色都不能直接改文件**，只能经 `account_svc.service` 接口执行；`system` / `update` 只能由系统初始化创建，运行期不可删改；**`system` 也不能读写自身账户记录与令牌配置**。
2. 密码与验证因子注册走 `auth.md`；账户锁定状态由 `auth` 失败计数与账户 `state` 共同决定。
3. 账户策略与配置存 `/system/security/account/config`，受 ACL 保护；变更写审计。
4. 删除账户：先注销其在线会话（`session.md`），再级联清理 `home`、配置视图、凭据引用与授权记录。
5. 系统账户强制 `policy.allow_interactive = false`：不能从登录界面登录。

## 6 与各子系统对接

1. **os.md**：账户对象复用对象‑条目模型。
2. **acl.md**：账户角色映射权限标签与完整性级别；最终访问判定仍由 ACL 完成，账户控制不替代 ACL。
3. **auth.md**：认证确认身份；本规范提供角色、状态与策略，二者共同决定登录是否放行。
4. **session.md**：会话绑定账户；登录后按角色生成标准令牌，管理员默认批准模式。
5. **process.md**：进程令牌来源、派生继承、提权不继承、服务按运行身份取令牌。
6. **security.md**：账户控制属于安全体系，账户与提权策略纳入安全策略。
7. **update.md**：更新服务以 `update` 账户（窄而深）运行，只授予更新区与系统部件写入权限。
8. **install.md / pkg.md**：安装、卸载、系统级配置变更需要提权，走同意 / 凭据提示。
9. **ui.md**：盾牌图标、安全桌面、同意 / 凭据提示框、锁定提示。
10. **secure_island.md**：凭据与令牌密钥由安全岛（TPM 2.0）密封保护。
11. **log.md**：账户创建 / 删除 / 改角色与提权决策写入审计。
12. **hardware_svc.md**：硬件操作（MMIO / DMA / 中断 / 固件）只由 `system` 账户与内核服务经硬件服务执行；`user` / `admin` / `root` 一律不可直接操作硬件。

## 7 设计取舍

1. **系统账户与用户账户严格隔离**：系统账户不可交互登录，避免用户会话借用系统身份。
2. **`update` 独立成账户且「窄而深」**：只能碰更新区与系统部件，却可改写系统重要文件；不碰硬件与账户管理，更新链路被攻破不等于全系统沦陷。
3. **管理员默认标准令牌（管理员批准模式）**：降低日常误操作与恶意程序静默提权，代价是管理操作多一次确认。
4. **`root` 按 Linux 语义但与系统核心隔离**：用户空间全权、提权无提示，但拿不到系统重要文件与硬件；用「权限面不重叠」代替「同一账户全权」。
5. **提权不继承、只作用于单进程**：防止权限沿进程树扩散。
6. **安全桌面承载提示**：防止伪造提权窗口与输入劫持。
7. **权限面切分**：`system` 广但**不能读写自身文件**，`update` 窄而深（系统重要文件唯一写入者），`admin` 日常可用，`user` 最小，`root` 用户空间全权——五个账户权限面互不重叠。

## 8 账户控制 `.api` 契约文件（account.api 全文）

账户控制对外接口统一由以下契约文件声明，文件存放于 `/system/api/account.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# account.api 账户控制对外接口契约
version = 1.0
target = account
env = python
exec = /system/security/account/account_svc.service
internal_loc = map + symbol + register
min_os_version = 1.0
global_acl = user_foreground
global_limit = 30次/秒

object /system/api/account account

# ---- 只读条目 ----
account: account_list { } /internal/account/list
account: account_info { } /internal/account/info
account: current_token { } /internal/account/token

# ---- 可写控制条目：账户管理 ----
account: account_create {
    return self.account_create(name, role)
} /internal/account/create
account: account_delete {
    return self.account_delete(account_id)
} /internal/account/delete
account: account_set_role {
    return self.account_set_role(account_id, role)
} /internal/account/set_role
account: account_enable {
    return self.account_enable(account_id)
} /internal/account/enable
account: account_disable {
    return self.account_disable(account_id)
} /internal/account/disable
account: account_set_policy {
    return self.account_set_policy(account_id, policy)
} /internal/account/set_policy

# ---- 可写控制条目：提权 ----
account: elevate_request {
    return self.elevate_request(account_id, reason)
} /internal/account/elevate_request
account: elevate_consent {
    return self.elevate_consent(request_id)
} /internal/account/elevate_consent
account: elevate_deny {
    return self.elevate_deny(request_id)
} /internal/account/elevate_deny

# ---- 对外可监听标准事件 ----
EVENT:
    account_created(account_id: str, role: str) ｜ /internal/event/account_create
    account_role_changed(account_id: str, role: str) ｜ /internal/event/account_role
    elevation_requested(account_id: str, program: str) ｜ /internal/event/elevation_request
    elevation_granted(account_id: str, program: str) ｜ /internal/event/elevation_grant
    elevation_denied(account_id: str, program: str) ｜ /internal/event/elevation_deny

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    account_create: user_admin
    account_delete: user_admin
    account_set_role: user_admin
    account_enable: user_admin
    account_disable: user_admin
    account_set_policy: user_admin
    elevate_request: user_foreground
    elevate_consent: user_admin
    elevate_deny: user_admin
```

契约要点：

1. 契约为后台常驻服务文件 `account_svc.service` 宿主，随服务运行打包容器运行。
2. 账户管理（创建 / 删除 / 改角色 / 启停 / 策略）ACL 为 `user_admin`，普通前台进程不可调用。
3. `elevate_request` 面向任意前台进程；`elevate_consent` / `elevate_deny` 只允许 `admin` / `root`。
4. 提权只创建新进程的高令牌，不改变会话令牌，不持久化。
5. 事件供 UI 渲染盾牌 / 提示框与审计子系统消费。
6. `system` / `update` 账户不可经 API 创建、删除、改角色。
7. `elevate_request` 只授予**用户空间令牌**（最高 `high` 完整性）；系统重要文件与硬件操作不经本契约授权：系统重要文件只由 `update` 在更新路径读写，硬件只由 `system` 与硬件服务执行。