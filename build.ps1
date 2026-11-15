# 混合内核完整构建脚本 (Windows PowerShell)
# 用法：.\build.ps1 [-Clean] [-RunQemu]

param(
    [switch]$Clean,
    [switch]$RunQemu
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $ProjectRoot

Write-Host "=== Hybrid OS Build System ===" -ForegroundColor Cyan

# 工具检查
function Check-Tool {
    param($Name, $Cmd)
    $result = Get-Command $Cmd -ErrorAction SilentlyContinue
    if ($result) {
        Write-Host "  [OK] $Name" -ForegroundColor Green
    } else {
        Write-Host "  [MISSING] $Name ($Cmd)" -ForegroundColor Red
    }
    return $result -ne $null
}

Write-Host "`n[1/6] Checking tools..." -ForegroundColor Yellow
$hasRustc = Check-Tool "Rust" "rustc"
$hasCargo = Check-Tool "Cargo" "cargo"
$hasNasm = Check-Tool "NASM" "nasm"
$hasLld = Check-Tool "LLD linker" "ld.lld"
$hasQemu = Check-Tool "QEMU" "qemu-system-x86_64"

if (-not $hasRustc -or -not $hasCargo) {
    Write-Host "`nERROR: Rust toolchain required. Install from https://rustup.rs" -ForegroundColor Red
    exit 1
}

# 安装no_std目标
Write-Host "`n[2/6] Ensuring x86_64-unknown-none target..." -ForegroundColor Yellow
rustup target add x86_64-unknown-none 2>$null | Out-Null
rustup component add rust-src 2>$null | Out-Null

# 清理
if ($Clean) {
    Write-Host "`n[Clean] Removing build artifacts..." -ForegroundColor Yellow
    Remove-Item -Recurse -Force target, build -ErrorAction SilentlyContinue
}

# 创建输出目录
New-Item -ItemType Directory -Force -Path build, build/iso, build/iso/EFI, build/iso/EFI/boot, build/iso/EFI/hybrid | Out-Null

# 编译汇编
Write-Host "`n[3/6] Compiling assembly..." -ForegroundColor Yellow
nasm -f elf64 src/asm/boot_entry.s -o build/boot_entry.o
if ($LASTEXITCODE -ne 0) { throw "NASM boot_entry failed" }
nasm -f elf64 src/asm/syscall_stub.s -o build/syscall_stub.o
if ($LASTEXITCODE -ne 0) { throw "NASM syscall_stub failed" }
Write-Host "  Assembly compiled OK" -ForegroundColor Green

# 编译内核
Write-Host "`n[4/6] Building kernel (x86_64-unknown-none, release)..." -ForegroundColor Yellow
cargo build --target x86_64-unknown-none --release --lib
if ($LASTEXITCODE -ne 0) { throw "Kernel build failed" }
Write-Host "  Kernel built OK" -ForegroundColor Green

# 链接内核ELF
Write-Host "`n[5/6] Linking kernel ELF..." -ForegroundColor Yellow
$kernelLib = "target/x86_64-unknown-none/release/libhybrid_kernel.a"
if (-not (Test-Path $kernelLib)) {
    # 尝试其他可能的输出名
    $kernelLib = Get-ChildItem target/x86_64-unknown-none/release/*.a | Select-Object -First 1 -ExpandProperty FullName
}
Write-Host "  Using: $kernelLib"

if ($hasLld) {
    ld.lld -n -T linker.ld -o build/kernel.elf build/boot_entry.o build/syscall_stub.o $kernelLib
} else {
    Write-Host "  WARNING: ld.lld not found, using rust-lld fallback" -ForegroundColor Yellow
    rust-lld -flavor gnu -n -T linker.ld -o build/kernel.elf build/boot_entry.o build/syscall_stub.o $kernelLib
}
if ($LASTEXITCODE -ne 0) { throw "Kernel link failed" }

# 生成二进制镜像（用于直接加载）
objcopy -O binary build/kernel.elf build/kernel.bin 2>$null
Write-Host "  Kernel ELF: build/kernel.elf" -ForegroundColor Green

# 制作可启动ISO
Write-Host "`n[6/6] Creating bootable ISO..." -ForegroundColor Yellow

# 复制内核到ISO结构
Copy-Item build/kernel.elf build/iso/EFI/hybrid/kernel.elf -Force

# 复制.api契约文件
Copy-Item -Recurse api_definitions build/iso/EFI/hybrid/api_definitions -Force -ErrorAction SilentlyContinue

# 使用预编译的UEFI bootloader（或使用OVMF）
# 简化方案：直接使用内核bin + grub配置，或使用limine
# 这里使用一个最小的启动配置

# 方案A：使用limine bootloader（推荐，现代）
if (Get-Command limine -ErrorAction SilentlyContinue) {
    Write-Host "  Using limine bootloader..." -ForegroundColor Cyan
    # limine 配置
    @"
TIMEOUT=3
GRAPHICS=yes
DEFAULT_ENTRY=Hybrid OS

:Hybrid OS
PROTOCOL=linux
KERNEL_PATH=boot(\EFI\hybrid\kernel.elf)
CMDLINE=
"@ | Set-Content build/iso/limine.cfg -Encoding ASCII
    limine bios-install build/hybrid-os.iso 2>$null
}

# 方案B：使用genisoimage/xorriso制作ISO（需要额外工具）
if (Get-Command xorriso -ErrorAction SilentlyContinue) {
    xorriso -as mkisofs -R -f -e EFI/hybrid/kernel.elf -no-emul-boot -o build/hybrid-os.iso build/iso
    Write-Host "  ISO created: build/hybrid-os.iso" -ForegroundColor Green
} elseif (Get-Command genisoimage -ErrorAction SilentlyContinue) {
    genisoimage -R -f -e EFI/hybrid/kernel.elf -no-emul-boot -o build/hybrid-os.iso build/iso
    Write-Host "  ISO created: build/hybrid-os.iso" -ForegroundColor Green
} else {
    Write-Host "  NOTE: xorriso/genisoimage not found." -ForegroundColor Yellow
    Write-Host "  Kernel files are in build/iso/ - can be packaged manually" -ForegroundColor Yellow
    Write-Host "  For QEMU direct boot: qemu-system-x86_64 -kernel build/kernel.elf" -ForegroundColor Yellow
}

Write-Host "`n=== Build Complete ===" -ForegroundColor Cyan
Write-Host "  Kernel ELF: build/kernel.elf"
Write-Host "  Kernel BIN: build/kernel.bin"
Write-Host "  ISO:        build/hybrid-os.iso (if tool available)"
Write-Host "  API defs:   api_definitions/*.api"

# 可选：QEMU运行
if ($RunQemu -and $hasQemu) {
    Write-Host "`n=== Launching QEMU ===" -ForegroundColor Cyan
    if (Test-Path build/hybrid-os.iso) {
        qemu-system-x86_64 -cdrom build/hybrid-os.iso -m 256M -serial stdio -no-reboot
    } else {
        # 直接用-kernel启动（简化测试）
        qemu-system-x86_64 -kernel build/kernel.elf -m 256M -serial stdio -no-reboot -append "console=ttyS0"
    }
}
