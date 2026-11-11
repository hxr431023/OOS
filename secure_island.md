# secure_island.md 安全岛与 TPM 2.0 规范

> 
> 前提说明
> 
> 1. **安全岛（secure island）**：由硬件信任根（TPM 2.0）保护的**隔离存储与执行区**，用于存放密码凭据、系统密钥与隐私数据；非授权组件（含内核其余部分）不可直接读取。
> 2. 信任根为 **TPM 2.0**：密钥分层（SRK / 存储密钥 / 应用密钥）、平台配置寄存器（PCR）完整性度量、密封（Sealing）与远程证明（Attestation）。
> 3. 系统登录 / 解锁凭据散列、磁盘加密密钥、平台签名密钥等全部密封存放于安全岛（见 `security.md` / `config.md`）。
> 4. 安全岛接口为**全系统最高敏感接口**，仅 `aux_system` 可调用，全部操作留审计日志（见 `log.md`）。
> 5. 与"安全模式 / 恢复"不同：安全岛在任何模式下都不可被软件绕过，除非物理重置 TPM。

## 1 总体约束

1. 安全岛存储受 **TPM 2.0 Sealing** 保护：密封数据仅当平台 PCR 状态与策略匹配时才能解封（unseal）。
2. 密钥层次：`TPM 主密钥（SRK，不可导出）→ 存储根密钥 → 各组件密封密钥`；应用密钥可导出但受策略约束。
3. 凭据只以**散列 / 密封形式**存在，安全岛内不存明文密码。
4. 启动完整性：引导链各阶段度量写入 PCR（见 `boot.md`），篡改导致解封失败 → 安全岛拒绝放行凭据。
5. 远程证明：向外部服务证明平台可信（PCR 引用签名），用于企业 / 云登录等场景。
6. 安全岛数据随 TPM 密钥迁移策略备份（见 `backup.md`），物理 TPM 更换需重新封装。

## 2 安全岛对象条目布局

### 安全岛对象，路径 `/object/secure_island/main`

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `tpm_status` | 只读 | TPM 存在 / 版本 / 厂商 / PCR 数量 |
| `pcr_read` | 只读 | 读取指定 PCR 度量值 |
| `island_status` | 只读 | 安全岛已密封条目 / 最近访问 / 完整性状态 |
| `key_list` | 只读 | 安全岛内密钥句柄列表（不含密钥内容） |
| `seal` | 只写 | 密封数据（绑定当前 PCR 策略） |
| `unseal` | 只写 | 解封数据（PCR 匹配才返回） |
| `store` | 只写 | 存储受保护凭据（内部散列 + 密封） |
| `load` | 只写 | 读取受保护凭据（返回前校验调用方） |
| `wipe` | 只写 | 清除安全岛内容（需二次确认 + 审计） |
| `attest` | 只写 | 发起远程证明（生成 PCR 引用签名） |

## 3 核心操作流程示例

### 示例 1：首次配置生成登录凭据（安装器）

1. 安装向导收集用户密码（`install.md` 首次配置）；
2. 安全岛内散列 + 密封存储（绑定当前安装环境的 PCR）；
3. 明文密码进程内存即清零；此后登录校验走安全岛解封。

### 示例 2：登录认证

1. 用户输入口令 → auth 请求安全岛校验；
2. 安全岛校验 PCR 匹配后解封凭据散列，比对结果返回 auth（仅返回布尔结果，不返回散列）；
3. 成功 → 建立会话（`session.md`）；失败计数与审计（`log.md`）。

### 示例 3：平台完整性验证（启动）

1. 引导链每阶段度量写入 PCR（`boot.md`）；
2. 内核初始化安全岛，验证 PCR 与预期值；
3. 不匹配 → 解封失败，敏感凭据不可用，进入受限模式。

## 4 和其它文档对接关系

1. **对接 security.md**：auth 认证 / 解锁凭据由安全岛存储与校验；防火墙 / 加密工具密钥入岛。
2. **对接 config.md**：sysconfig 中的敏感项（凭据、加密密钥）由安全岛密封保护，配置表只存引用。
3. **对接 boot.md**：引导链 PCR 度量与安全岛初始化时机。
4. **对接 session.md**：会话解锁口令经安全岛校验。
5. **对接 install.md**：首次配置凭据生成即入岛。
6. **对接 backup.md**：安全岛数据随 TPM 迁移策略备份。
7. **对接 log.md**：安全岛全部操作审计（访问 / 解封失败 / 擦除）。

## 5 设计取舍

1. 全部接口 `aux_system`：宁严勿宽，安全岛不向普通应用开放直接接口。
2. 凭据不落明文：即使存储介质被读走，无 TPM / PCR 匹配也解不开。
3. 解封失败降级为受限模式而非崩溃：保证系统可进入恢复流程。

## 6 安全岛 `.api` 契约文件（secure_island.api 全文）

安全岛对外接口统一由以下契约文件声明，文件存放于 `/system/api/secure_island.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# secure_island.api 安全岛与 TPM 2.0 对外接口契约
version = 1.0
target = secure_island
env = python
exec = /system/security/island/island_svc
internal_loc = map + symbol + register
min_os_version = 1.0
global_acl = aux_system
global_limit = 10次/秒

object /system/api/secure_island secure_island

# ---- 只读条目 ----
secure_island: tpm_status { } /internal/island/tpm
secure_island: pcr_read { } /internal/island/pcr
secure_island: island_status { } /internal/island/status
secure_island: key_list { } /internal/island/keys

# ---- 可写控制条目（全部仅 aux_system） ----
secure_island: island_seal {
    return self.island_seal(data, policy)
} /internal/island/seal
secure_island: island_unseal {
    return self.island_unseal(handle)
} /internal/island/unseal
secure_island: island_store {
    return self.island_store(kind, blob)
} /internal/island/store
secure_island: island_load {
    return self.island_load(kind)
} /internal/island/load
secure_island: island_wipe {
    return self.island_wipe(confirm)
} /internal/island/wipe
secure_island: island_attest {
    return self.island_attest(challenge)
} /internal/island/attest

# ---- 对外可监听标准事件 ----
EVENT:
    island_accessed(op: str) ｜ /internal/event/island_access
    island_wiped() ｜ /internal/event/island_wipe
    tpm_error(code: int) ｜ /internal/event/island_tpm

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    island_unseal: aux_system
    island_load: aux_system
    island_wipe: aux_system
```

契约要点：

1. 安全岛全部接口默认 `aux_system` 且全局限流 10 次/秒：全系统最高敏感接口。
2. `island_seal` / `island_unseal` 对：密封绑定 PCR 策略，解封前校验平台完整性。
3. `island_store` / `island_load` 用于凭据与密钥的受保护存取，返回前校验调用方。
4. `island_wipe` 需二次确认，触发后广播 `island_wiped` 并全程审计。
5. `island_attest` 生成 PCR 引用签名，供远程证明。
