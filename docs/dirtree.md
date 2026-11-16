# dirtree.md 系统目录树（/system）

> 
> 前提说明
> 
> 
> 1. 本文件为 `/system` 单根统一目录树的**唯一权威版本**，由 `fs.md` 分离独立。
> 2. 各子系统目录（kernel、devmgr、desktop、userland、cmd、hwsched、log、pkg、media、security、network、update、help、vm、storagemgr、backup、suspend、asm、api、filemgr、linux_compat、objmgr）分别对应 `os/specs` 下各子系统规范文档。
> 3. 文档中涉及目录结构处一律以本文件为准；改动目录须同步维护本文件。
> 4. 依赖：`fs.md`（VFS 与目录挂载语义）、`os.md`（对象模型）。

## 图例

| 图标 | 含义 |
| --- | --- |
| 🟢 | 可执行程序 / 二进制 |
| ⚙️ | 配置文件 / 模板 |
| 📦 | 库文件 |
| 🖼️ | 图形 / 媒体资源 |
| 💾 | 运行时数据 |
| ❗ | 特殊（如成品系统为空） |
| 📖 | 人读文档 |

## /system 目录树

/system
├─ kernel/
│  ├─ core/                 🟢 内核主镜像、内核扩展二进制
│  ├─ runtime/              ⚙️🖼️ 开机画面、内核参数模板
│  ├─ modules/              🟢 Ring0内核硬件服务模块
│  ├─ rtc/                  🟢 时钟与时间服务（RTC/墙上/单调/定时器）
│  └─ aux_modules/          🟢 内核附属功能模块（含嵌入内核态的第一方服务）
├─ ipc/                      📦 进程间通信（IPC）实现代码（所有进程与进程之间通信）
│  ├─ core/                  🟢 IPC 核心（对象-条目承载与事件等待框架，含事件信号）
│  ├─ mq/                    🟢 消息队列实现
│  ├─ shm/                   🟢 共享内存实现
│  ├─ sem/                   🟢 信号量实现
│  ├─ fifo/                  🟢 命名管道实现
│  ├─ sock/                  🟢 域套接字实现
│  ├─ sig/                   🟢 进程信号实现
│  ├─ mirror/                🟢 定期镜像同步实现
│  ├─ dual/                  🟢 双向映射实现
│  └─ config/                ⚙️ IPC 全局配置
├─ devmgr/
│  ├─ manager/              🟢 设备管理器主程序
│  ├─ match_rules/          ⚙️ 硬件服务匹配规则
│  └─ config/                ⚙️ 设备管理配置
├─ input/                   📦 输入子系统（事件采集/归一化/焦点分发/注入）
│  ├─ core/                 🟢 输入核心（事件链、焦点分发、注入、IME接入）
│  └─ config/               ⚙️ 输入配置（键位映射、全局热键表）
├─ desktop/
│  ├─ compositor/           🟢 窗口合成器可执行
│  ├─ launcher/             🟢 桌面启动器程序
│  ├─ start_menu/           🟢 开始菜单（类 Win10：应用列表/固定项/搜索/电源）
│  ├─ widgets/              🟢 状态栏、小组件二进制
│  ├─ floating/             🟢 悬浮窗口组件（悬浮球等，常驻顶层）
│  ├─ notifications/        🟢 通知中心（类 Windows 通知）
│  ├─ session/              🟢 会话与登录管理器（登录/注销/锁屏/切换）
│  ├─ clipboard/            🟢 剪贴板服务（多格式/延迟渲染/所有权）
│  ├─ layer_table/          ⚙️ 图层表（记录哪个对象/空间在哪个图层）
│  ├─ splash/                🟢 加载框（安装/启动时正中央最高图层加载窗口）
│  ├─ settings_panel/        🟢 设置框（左侧面板：标题/解释/选择按钮）
│  ├─ msgbox/                🟢 消息弹窗（类 Windows 7：标题/图标/文本/按钮，模态）
│  ├─ uio/                  📦 图形界面专用动态链接库对象（UI DLLO，进程内直接链接）
│  ├─ themes/               🖼️ 壁纸、图标、窗口样式资源（Win11 圆角默认 / Win10 直角）
│  └─ config/               ⚙️ 桌面配置
├─ userland/
│  ├─ app_launcher/         🟢 普通应用启动器程序
│  ├─ runtime_libs/         📦 用户态系统基础库
│  ├─ services/
│     ├─ manager/            🟢 服务管理器（命令行 + 图形界面，类 SCM/sc + services.msc）
│     ├─ loader/             🟢 服务加载器（二进制程序，加载 .service 服务文件）
│     ├─ host/               🟢 服务运行打包容器（多服务打包一进程/每服务一线程，类 svchost）
│     ├─ daemons/           🟢 全部常驻后台系统守护进程
│     └─ config/            ⚙️ 服务自启、权限配置
│  └─ trigger/              🟢 触发任务器（类 Windows 任务计划程序：按计划/条件触发任务）
│     ├─ trigger_task/      🟢 触发任务器主程序
│     └─ config/            ⚙️ 触发任务、计划配置
├─ dllo/                     📦 动态链接库对象（DLLO）子系统
│  ├─ loader/               🟢 动态链接器 / 加载器（依赖图解析、符号解析、重定位）
│  ├─ objects/              ⚙️ 已登记 DLLO 对象元数据
│  └─ config/               ⚙️ 加载策略、白名单配置
├─ exeo/                     📦 可执行对象（.exeo）子系统
│  ├─ exeo_mgr/             🟢 可执行对象管理器（开机登记固定对象、启动/参数修改）
│  └─ config/               ⚙️ 可执行对象白名单、策略配置
├─ tpmeo/                    📦 临时文件对象（.tpmeo）子系统
│  ├─ tpmeo_mgr/            🟢 临时文件对象管理器（创建/读写/过期清理）
│  └─ config/               ⚙️ 临时目录、默认 ttl 配置
├─ cmd/
│  ├─ terminal/             🟢 图形终端程序
│  ├─ builtin_cmds/         🟢 内置基础命令可执行
│  ├─ tools/                🟢 命令行工具集二进制
│  ├─ script_interp/        🟢 脚本解释器可执行
│  ├─ config/               ⚙️ 终端配色、别名配置
│  └─ scripts/              🟢 系统可运行脚本
├─ hwsched/
│  ├─ cpu/
│  │  ├─ scheduler/         🟢 CPU调度辅助用户态程序
│  │  ├─ policies/          ⚙️ CPU调度策略配置
│  │  └─ monitor/           🟢 CPU负载监控工具
│  └─ memory/
│     ├─ manager/           🟢 内存调度辅助用户态程序
│     ├─ policies/          ⚙️ 内存挤压、配额策略配置
│     └─ monitor/           🟢 内存监控、OOM观测工具
├─ log/
│  ├─ logger_daemon/        🟢 日志持久化守护进程
│  ├─ viewer/               🟢 图形+命令行日志查看工具
│  ├─ storage_rules/        ⚙️ 日志轮转、留存规则
│  └─ config/               ⚙️ 日志全局配置
├─ pkg/
│  ├─ pkg_manager/          🟢 软件包管理器主程序
│  ├─ sources/              ⚙️ 软件源配置
│  ├─ install_rules/        ⚙️ 软件安装权限规则
│  └─ cache_template/       ⚙️ 包缓存目录模板
├─ media/                   📦🖼️ 系统媒体资源库：字体、系统音效、光标、音视频解码库
│  ├─ fonts/                 📦 系统字体文件（按字体族组织）
│  │  ├─ font_loader/        🟢 字体加载器（加载字体文件）
│  │  ├─ font_resmap/        🟢 字体资源映射器（序号↔字体资源，记录位置与格式）
│  │  └─ char_mapper/        🟢 字符映射器（字符→字形映射，类似 Windows 字符映射表）
│  ├─ sounds/                📦 系统音效资源
│  ├─ cursors/               📦 光标资源
│  └─ decoders/              📦 音视频解码库
├─ security/
│  ├─ auth/
│  │  ├─ login_app/           🟢 开机登录程序（登录界面/凭据提交，.exeo 可执行对象）
│  │  ├─ auth_svc.service     🟢 后台常驻验证服务文件（服务加载器加载/打包容器运行）
│  │  └─ config/             ⚙️ 验证因子策略、锁定阈值配置
│  ├─ account/
│  │  ├─ account_svc.service  🟢 后台常驻账户控制服务文件（账户/角色/提权，服务加载器加载/打包容器运行）
│  │  └─ config/             ⚙️ 账户策略、提权策略配置
│  ├─ island/               🟢 安全岛（TPM 2.0 密封存储/解封、PCR 度量、远程证明）
│  ├─ firewall/              🟢 防火墙守护进程
│  ├─ encrypt_tools/        🟢 加密解密工具二进制
│  ├─ audit_rules/          ⚙️ 安全审计规则
│  └─ config/               ⚙️ 安全全局配置
├─ network/
│  ├─ net_daemon/           🟢 网络管理后台守护进程
│  ├─ diag_tools/           🟢 ping、路由、抓包等网络诊断工具
│  ├─ dns_proxy/            🟢 DNS代理服务程序
│  └─ config/               ⚙️ IP、DNS全局配置模板
├─ config/
│  ├─ settings/                 ⚙️ 设置与控制面板
│  │  ├─ settings_app/          🟢 设置应用（查看与更改系统配置表）
│  │  ├─ control_panel/         🟢 控制面板（查看与更改程序配置目录，向程序提供 API）
│  │  └─ config/                ⚙️ 设置与控制面板配置
│  ├─ sysconfig/                ⚙️ 系统配置表（类 Windows 注册表，系统级）
│  ├─ appconf/                  ⚙️ 程序配置目录（类 Linux /etc，程序级）
│  └─ locale/                   ⚙️ 本地化配置（语言/区域/键盘布局/语言包）
├─ update/
│  ├─ update_service/       🟢 系统更新后台守护
│  ├─ checker/              🟢 更新检测工具
│  ├─ rollback_rules/       ⚙️ 更新回滚策略
│  └─ verify/               🟢 更新包签名校验工具
├─ install/                 📦 系统安装器（引导介质加载的最小安装环境）
│  ├─ installer/            🟢 安装器主程序（向导流程控制）
│  ├─ part_tool/            🟢 分区规划与写入工具
│  ├─ img_verify/           🟢 镜像哈希与签名校验工具
│  ├─ img_writer/           🟢 系统镜像 / 引导项写入工具
│  └─ first_boot_cfg/       🟢 首次配置向导（账户 / 时区 / 策略）
├─ help/
│  ├─ cmd_manual/           ⚙️ 命令手册（人读文档）
│  ├─ api_docs/             ⚙️ 📖【人读API参考文档，md/html】
│  ├─ service_guide/         ⚙️ 服务开发指南文档
│  ├─ linux_compat_doc/     ⚙️ Linux兼容层使用文档
│  └─ viewer/               🟢 帮助文档阅读器程序
├─ vm/
│  ├─ vm_manager/           🟢 虚拟机管理用户态程序
│  ├─ templates/            ⚙️ 虚拟机硬件模板配置
│  ├─ snapshots/            ⚙️ 快照策略模板
│  ├─ net_virt/             ⚙️ 虚拟网络默认配置
│  └─ config/               ⚙️ 虚拟机全局配额、权限配置
├─ storagemgr/
│  ├─ storage_daemon/       🟢 存储管理守护进程
│  ├─ raid_ctrl/            🟢 RAID管理工具
│  ├─ disk_health/          🟢 SMART磁盘健康检测工具
│  ├─ encrypt_cfg/          ⚙️ 磁盘加密参数模板
│  ├─ mount_rules/          ⚙️ 自动挂载策略
│  └─ config/                ⚙️ 存储全局配置
├─ backup/
│  ├─ backup_daemon/        🟢 备份后台守护进程
│  ├─ restore_tools/        🟢 系统还原工具集
│  ├─ rules/                ⚙️ 增量/全量备份策略
│  └─ config/               ⚙️ 备份保留周期配置
├─ suspend/
│  ├─ suspend_daemon/       🟢 待机/休眠管理守护进程
│  ├─ hibernate_tools/      🟢 休眠镜像校验、处理工具
│  ├─ power_rules/          ⚙️ 电源策略、唤醒源白名单
│  ├─ config/               ⚙️ 休眠压缩参数、唤醒脚本
│  └─ data/                 💾 运行时休眠镜像、进程转存快照，无可执行
│     ├─ hiber_system/
│     └─ proc_dump/
│        └─ …… #按程序名动态生成文件夹
├─ asm/
│  ├─ asm_tools/            🟢 汇编器、链接器、反汇编器工具
│  ├─ src/                  ❗成品系统为空，不存放汇编源码
│  ├─ bin/                  🟢 汇编工具链产出二进制
│  └─ config/               ⚙️ 汇编编译架构配置
├─ SystemApps/                 📦 系统应用目录（语言编译器 + 全部系统应用）
│  ├─ python/                 📦 Python 编译器 / 运行时
│  ├─ java/                   📦 Java 编译器 / 运行时
│  ├─ map/
│  │  ├─ map_app/              🟢 地图应用主程序
│  │  ├─ location_svc/         🟢 定位服务（聚合 GPS / 网络定位）
│  │  ├─ map_data/             📦 地图数据（瓦片、POI）
│  │  └─ config/               ⚙️ 地图与定位配置
│  ├─ img/
│  │  ├─ img_app/              🟢 截屏与图片应用主程序
│  │  ├─ screenshot/           🟢 截图组件（全屏/区域/窗口）
│  │  ├─ filters/              📦 图片滤镜/处理库
│  │  └─ config/               ⚙️ 截屏与图片配置
│  ├─ cam/
│  │  ├─ cam_app/              🟢 摄像头应用主程序（拍照/录像）
│  │  ├─ capture/              🟢 拍照组件
│  │  ├─ recorder/             🟢 录像组件
│  │  └─ config/               ⚙️ 摄像头应用配置
│  ├─ rec/
│  │  ├─ rec_app/              🟢 录音与音乐应用主程序
│  │  ├─ music_player/         🟢 音乐播放器
│  │  ├─ music_editor/         🟢 音乐编辑器
│  │  ├─ asr_engine/           🟢 语音识别引擎（加载模型推理）
│  │  ├─ asr_model/            📦 1.5B 语音识别模型
│  │  ├─ codecs/               📦 音乐解码库（随应用卸载一并卸载）
│  │  └─ config/               ⚙️ 录音与音乐配置
│  ├─ video/
│  │  ├─ video_app/            🟢 电影与视频应用主程序
│  │  ├─ player/               🟢 视频播放器
│  │  ├─ codecs/               📦 视频解码库（随应用卸载一并卸载，全格式）
│  │  └─ config/               ⚙️ 电影与视频配置
│  ├─ note/
│  │  ├─ note_app/             🟢 便签与文档编辑器主程序
│  │  ├─ sticky_notes/         🟢 便签组件（桌面便签）
│  │  ├─ editor/               🟢 文档编辑器（记事本）
│  │  ├─ formats/              📦 文档格式库（txt/富文本）
│  │  └─ config/               ⚙️ 便签与文档配置
│  ├─ time/
│  │  ├─ time_app/             🟢 时间应用主程序（任务栏时钟、地区时间、计时工具）
│  │  ├─ time_svc.service      🟢 后台校时与时间服务器服务（NTP 客户端/服务端）
│  │  └─ config/               ⚙️ 时区库、地区列表、时间服务器、日历/日程/闹钟与计时配置
│  └─ procmon/
│  │  ├─ procmon_app/          🟢 进程资源监视器主程序（类任务管理器/资源监视器）
│  │  └─ config/               ⚙️ 监控采样周期、告警阈值配置
├─ api/         ⚙️【机器契约！不是文档】存放*.api契约文件，供桥接层加载解析，机器读取
│  ├─ ipc/                  ⚙️ IPC 子系统契约（9 个：core/mq/shm/sem/fifo/sock/sig/mirror/dual）
│  └─ bridge/               🟢 API桥接服务本体可执行程序（解析加载 .api 契约）
├─ filemgr/                 🟢 系统文件管理器主程序
├─ linux_compat/
│  ├─ compat/               🟢 Linux 兼容层主可执行程序（ELF 加载 / 系统调用翻译 / 应用与进程管理）
│  ├─ config/               ⚙️ Linux兼容层配置
│  ├─ lib/                  📦 Linux模拟兼容库
│  ├─ profiles/             ⚙️ 兼容环境配置文件
│  └─ api_adapt/            ⚙️ Linux适配用的api契约文件
└─ objmgr/
   └─ policies/             ⚙️ 对象管理器ACL、初始化策略