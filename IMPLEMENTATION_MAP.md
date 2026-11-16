# OOS 实现归属矩阵（内核 / 内核扩展模块 / 用户态服务）

> 依据：`specs/docs/kernel.md` §1「归属判据」——**能提高速度、且出错率较低** → 内核扩展模块（Ring0，`kernel/modules` 或 `kernel/aux_modules`）；性能收益不足或出错概率高 → 用户态服务（Ring3）。
> 另依据 `boot.md` 阶段划分：阶段 1–3 裸微内核，阶段 4 对象管理器（分水岭），阶段 5 内核扩展，阶段 6 init 与常驻前台程序。
> 本矩阵是本轮实现的施工依据：凡标「内核」的必须常驻 `hybrid_kernel` 主镜像；标「扩展」的走 `module::load_module` 可加载路径；标「服务」的只建对象/契约/进程骨架，不写图形界面。

## 0 一句话结论

| 层 | 放什么 | 不放什么 |
| --- | --- | --- |
| 微内核常驻（Ring0） | 调度、物理内存、页表、中断/APIC、IPC 原语、对象管理器、ACL、PCI 枚举、系统调用入口 | 文件系统实现、协议栈、驱动业务、任何用户态策略 |
| 内核扩展模块（Ring0，可加载） | 块设备、磁盘 FS、VFS 适配、`/system/dev` `/system/proc` `/system/sys` 虚拟节点后端、网络协议栈、网卡、显卡、输入、VMX（按需） | 服务编排、账户/登录策略、应用逻辑 |
| 用户态服务（Ring3） | init、服务管理器、服务运行容器、触发任务器、设备管理器、网络管理、音频、日志、电源、时间同步、账户/认证、会话、安装/更新、备份、Linux 兼容层 | GUI/合成器/桌面（本轮不写）、系统应用（本轮不做） |

## 1 启动流程归属（boot.md 阶段）

| 阶段 | 内容 | 归属 | 落地位置 | 本轮状态 |
| --- | --- | --- | --- | --- |
| 1–2 | UEFI、bootloader、内存图、GOP、跳转内核 | 引导程序（内核外） | `bootloader/` | 已有（描述符数组） |
| 3 | 临时页表、物理内存、异常/APIC、PCI 枚举、文字控制台 | **内核（常驻）** | `mm/`、`hw/`、`console.rs`、`fb.rs` | 已有 |
| 4 | 对象管理器、ACL、IPC、PCI 硬件对象批量创建、全局基础对象 | **内核（常驻）** | `object/`、`acl.rs`、`ipc.rs`、`hw/pci.rs` | 已有（需补阶段编排） |
| 5 | 块设备 / FS / VFS / 虚拟节点 / 网络栈 / 显卡 / 输入 / VMX 模块加载 | **内核扩展模块** | `module/` + 各模块注册函数 | 加载器为占位，本轮写实 |
| 6 | 创建 init 进程对象、拉起常驻前台程序 | **用户态服务** | `process/`、`service/`、`user.rs` | 仅有进程表，本轮补服务骨架 |

## 2 逐子系统归属

### 2.1 微内核常驻（不可卸载）

| 子系统 | 归属理由 | 代码 |
| --- | --- | --- |
| 物理内存 / 页表 / free ranges | 调度与分配的最底层，必须常驻 | `memory.rs`、`mm/page_table.rs` |
| 调度器 / 上下文切换 | 性能敏感，IPC 直达 | `process/`（本轮补 PCB 与就绪队列） |
| 中断 / APIC / 异常 | 硬件基础，不可延后 | `hw/apic.rs`、`user.rs` 异常门 |
| IPC 原语（mq/event/waitqueue） | 微内核核心，必须常驻 | `ipc.rs` |
| 对象管理器 / 条目 / ACL | 系统分水岭，基座 | `object/`、`acl.rs` |
| PCIe 枚举 | 阶段 3–4 硬件基础 | `hw/pci.rs` |
| 系统调用入口 | Ring3→Ring0 唯一门 | `syscall.rs`、`user.rs` |
| 内核模块加载器 | 需要 Ring0 映射与重定位 | `module/`（本轮写实） |

