# OOS v0.1 发布公告

**发布日期**：2026-11-12
**版本**：v0.1（首个实现版本）
**性质**：基础内核，可引导运行

---

v0.1 是 OOS 从设计规范走向实现的第一步：仓库新增 `src/kernel/`，发布一套以 Rust 编写的 x86-64 基础内核（`no_std`），并附 UEFI bootloader、契约文件与参考网络栈。

## 本版内容

### 内核主体
- **对象管理器**：`KObject` / `KEntry`、全局对象树、lookup / create
- **访问控制**：ACL 掩码与权限检查
- **IPC**：对象化 IPC 通道
- **内存**：UEFI 内存图解析、页分配、页表、IOMMU、伙伴分配器
- **进程与调度**：进程 / 线程生命周期、上下文、调度器
- **文件系统**：VFS、inode、挂载点、ISO9660、ATA 块设备
- **网络**：校验和、IPv4、UDP、ARP、TCP 状态机（Rust 实现）
- **虚拟化**：VMX 启用
- **Linux 兼容层**：procfs / sysfs 模拟
- **日志**：内核日志、审计、环形缓冲
- **驱动与模块管理**：`driver_mgr`、模块加载与内核符号导出
- **硬件服务**：APIC、PCI 枚举、时钟
- **图形基础**：framebuffer 文本输出、字体加载与字符映射、中日韩渲染雏形
- **系统调用**：syscall 入口与自检
- **UI**：窗口对象与图形基础接口

### 子系统控制对象
`storage`、`security`、`service`、`power`、`backup`、`pkg`、`update`、`filemgr`、`cmd`、`audio`、`dev`、`devmgr` 的对象与条目已按 `docs/` 中各 `.api` 契约注册进对象管理器，可用 `tree` 与 `sysobj` 命令直接查询。

### 命令行
- 交互式 shell，支持 `cd`、`pwd`、`meminfo`、`cpuinfo`、`sysinfo`、`tree`、`netstat`、`whoami`、`acl`、`help` 等命令
- `sysobj <subsys>` 读取任意子系统控制对象条目
- `bootdiag` 开机自检输出

### 构建与工具
- `Cargo.toml`（`hybrid_kernel`，staticlib，edition 2024）
- `linker.ld`、`Makefile`、`build.ps1` 构建脚本
- `api_definitions/` 四份契约：driver.manager、net.socket、ui.window、vm.instance
- `netstack/` Go 参考实现（方案 C）
- `generate_font.py`、`test_vfs.py` 辅助脚本

## 验证

在 VirtualBox（EFI GOP 固件）中构建可引导 ISO 并启动，串口与屏幕终端输出：

```text
=== ALL SUBSYSTEMS INITIALIZED ===
[syscall] self-test done
=== Hybrid Shell ===
type 'help' for commands
[bootdiag] storage mgr: 7 entry(s)
HYBRID>
```

对象树、子系统条目、shell 命令均正常，内核引导无回归。

## 已知限制

- 这是**基础内核**：可引导、可交互，但多数子系统仍是对象-条目契约层，用户态守护进程主体尚未实现。
- `audio`、`dev` 为命名空间占位，需真实硬件接入后实例化。
- 图形仅 framebuffer 文本终端，尚无窗口合成器与完整 UI。
- 网络为内核内协议栈实现，尚未接入用户态 socket 服务。
- 构建需 nightly 工具链、`rust-lld`、`xorriso` 与 VirtualBox。

## 下一步

- v0.2：用户态服务框架与首个守护进程（service manager）
- 完善 security 策略模型与 storage 块设备后端
- 补齐窗口管理器与合成器，接入真实输入设备
- 将 `.api` 契约接入桥接层运行时，实现程序自动化调用

---

设计规范见 [`docs/`](docs/)，实现代码见 [`src/kernel/`](src/kernel/)。