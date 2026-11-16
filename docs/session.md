# session.md 会话与登录规范

> 
> 前提说明
> 
> 1. 会话（session）是从用户通过认证到用户离开之间的完整生命周期：登录 → 桌面会话 → 注销 / 锁屏 / 切换。
> 2. 登录认证由 `security.md` 的 auth 完成；本规范只负责会话的建立、隔离、切换与销毁。
> 3. 每个会话拥有**独立的桌面、配置视图与权限上下文**，进程归属到会话，会话销毁时回收其全部进程（见 `process.md`）。
> 4. 锁屏只隐藏界面与挂起输入，不销毁会话；解锁必须重新认证。
> 5. 多用户快速切换：保留当前会话在后台，新用户建立新会话。

## 1 总体约束

1. 同一时刻允许一个前台活动会话 + 多个后台会话；后台会话的进程不因失焦被杀（由内存策略决定是否换出，见 `memory.md`）。
2. 会话绑定安全凭据上下文：会话内进程的 ACL 判断基于该会话用户 + 会话级权限标签。
3. 注销 = 销毁会话：结束该会话全部进程 → 释放会话资源 → 回到登录界面。
4. 锁屏 = 保留会话：桌面合成停止输出、输入仅放行解锁通道（见 `ui.md` 图层）。
5. 会话管理器（session_manager）是会话的唯一权威，开机流程在登录界面之前启动（见 `boot.md`）。

## 2 会话生命周期

1. **登录界面**：系统启动到登录界面（或注销后返回），等待认证。
2. **认证**：auth 校验凭据（密码 / 生物 / 安全岛凭据，见 `secure_island.md`）→ 通过后返回用户身份。
3. **会话建立**：session_manager 创建会话上下文（桌面、配置视图、权限），启动该用户桌面。
4. **活动**：会话运行中，可发生锁屏 / 解锁 / 切换 / 注销。
5. **注销 / 会话销毁**：结束全部会话进程，清理资源，回到登录界面。

## 3 会话对象条目布局

### 会话对象，路径 `/object/session/<session_id>`

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `state` | 只读 | 会话状态：active / background / locked / closing |
| `user` | 只读 | 会话用户身份 |
| `processes` | 只读 | 会话内进程列表 |
| `lock` | 等待可读 / 只写 | 写=锁定；读=等待解锁（解锁后返回） |
| `cfg` | 可读可写 | 会话级配置视图 |

## 4 核心操作流程示例

### 示例 1：登录建立会话

1. auth 完成认证，返回用户身份；
2. session_manager 创建会话对象，启动桌面；
3. 系统切换到该会话图层，加载框（splash）关闭，进入桌面。

### 示例 2：快速切换用户

1. 当前会话标记为 background（保留进程）；
2. 回到登录界面，新用户认证后建立新会话；
3. 切回时旧会话恢复 foreground，无需重新登录。

## 5 和其它文档对接关系

1. **对接 boot.md**：开机流程的"登录界面"步骤由本规范承接。
2. **对接 auth.md**：认证（auth）在会话建立 / 解锁前调用（`auth.md`）；安全策略由 `security.md` 管控。
3. **对接 ui.md**：锁屏图层、登录界面、桌面切换由窗口合成器配合。
4. **对接 process.md**：进程归属会话，会话销毁级联结束进程。
5. **对接 config.md**：会话级配置视图（sysconfig / appconf 按用户隔离）。
6. **对接 secure_island.md**：登录凭据校验与解锁口令由安全岛保护。
7. **对接 account.md**：会话绑定账户；登录后按账户角色生成标准令牌，管理员默认批准模式（account.md）。

## 6 设计取舍

1. 会话与认证分离：认证只管"验身份"，会话只管"生命周期"，互不耦合。
2. 快速切换保留后台会话换取切换速度，代价是内存占用，交由内存策略平衡。
3. 锁屏不销毁会话：避免用户数据与状态丢失。

## 7 会话 `.api` 契约文件（session.api 全文）

会话对外接口统一由以下契约文件声明，文件存放于 `/system/api/session.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# session.api 会话与登录对外接口契约
version = 1.0
target = session
env = python
exec = /system/desktop/session/session_mgr
internal_loc = map + symbol + register
min_os_version = 1.0
global_acl = user_foreground
global_limit = 50次/秒

object /system/api/session session

# ---- 只读条目 ----
session: session_list { } /internal/session/list
session: current_session { } /internal/session/current

# ---- 可写控制条目：会话生命周期管理 ----
session: session_login {
    return self.session_login(user, cred)
} /internal/session/login
session: session_logout {
    return self.session_logout()
} /internal/session/logout
session: session_lock {
    return self.session_lock()
} /internal/session/lock
session: session_unlock {
    return self.session_unlock(cred)
} /internal/session/unlock
session: session_switch {
    return self.session_switch(session_id)
} /internal/session/switch

# ---- 对外可监听标准事件 ----
EVENT:
    session_created(user: str) ｜ /internal/event/session_create
    session_destroyed(user: str) ｜ /internal/event/session_destroy
    session_locked() ｜ /internal/event/session_lock
    session_unlocked() ｜ /internal/event/session_unlock
    session_switched(session_id: str) ｜ /internal/event/session_switch

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    session_login: user_foreground
    session_logout: user_foreground
    session_switch: user_foreground
```

契约要点：

1. 会话生命周期（登录 / 注销 / 锁 / 解锁 / 切换）面向当前用户，ACL 为 `user_foreground`。
2. `session_unlock` / `session_login` 需携带安全凭据，由 auth 校验。
3. 会话事件供登录界面、锁屏界面与程序自动化感知。