### 2.2 内核扩展模块（Ring0，可加载）

| 模块 | 目录（规范） | 归属理由 | 本轮落地 |
| --- | --- | --- | --- |
| 块设备服务 | `kernel/modules/block` | 中断/ DMA 密集，用户态往返代价高 | 注册为内置模块（`block`），模块表可见 |
| 磁盘文件系统 | `kernel/aux_modules/fs` | 与块设备同层，缓存命中要求高 | 内置模块 `fs` |
| VFS 适配层 | `kernel/aux_modules/vfs` | 路径解析高频，放用户态收益低 | 内置模块 `vfs` |
| `/system/dev` `/system/proc` `/system/sys` 虚拟节点后端 | `kernel/aux_modules/vfs` | 与 VFS 同层，直接读内核计数 | 绑定到 VFS 挂载 |
| 网络协议栈 | `kernel/aux_modules/net` | 收包路径性能敏感 | 内置模块 `net` |
| 网卡硬件服务 | `kernel/modules/net` | 中断/DMA | 内置模块 `nic` |
| 显卡服务 | `kernel/modules/gpu` | 帧缓冲直写 | 内置模块 `gpu` |
| 输入服务 | `kernel/modules/input` | 中断驱动 | 内置模块 `input` |
| VMX 虚拟化 | `kernel/modules/vmx` | 仅 CPU 支持时加载 | 条件模块（按 CPUID 判定） |
| 时间/RTC 底层 | `kernel/modules/rtc` | 端口 IO，极轻 | 内置模块 `rtc` |

> 上述模块本轮以「内置模块 + 统一模块表 + 可加载接口」实现：`module::load_module` 走真实 ELF 头解析与段映射，`module::register_builtin` 走静态注册；两条路径都进同一张模块表，`kernel.api` 的 `modules` 条目可读。

### 2.3 用户态服务（Ring3）

| 服务 | 目录（规范） | 归属理由 | 本轮落地 |
| --- | --- | --- | --- |
| init（PID=1） | `userland/init` | 用户空间总编排，崩溃可重启 | 进程对象 + 启动组编排 |
| 服务管理器 SCM | `userland/services/manager` | 策略多、易出错 | 对象 `/object/service/mgr` + 契约 |
| 服务加载器 | `userland/services/loader` | 解析 `.service`、校验 | 服务描述解析（内存表） |
| 服务运行容器 | `userland/services/host` | 打包多服务、线程隔离 | 容器对象 + 服务线程登记 |
| 触发任务器 | `userland/services/trigger` | 计划/条件触发，策略复杂 | 触发器对象 + 定时/事件规则 |
| 设备管理器 | `userland/devmgr` | 设备发现与匹配编排 | 对象 + 契约骨架 |
| 网络管理 | `userland/netmgr` | 策略与配置 | 对象 + 契约骨架 |
| 日志服务 | `userland/log` | 落盘与轮转 | 对象 + 契约骨架 |
| 电源管理 | `userland/power` | 策略、休眠 | 对象 + 契约骨架 |
| 音频服务 | `userland/audio` | 混音策略 | 对象 + 契约骨架 |
| 账户/认证 | `userland/auth` | 凭据、策略 | 对象 + 契约骨架 |
| 会话 | `userland/session` | 登录会话 | 对象 + 契约骨架 |
| 安装/更新/备份 | `userland/install|update|backup` | 流程编排 | 对象 + 契约骨架 |
| Linux 兼容层 | `userland/linux_compat` | 系统调用翻译 | 现有 `linux_compat/` 保留 |
| **GUI / 合成器 / 桌面 / 系统应用** | — | — | **本轮不实现**（用户明确排除） |

## 3 判据落地规则（写代码时的硬约束）

