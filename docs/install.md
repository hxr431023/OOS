# install.md 安装器规范（系统安装与应用安装，x86‑64）

> 
> 前提说明
> 
> 1. 负责操作系统的**全新安装 / 覆盖安装 / 修复安装**全流程：从引导介质启动到首次配置完成。
> 2. 安装 / 启动阶段在屏幕正中央最高图层显示**加载框（splash）**，展示安装进度、图标、程序名称（复用 `ui.md` 的 ui.splash 契约）。
> 3. 安装过程涉及磁盘分区、镜像写入、引导项写入等**不可逆高危操作**，全部接口仅授权系统级调用（`aux_system`）。
> 4. 安装介质镜像必须经过**签名与哈希校验**，未通过校验禁止写入（防篡改 / 防供应链攻击）。
> 5. 升级 / 修复场景复用安装器的镜像校验与写入能力，具体升级策略见 `update.md`。

## 1 总体约束

1. 安装器是独立于已安装系统之外的最小可执行环境（由引导介质加载），不依赖目标磁盘上的系统文件。
2. 安装操作按阶段推进，每阶段可查询状态、可中止；**分区与写入阶段一旦开始不可回退**，中止即进入失败态并给出可恢复指引。
3. 安装器界面使用图形向导（加载框 + 分步页面），同时保留命令行安装模式（`cmd.md` 终端）。
4. 首次配置产出（语言、时区、账户、键盘布局）写入系统配置表（`config.md` 的 sysconfig）。
5. 安装完成后重启进入正常开机流程（`boot.md`）。
6. **应用安装**由**安装程序**读取**安装程序文件**（安装包）执行；安装器必须支持的操作原语：**解压、提取、安装、更新配置文件、调用系统更新程序更新文件、执行安装脚本**（第 2 节），否则程序无法安装。
7. 本规范覆盖**系统安装**（第 3 节）与**应用安装**（第 2 节）：系统安装面向 OS 部署，应用安装面向安装程序文件（应用 / 库 / 配置 / 系统文件更新）。
8. **系统文件边界**：应用安装可直接写入应用与数据区域；系统文件（`/system`）的更新必须经系统更新程序（`update.md`）签名校验，安装器调用更新接口完成，不直接逐文件写系统目录。

## 2 应用安装（安装程序与安装程序文件）

1. **安装程序**（installer）：通用应用安装引擎（向导 + 命令行），读取**安装程序文件**并按其声明执行安装；与系统安装共用本规范。
2. **安装程序文件**（安装包 `.apkg`）：包含**安装清单**（协议面：应用标识、版本、依赖、目标目录、操作序列、权限声明）与**载荷**（文件 / 压缩包 / 脚本 / 配置模板）。
3. **读取与保存**：安装程序解析安装清单，将协议面**保存**为已安装记录（供卸载 / 修复 / 升级比对）。
4. **操作原语支持范围**（安装器必须全部支持，否则程序无法安装）：
   1. **解压**：压缩载荷解压到临时区；
   2. **提取**：从安装包提取文件到目标目录（应用 / 数据区域）；
   3. **安装**：部署可执行对象（`.exeo`）、动态链接库对象（`.dllo` / `.uio`）、资源文件；
   4. **更新配置文件**：写入 / 合并应用配置（`config.md`）；
   5. **调用系统更新程序**：更新系统共享文件或部件，经 `update.md` 签名校验执行；
   6. **执行安装脚本**：安装后置命令、注册服务 / 开机项（脚本来源经签名校验，受 ACL 约束）。
5. **系统文件边界**：应用安装直接写入应用与数据区域；**系统文件（`/system`）的更新必须经系统更新程序（`update.md`）签名校验**，安装器通过调用更新接口完成，不直接逐文件写系统目录。
6. **卸载 / 修复 / 升级**：依据已保存的安装清单反向执行（删除文件、回滚配置、注销服务）。

## 3 系统安装流程（7 步）

1. **引导加载**：从安装介质启动，加载安装器内核与镜像（显示加载框）。
2. **语言与时区选择**：写入临时配置，供安装界面与首次配置使用。
3. **分区规划**：选择目标磁盘，创建分区布局（系统分区 / 数据分区 / 交换分区），可预览与修改。
4. **镜像校验**：读取安装镜像，校验哈希与数字签名；失败则终止并提示更换介质。
5. **写入系统**：格式化分区、写入系统镜像与内核、写入引导程序。**系统镜像校验后整体写入（不逐文件解压 / 复制）**；系统文件不参与安装器的常规文件操作（第 1 节第 6 条）。
6. **写入引导项**：向启动管理器注册新系统（含恢复 / 修复入口）。
7. **首次配置**：创建用户账户、确认时区语言、选择隐私与更新策略；完成后重启进入系统。

