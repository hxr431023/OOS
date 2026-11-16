# src — 真实代码目录

本目录存放 OOS 的实现代码。`docs/` 是对应的设计规范，代码与文档按子系统一一对应。

## 目录

| 路径 | 内容 |
| --- | --- |
| `kernel/` | Rust 内核工程（微内核、内核扩展文件、启动、对象与子系统契约层） |
| `README.md` | 本说明 |

## kernel 工程结构

| 路径 | 内容 |
| --- | --- |
| `kernel/src/kernel/` | 内核主体：`main.rs`、`object`、`acl`、`ipc`、`memory`、`process`、`vfs`、`net`、`ui`、`vm`、`linux_compat`、`log`、`subsys` 等 |
| `kernel/src/kernel/driver/` | ATA 等内核扩展文件（驱动） |
| `kernel/src/kernel/hw/` | APIC、PCI、时间等硬件服务 |
| `kernel/src/kernel/mm/` | 页表、IOMMU |
| `kernel/src/kernel/font/` | CFF 字体解析；`assets/hybrid.cfont` 为内置字体资源 |
| `kernel/src/asm/` | 启动入口与 syscall 桩（汇编） |
| `kernel/src/user/` | 用户态库与 init 程序 |
| `kernel/system/bootloader/` | UEFI bootloader（Rust，源码） |
| `kernel/netstack/` | Go 参考网络栈（方案 C，保留） |
| `kernel/api_definitions/` | `.api` 契约文件（driver.manager、net.socket、ui.window、vm.instance） |
| `kernel/Cargo.toml`、`linker.ld`、`Makefile`、`build.ps1` | 工程与构建配置 |

## 构建链

```text
cargo +nightly build --target x86_64-unknown-none --release --lib
rust-lld -flavor gnu -n -T linker.ld -o build/kernel.elf libhybrid_kernel.a
xorriso -as mkisofs ... -b isolinux.bin -e esp.fat -o hybrid.iso iso_src
```

`target/`、`build/` 与镜像产物已在 `.gitignore` 中排除，仓库只保留源码。

## 状态

基础内核已可在 VirtualBox（EFI GOP）引导，串口与 framebuffer 终端可交互。子系统控制对象契约层已注册，用户态守护进程主体待实现。

## 许可

本目录代码采用 **MPL 2.0**。修改本目录中的文件并以源码形式分发时，修改后的文件须继续以 MPL 2.0 公开；与专有代码组合成更大作品不受限制，无需公开上层专有代码。