1. 中断上下文里只做「内核」层的事：收包、置位、唤醒；协议解析若在扩展模块也必须短路径。
2. 任何需要「策略、配置、重试、用户可见状态」的逻辑，一律进服务层，内核只提供对象与 IPC。
3. 扩展模块失败必须可降级：`module::load_module` 返回错误、打日志、不影响已加载模块与基座。
4. 服务崩溃由服务管理器按「重启 + 熔断」处理，内核不参与业务重启策略。
5. 新增对外接口一律先写 `.api` 片段（`/system/api/<target>`），再实现对象与条目。

## 4 本轮实现清单（对应代码）

| 编号 | 内容 | 文件 |
| --- | --- | --- |
| I1 | 启动阶段机（阶段 3→6 推进、阶段事件、`/system/api/boot` 契约） | `boot.rs`（新增） |
| I2 | 模块加载器写实（ELF64 头/段校验、内置模块表、按需 VMX） | `module/mod.rs` |
| I3 | 服务子系统（管理器/加载器/容器/触发器 + `/object/service/mgr` + 契约） | `service/mod.rs`（新增） |
| I4 | 进程层补 PCB 状态机、init 启动组、服务进程登记 | `process/mod.rs` |
| I5 | 系统调用入口从占位 `'E'` 改为真实分发 | `user.rs` |
| I6 | 启动编排接入 `main.rs`，串口输出阶段证据 | `main.rs` |
| I7 | 契约文件 `/system/api/boot.api`、`service.api`、`kernel.api` | `system/api/` |

---

## §6 本轮落地状态（2026-09-10）

| 编号 | 内容 | 落点 | 状态 |
|---|---|---|---|
| I1 | 启动阶段机（3→5→6→7→8） | `src/kernel/boot.rs` | 已实现，串口验证 phase 3→5→6→7 |
| I2 | 内核扩展模块装载（ELF 准入 + 内置表 + VMX 条件） | `src/kernel/module/mod.rs` | 已实现，10 个内置模块装载，VMX 按 CPUID 跳过 |
| I3 | 服务管理子系统（管理器/加载器/容器/触发任务器语义） | `src/kernel/service/mod.rs` | 已实现，15 注册 / 12 自启运行 / 3 容器 + selftest PASS |
| I4 | 启动编排接入 | `src/kernel/main.rs` | 已实现，模块阶段前移到进程阶段之前 |
| I5 | int80 真实系统调用分发 | `src/kernel/user.rs` | 已实现，寄存器按 SysV 映射后 `call syscall_dispatch` |
| I6 | 运行期查询命令 | `src/kernel/shell.rs` | 已实现 `boot` / `svc` / `module` |
| I7 | 片段契约 | `system/api/{boot,service,kernel}.api` | 已写入，待接入 `api_parser` 加载列表 |

**边界（按用户口径）**：系统应用与图形界面本轮不做；用户态服务当前落在"对象 + 条目 + 状态机 + 打包容器 + 进程对象"层，真实 Ring3 服务二进制与调度执行体为下一步。

## §7 三项补齐状态

| 项 | 实现 | 验证 |
|---|---|---|
| `system/api/*.api` 接入 `api_parser` | `CONTRACTS` 扩到 7 项（4 JSON + 3 DSL 片段），DSL 走 `parse_dsl` → 建对象/条目 | 串口 `[api] loaded 7 contract(s), 7 object(s), 63 entry(s); dsl=3 json=4 failed=0` |
| 模块段映射 + 重定位 | `ModuleManager::elf_load`：节头表解析、ALLOC 节拷贝到堆映像、SHT_RELA 处理 `R_X86_64_64/PC32/PLT32/32`、按符号表取址；`load_module` 改走该路径 | 代码编译通过；仓库内暂无真实 .ko 样本，未做加载实测 |
| 真实 Ring3 服务执行体 | `switch_to_user` 改 naked（精确保存返回栈）、`run_user`、`user_demo` 执行 `int 0x80`（eax=99）、int80 入口置 `USER_EXIT_REQUESTED` 后回内核、用户页权限支持 1G/2M/4K 三级 | 开机路径不进入 ring3（保留 `ring3` 命令）。VBox 触发实测仍卡死，未达到可用验收，先不阻塞开机 |
