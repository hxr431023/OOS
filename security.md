# security.md 安全体系规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；安全体系为**用户态服务**与内核 ACL 协同：对象级权限由 `acl.md` 承载，应用级安全（认证、防火墙、加密、审计）由本子系统提供。
> 2. Ring1/Ring2 闲置；安全操作受 ACL 管控；敏感事件写入审计日志（`log.md` 审计分支）。
> 3. **条目不在此文档硬编码**：安全体系对外条目的集合、命名、权限由 `security.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义安全体系的行为语义，契约全文见第 8 节。
> 4. 对应目录树 `/system/security/`：`auth`（用户认证、登录程序）、`account`（账户控制、提权）、`firewall`（防火墙守护进程）、`encrypt_tools`（加密解密工具）、`audit_rules`（安全审计规则）、`config`（安全全局配置）。
> 5. 依赖：`os.md`、`acl.md`、`process.md`、`fs.md`、`log.md`、`net.md`、`pkg.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 认证 | 验证用户/程序身份的过程；登录认证、进程身份验证 |
| 授权 | 认证通过后授予访问权限；与 `acl.md` 对象权限协同 |
| 防火墙 | 网络访问控制：按规则放行/拦截入站出站流量 |
| 加密 | 数据加密与解密：磁盘加密、文件加密、传输加密 |
| 审计 | 安全事件的记录与留痕，不可被普通进程清理 |
| 安全策略 | 全局安全配置：密码策略、会话超时、防火墙默认策略等 |

## 1 安全体系构成

1. **认证与登录（`auth`）**：用户登录认证、会话管理、进程身份标记。
2. **防火墙（`firewall`）**：网络流量过滤与访问控制（联动 `net.md`）。
3. **加密工具（`encrypt_tools`）**：磁盘、文件、传输数据加密解密。
4. **审计规则（`audit_rules`）**：定义哪些事件必须审计、留存时长。
5. **全局配置（`config`）**：密码策略、会话超时、安全等级。
6. **账户控制（`account`）**：账户模型（系统账户 / 用户账户）、角色分级、令牌与完整性级别、提权同意 / 凭据提示（`account.md`）。

## 2 认证与授权

1. 用户登录经 `auth` 服务认证（密码/凭证校验），认证成功建立会话。
2. 会话关联用户身份与授权集合；进程派生继承所属用户身份。
3. 对象访问最终由 `acl.md` 校验：认证确定"你是谁"，ACL 确定"你能做什么"。
4. 会话超时、锁定策略由安全策略配置控制；认证失败连续多次触发锁定并审计。

## 3 防火墙

1. 防火墙守护进程（`/system/security/firewall`）管理网络访问控制规则（联动 `net.md` 网络管理服务）。
2. 规则按五元组/方向/协议匹配：允许、拒绝、记录；默认策略（放行/拒绝）由 `config` 配置。
3. 规则变更受 ACL 管控，仅授权进程可修改；变更写入审计日志。
4. 防火墙不改变内核协议栈，仅做流量过滤控制。

## 4 加密与校验

1. **磁盘加密**：存储管理（`storage.md`）配合加密工具对磁盘卷加密；密钥管理受保护。
2. **文件加密**：提供加密/解密工具，供用户与程序调用。
3. **传输加密**：网络数据加密能力，供网络服务与安全通道使用。
4. **完整性校验**：软件包、更新包签名与完整性校验（联动 `pkg.md`、`update.md`）。

## 5 审计

1. 审计事件覆盖：登录成功/失败、权限变更、安装/卸载、网络规则变更、硬件插拔、系统启停等（`log.md` 第 7 节审计约束）。
2. 审计日志独立存储，仅管理员可读，不可被普通进程清理。
3. 审计规则（`audit_rules`）配置留存时长与轮转策略。

## 6 容错与降级

1. 认证服务崩溃：init 重启认证服务；已建立会话短时保留，新登录中断直至恢复。
2. 防火墙崩溃：内核默认放行或按保守策略处理，防火墙重启后恢复规则。
3. 密钥丢失：磁盘加密卷无法解密，需备份密钥/恢复手段；不可暴力绕过。

## 7 与各子系统对接

1. **os.md**：安全体系服务为纯控制对象使用者，复用对象‑条目模型。
2. **acl.md**：对象访问权限由 ACL 承载；安全体系不重复实现对象权限。
3. **log.md**：安全事件写入审计日志分支。
4. **net.md**：防火墙联动网络管理服务。
5. **pkg.md / update.md**：安装、更新包完整性校验。
6. **storage.md**：磁盘加密联动存储管理。
7. **.api.md**：安全体系对外接口经 `security.api` 契约与桥接层。8. **secure_island.md**：认证与磁盘加密凭据由安全岛（TPM 2.0）密封保护。
9. **session.md**：认证通过后由会话子系统（session.md）建立会话。
10. **account.md**：账户角色、令牌、提权策略与账户管理由账户控制子系统（account.md）承载；安全策略包含账户与提权配置。

## 8 安全体系 `.api` 契约文件（security.api 全文）

安全体系对外接口统一由以下契约文件声明，文件存放于 `/system/api/security.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# security.api 安全体系对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = security
env = python
exec = /system/security/auth
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 100次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/security security

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
security: session { } /internal/security/session
security: fw_rules { } /internal/security/firewall/rules
security: audit_list { } /internal/security/audit/list

# ---- 可写控制条目：转换代码为Python，实现安全操作 ----
security: login {
    if not self.verify(username, credential):
        raise PermissionError("认证失败")
    return self.open_session(username)
} /internal/security/auth/login
security: logout { self.close_session(session_id) } /internal/security/auth/logout
security: fw_add_rule {
    self.add_rule(rule)
} /internal/security/firewall/add
security: encrypt {
    return self.encrypt(data, key_id)
} /internal/security/crypto/encrypt
security: decrypt { return self.decrypt(data, key_id) } /internal/security/crypto/decrypt

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    login_failed(username: str, count: u32) ｜ /internal/event/auth_login_fail
    fw_rule_changed(rule: str) ｜ /internal/event/fw_rule_change
    security_alert(level: str, message: str) ｜ /internal/event/security_alert

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    login: 5次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    fw_add_rule: aux_system
    encrypt: user_foreground
    decrypt: user_foreground
```

契约要点：

1. 外部程序仅能访问 `/system/api/security` 标准虚拟对象，禁止直连安全服务内部实现。
2. `login` 接口经 Python 转换代码校验凭证，失败拒绝并触发 `login_failed` 事件；登录限流防爆破。
3. 防火墙规则修改仅授权 `aux_system`；加密/解密授权前台用户。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 9 设计取舍

1. 对象权限由 ACL 统一承载，安全体系专注认证、网络控制、加密、审计，职责分离。
2. 安全事件统一进审计日志，独立留存，保障可追溯。
3. 防火墙规则变更受限流与审计，降低误操作与攻击风险。
4. 对外只暴露 `/system/api/security` 标准接口，密钥、会话等内部状态完全隐藏。
