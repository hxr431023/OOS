# auth.md 登录与验证组件规范（x86‑64）

> 前提：
> 1. **登录与验证组件**（auth）对应 Windows Hello 与 Windows 安全中心的登录 / 验证能力：凭据校验、生物特征验证、账户保护。
> 2. 部署于 `/system/security/auth/`（`dirtree.md`），由两个可执行对象组成：**登录程序** `login_app`（可执行对象 `.exeo`，开机登录阶段由 `boot.md` 拉起）与**验证服务** `auth_svc.service`（后台常驻**服务文件** `.service`，由服务文件加载器加载、在服务打包容器中运行）；会话建立由 `session.md` 完成。
> 3. 凭据与密钥材料由安全岛（`secure_island.md`，TPM 2.0）密封保护；生物特征比对本地完成，不外发。
> 4. 依赖：`session.md`、`secure_island.md`、`security.md`、`input.md`、`cam.md`、`ui.md`、`acl.md`、`log.md`、`.api.md`。

## 1 定位与职责

1. **凭据校验**：密码、PIN、生物特征（指纹 / 人脸 / 虹膜）、安全密钥验证；
2. **登录验证**：登录界面提交凭据 → 校验 → 返回用户身份（交 `session.md` 建立会话）；
3. **账户保护**：连续失败计数、锁定与冷却提示（类 Windows 安全中心）；
4. **凭据管理**：注册 / 更新 / 删除验证因子（受 ACL 与安全岛保护）；
5. **多因素验证**：密码 + PIN / 生物特征组合。

**程序构成**：

1. **`login_app`（登录程序）**：可执行对象（`.exeo`），开机进入登录界面阶段由 `boot.md` 拉起；显示登录界面、收集凭据并调用 `auth_svc.service` 完成验证；登录成功后交 `session.md` 建立会话。它是调用方，不暴露对象条目。
2. **`auth_svc.service`（验证服务文件）**：后台常驻**服务文件**（`.service`），由服务加载器加载（`service.md`），按"不冲突即打包"策略在**服务运行打包容器**中运行（每个服务一个线程，类 Windows svchost.exe）；承接登录验证、程序调用与凭据管理，是 `.api` 契约的宿主可执行对象。

两个对象共享同一套验证对象与凭据存储，登录与程序调用走同一验证逻辑。

## 2 验证因子

1. **密码**（password）：文本凭据，安全岛密封存储，不落明文。
2. **PIN**：设备本地短凭据（类 Windows Hello PIN），绑定设备与安全岛。
3. **生物特征**（biometric）：指纹（`input.md` 指纹设备）、人脸（`cam.md` 摄像头采集）、虹膜；模板密封存储，比对本地完成。
4. **安全密钥**（security key）：外接硬件密钥（FIDO 类）。
5. **组合策略**：单因子 / 多因子（密码 + PIN、生物 + PIN），由 `security.md` 策略配置。

## 3 验证对象条目

验证以**用户验证对象** `/object/auth/<user_id>`（纯控制对象）承载，条目：

1. `user`：只读，用户标识。
2. `factors`：只读，已注册验证因子列表（password / pin / fingerprint / face / iris / key）。
3. `state`：只读，账户状态（normal / locked / protected）。
4. `fail_count`：只读，连续失败次数。
5. `last_login`：只读，最近成功登录时间。

## 4 验证流程

1. **提交**：登录界面（`desktop/session`）提交因子类型与凭据。
2. **校验**：格式校验 → 安全岛解封对照（密码 / PIN）或本地模板比对（生物特征）→ 返回布尔结果。
3. **失败处理**：失败计数 +1；达阈值锁定账户并显示提示（类安全中心），冷却期内拒绝。
4. **成功**：返回用户身份 → `session.md` 建立会话；计数清零。
5. **审计**：验证结果写入 `log.md`（只记结果与因子类型，不记凭据原文）。

## 5 与各子系统对接

1. **session.md**：登录建立会话、锁屏 / 解锁调用验证。
2. **secure_island.md**：凭据密封存储、TPM 2.0 解封、PCR 度量。
3. **security.md**：账户保护策略、失败阈值、审计规则。
4. **input.md**：指纹设备事件、键盘输入（PIN）。
5. **cam.md**：人脸 / 虹膜采集（仅本地比对，不外发）。
6. **ui.md**：登录界面、验证提示框、锁定提示。
7. **acl.md**：凭据注册 / 删除权限管控。
8. **log.md**：验证审计（不含凭据原文）。
9. **account.md**：认证通过后按账户角色与状态放行；账户锁定与提权策略由账户控制（account.md）承载。

## 6 设计取舍

1. 生物特征模板与比对全部本地完成（采集与模板不出设备），接口只返回布尔结果。
2. 凭据原文不落盘、不进日志；密码 / PIN 由安全岛密封，解封仅在验证瞬间。
3. 锁定策略类 Windows 安全中心：失败计数 + 冷却时间，不永久锁死（可管理员解锁）。
4. 不引入云端账户体系；验证组件只做本地设备登录验证。

## 7 登录与验证组件 `.api` 契约文件（auth.api 全文）

```text
# auth.api 登录与验证组件对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = auth
env = python
exec = /system/security/auth/auth_svc.service
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 20次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/auth auth

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
auth: factors { } /internal/auth/factors
auth: state { } /internal/auth/state
auth: fail_count { } /internal/auth/fail_count
auth: last_login { } /internal/auth/last_login

# ---- 可写控制条目：转换代码为Python，实现验证与凭据管理 ----
auth: verify { return self.verify(user_id, factor, credential) } /internal/auth/verify
auth: register_factor { return self.register_factor(user_id, factor, data) } /internal/auth/register
auth: remove_factor { return self.remove_factor(user_id, factor) } /internal/auth/remove
auth: lock { return self.lock(user_id, reason) } /internal/auth/lock
auth: unlock { return self.unlock(user_id) } /internal/auth/unlock

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    auth_succeeded(user_id: str, factor: str) ｜ /internal/event/auth_succeeded
    auth_failed(user_id: str, factor: str, reason: str) ｜ /internal/event/auth_failed
    account_locked(user_id: str, until: u64) ｜ /internal/event/account_locked

# ---- 单接口独立限流规则（覆写全局限流，防暴力破解） ----
LIMIT_OVERRIDE:
    verify: 5次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    register_factor: user_admin
    remove_factor: user_admin
```

契约要点：

1. 只读条目透传因子列表、账户状态、失败计数、最近登录时间；写条目为验证与凭据管理命令。
2. `verify` 独立限流 5 次/秒（防暴力破解）；验证结果只返回布尔值，不含凭据原文。
3. 账户锁定 / 解锁事件供会话与安全中心提示（`session.md`、`ui.md`）。
4. 凭据注册 / 删除需管理员权限（`user_admin`），受 `acl.md` 与安全岛双重保护。
5. 契约宿主为后台常驻服务文件 `auth_svc.service`；开机登录程序 `login_app` 通过同一契约的 `verify` 完成登录验证，不单独暴露接口。