> 
> 修复安装（repair）流程：从引导介质启动后选择"修复"，跳过分区与首次配置，仅校验 + 覆盖系统文件 + 修复引导项。

> 修复安装的覆盖对象为系统文件整体（镜像级校验后整体覆盖），不做逐文件复制 / 提取；个别系统部件更新走 `update.md` 部件级更新。

## 4 安装器组件

| 组件 | 作用 |
| --- | --- |
| `installer` | 安装器主程序（向导流程控制） |
| `part_tool` | 分区规划与写入工具 |
| `img_verify` | 镜像哈希与签名校验工具 |
| `img_writer` | 系统镜像写入、引导项写入 |
| `first_boot_cfg` | 首次配置向导（账户 / 时区 / 策略） |

## 5 安全要求

1. 安装镜像发布时计算 SHA‑256 哈希并签名；安装器内嵌发布者公钥，校验失败即终止。
2. 分区与写入接口仅 `aux_system` 可调用；安装器进程不运行任何用户可写代码。
3. 引导项写入校验目标磁盘是否为受支持的启动介质类型，防止误写数据盘。
4. 首次配置的账户密码散列后存储（由 `secure_island.md` 安全岛保护），明文不落盘。
5. 安装操作原语（解压 / 提取 / 安装 / 更新配置 / 调用系统更新 / 执行脚本）全部受 ACL 约束并写入审计日志（`log.md`）；安装包载荷与脚本经签名校验；**系统文件更新一律经系统更新程序（`update.md`）签名校验**，安装器不直接写 `/system`。

## 6 和其它文档对接关系

1. **对接 boot.md**：安装完成后重启进入开机流程；引导项由安装器写入。
2. **对接 storage.md**：分区规划与格式化调用存储子系统能力。
3. **对接 config.md**：首次配置写入 sysconfig（语言 / 时区 / 账户 / 策略）。
4. **对接 ui.md**：安装过程使用加载框（ui.splash）与向导页面（ui.panel）。
5. **对接 update.md**：在线升级复用镜像校验 / 写入能力，策略见 update.md。
6. **对接 secure_island.md**：首次配置生成的凭据由安全岛保护。
7. **对接 exeo.md / dllo.md / uio.md**：应用安装部署可执行对象与动态链接库对象（含图形界面专用 uio）。
8. **对接 process.md**：安装脚本执行与服务注册受进程与 ACL 约束。

## 7 设计取舍

1. 安装器做成独立最小环境，不与目标系统混装，保证任何状态下都可修复。
2. 分区阶段允许人工干预，镜像写入阶段全自动、不可中断（防止半写入损坏）。
3. 校验优先于写入：宁可拒绝安装，不装一个被篡改的系统。

## 8 安装器 `.api` 契约文件（install.api 全文）

安装器对外接口统一由以下契约文件声明，文件存放于 `/system/api/install.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# install.api 系统安装器对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = install
# 转换代码运行环境
env = python
# 所属可执行文件命名空间（安装器主程序）
exec = /system/install/installer
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 10次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/install install

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
install: install_status { } /internal/install/status
install: disk_layout { } /internal/install/disk
install: install_log { } /internal/install/log
install: pkg_manifest { } /internal/install/pkg_manifest

# ---- 可写控制条目：转换代码为Python，实现安装流程控制 ----
install: verify_image {
    return self.verify_image(path)
} /internal/install/verify
install: partition_apply {
    return self.partition_apply(layout)
} /internal/install/partition
install: install_begin {
    return self.install_begin(cfg)
} /internal/install/begin
install: install_abort {
    return self.install_abort()
} /internal/install/abort
install: install_finish {
    return self.install_finish()
} /internal/install/finish
install: app_install { return self.app_install(pkg) } /internal/install/app_install
install: app_uninstall { return self.app_uninstall(app_id) } /internal/install/app_uninstall

# ---- 对外可监听标准事件（安装进度与结果） ----
EVENT:
    install_started(phase: str) ｜ /internal/event/install_start
    install_progress(phase: str, percent: int) ｜ /internal/event/install_progress
    install_finished() ｜ /internal/event/install_finish
    install_failed(phase: str, reason: str) ｜ /internal/event/install_fail

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    install_status: aux_system
    disk_layout: aux_system
    app_install: aux_system
```

契约要点：

1. 安装器全部接口默认 `aux_system`：安装是最高权限操作，普通应用不可触达。
2. `verify_image` 必须先于 `partition_apply` / `install_begin`；未校验通过禁止写入。
3. `install_abort` 仅可在非写入阶段中止；分区 / 写入阶段中止即失败态。
4. 进度事件供加载框与安装向导实时显示。
5. 应用安装经 pp_install 读取安装程序文件（安装包）清单，按操作原语（解压 / 提取 / 安装 / 更新配置 / 调用系统更新 / 执行脚本）执行；卸载经 pp_uninstall（第 2 节）。
