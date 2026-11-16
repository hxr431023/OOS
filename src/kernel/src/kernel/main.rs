#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![allow(unsafe_op_in_unsafe_fn)]

extern crate alloc;

use core::panic::PanicInfo;

// 底层底座
mod sync;
mod memory;
mod console;
mod vga;
mod font8x8;
mod fb;
mod keyboard;
mod shell;
// 字体子系统（font.md）：加载器 + 资源映射器 + 字符映射器
mod font_loader;
mod font_resmap;
mod char_mapper;
mod font;
mod cjk_render;

// 核心业务
mod object;
mod acl;
mod ipc;
mod glue_layer;
mod api_parser;
mod syscall;
mod vfs;
// 文件子系统（fs.md）：磁盘文件系统
mod fs;

// 内存管理（虚拟内存 + IOMMU）
mod mm;

// 进程与调度
mod process;
#[path = "process/scheduler.rs"]
mod scheduler;
mod user;

// 硬件抽象
mod hw;

// 子系统
#[path = "api/mod.rs"]
mod api_bridge;
#[path = "log/mod.rs"]
mod log;
#[path = "driver/mod.rs"]
mod driver_mgr;
#[path = "net/mod.rs"]
mod net;
#[path = "ui/mod.rs"]
mod gfx;
#[path = "vm/mod.rs"]
mod vm;
#[path = "linux_compat/mod.rs"]
mod linux_compat;
#[path = "module/mod.rs"]
mod module_mgr;
mod subsys;

// 启动入口与 syscall 入口的全局汇编（替代 src/asm/*.s，避免依赖独立 nasm）
core::arch::global_asm!(
    r#"

    .section .boot
    .global _boot_rust_entry
    _boot_rust_entry:
        mov rsp, offset kernel_stack_top
        movabs rax, 0x800000
        mov rbx, qword ptr [rax]
        mov rbp, qword ptr [rax+8]
        mov r12, qword ptr [rax+16]
        mov r13, qword ptr [rax+24]
        mov r14, qword ptr [rax+32]
        mov r15, qword ptr [rax+40]
        mov qword ptr [rip + FB_BASE], rbx
        mov qword ptr [rip + FB_W], rbp
        mov qword ptr [rip + FB_H], r12
        mov qword ptr [rip + FB_STRIDE], r13
        mov qword ptr [rip + FB_BPP], r14
        mov qword ptr [rip + FB_FORMAT], r15
        movabs rax, offset _kmain
        jmp rax

    .section .boot
    .hang:
        hlt
        jmp .hang

    .section .bss
    .align 16
    kernel_stack:
        .space 1048576
    .global kernel_stack_top
    kernel_stack_top:

    "#
);

core::arch::global_asm!(
    r#"
    .section .text
    .global _syscall_entry
    _syscall_entry:
        swapgs
        mov qword ptr gs:[0x8], rsp
        mov rsp, qword ptr gs:[0x0]
        push qword ptr gs:[0x8]
        push rcx
        push r11
        push rdi
        push rsi
        push rdx
        push r10
        push r8
        push r9
        mov rdi, rax
        movabs rax, offset syscall_dispatch
        call rax
        pop r9
        pop r8
        pop r10
        pop rdx
        pop rsi
        pop rdi
        pop r11
        pop rcx
        pop rsp
        swapgs
        sysretq
    "#
);

fn sio_putc(c: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") 0x3F8u16, in("al") c, options(nomem, nostack));
    }
}
fn sio_str(s: &str) {
    for b in s.bytes() {
        sio_putc(b);
    }
}
const SIO_HEX: &[u8; 16] = b"0123456789abcdef";
fn sio_hex64(v: u64) {
    for i in 0..16 {
        let n = ((v >> (60 - i * 4)) & 0xF) as usize;
        sio_putc(SIO_HEX[n]);
    }
}

#[unsafe(no_mangle)]
static mut FB_BASE: u64 = 0;
#[unsafe(no_mangle)]
static mut FB_W: u64 = 0;
#[unsafe(no_mangle)]
static mut FB_H: u64 = 0;
#[unsafe(no_mangle)]
static mut FB_STRIDE: u64 = 0;
#[unsafe(no_mangle)]
static mut FB_BPP: u64 = 0;
#[unsafe(no_mangle)]
static mut FB_FORMAT: u64 = 0;

#[unsafe(no_mangle)]
unsafe extern "C" fn _kmain(_uefi_mem_info: *const memory::UefiMemoryMap) {
    // D2：真实 ram 区间由 bootloader 在 exit_boot_services 前读 UEFI memory map 得到，
    // 写到固定物理地址 0x800030/0x800038（exit 后保留）；fb 信息仍在 0x800000。
    // 本机 bootloader 用 jmp 进入、不按 C ABI 传参，因此以固定地址为准。
    let ram_start = unsafe { core::ptr::read_volatile(0x800030 as *const u64) };
    let ram_end = unsafe { core::ptr::read_volatile(0x800038 as *const u64) };
    if ram_start < 0x100000 || ram_end <= ram_start {
        sio_str("[mem] FATAL: invalid UEFI ram range (bootloader did not publish memory map)\n");
        loop {
            unsafe { core::arch::asm!("hlt", options(nomem, nostack)); }
        }
    }
    // framebuffer：保留 0x800000 机制，但先校验宽高/stride 是否合理，不合理回退文本 0xB8000
    let fb_base_raw = unsafe { core::ptr::read_volatile(0x800000 as *const u64) };
    let fb_w = unsafe { core::ptr::read_volatile(0x800008 as *const u64) } as u32;
    let fb_h = unsafe { core::ptr::read_volatile(0x800010 as *const u64) } as u32;
    let fb_stride = unsafe { core::ptr::read_volatile(0x800018 as *const u64) } as u32;
    let fb_bpp = unsafe { core::ptr::read_volatile(0x800020 as *const u64) } as u32;
    let fb_fmt = unsafe { core::ptr::read_volatile(0x800028 as *const u64) } as u32;
    let fb_ok = fb_base_raw != 0
        && fb_w > 0 && fb_w <= 8192
        && fb_h > 0 && fb_h <= 8192
        && fb_bpp > 0
        && fb_stride >= fb_w.saturating_mul(fb_bpp);
    let mem = memory::UefiMemoryMap {
        ram_start,
        ram_end,
        fb_base: if fb_ok { fb_base_raw } else { 0 },
        fb_width: if fb_ok { fb_w } else { 0 },
        fb_height: if fb_ok { fb_h } else { 0 },
        fb_stride: if fb_ok { fb_stride } else { 0 },
        fb_bpp: if fb_ok { fb_bpp } else { 0 },
        fb_format: if fb_ok { fb_fmt } else { 0 },
    };
    // 结构化一行日志（替代原 [K800=][K900=][PTR=][MEM=][MM start=] 裸十六进制转储）
    sio_str("[mem] uefi ram=0x");
    sio_hex64(mem.ram_start);
    sio_str("-0x");
    sio_hex64(mem.ram_end);
    sio_str(" fb=");
    sio_hex64(mem.fb_width as u64);
    sio_str("x");
    sio_hex64(mem.fb_height as u64);
    sio_str("x");
    sio_hex64(mem.fb_bpp as u64);
    sio_str("bpp stride=");
    sio_hex64(mem.fb_stride as u64);
    sio_str(" fmt=");
    sio_hex64(mem.fb_format as u64);
    sio_str("\n");
    memory::init(&mem);
    // D1：内核镜像区间与页分配器区间核对（串口证据）
    {
        let (pstart, pend) = memory::phys_range();
        sio_str("[mem] kernel_end=0x");
        sio_hex64(memory::kernel_end_addr() as u64);
        sio_str(" phys_start=0x");
        sio_hex64(pstart as u64);
        sio_str(" phys_end=0x");
        sio_hex64(pend as u64);
        sio_str("\n");
        if pstart >= pend {
            sio_str("[mem] FATAL: no allocatable physical memory after kernel image\n");
            loop {
                unsafe { core::arch::asm!("hlt", options(nomem, nostack)); }
            }
        }
    }
    // 必须在 memory::init 之后初始化显示层，否则 FB_INFO 仍为 0，vga 会误判无 GOP 走 0xB8000
    console::init();
    vga::init();
    println!("=== Hybrid kernel starting ===");
    println!("heap allocator ready");

    hw::time::init();
    keyboard::init();
    log::init();
    hw::apic::init();

    mm::page_table::init();
    mm::iommu::init();

    object::init();
    acl::init();
    ipc::init();
    println!("kobject subsystem ready");

    glue_layer::init();
    syscall::init();
    vfs::init();
    println!("syscall + vfs ready");

    hw::pci::init();

    process::init();
    scheduler::init();

    api_bridge::init();
    module_mgr::init();
    driver_mgr::init();
    net::init();
    gfx::init();
    vm::init();
    linux_compat::init();
    subsys::init();

    println!("=== ALL SUBSYSTEMS INITIALIZED ===");

    // D3 启动自检：ACL 判定（不依赖键盘交互，串口/屏幕留证）
    {
        use acl::AclMask;
        let obj = crate::object::create_object("/object/acltest")
            .unwrap_or_else(|_| crate::object::lookup("/object/acltest").unwrap());
        let _ = obj.create_entry("probe", AclMask::default_system());
        let _ = obj.write_entry("probe", b"seed", AclMask::default_system());
        let r_empty = obj.read_entry("probe", AclMask::empty());
        let r_rd = obj.read_entry("probe", AclMask::ENTRY_READ);
        let r_wr = obj.write_entry("probe", b"x", AclMask::ENTRY_READ);
        println!("[acltest] empty read => {}", if r_empty.is_err() { "EACCES(deny) OK" } else { "ALLOWED BAD" });
        println!("[acltest] read-only read => {}", if r_rd.is_ok() { "OK" } else { "DENIED BAD" });
        println!("[acltest] read-only write => {}", if r_wr.is_err() { "EACCES(deny) OK" } else { "ALLOWED BAD" });
        let pass = r_empty.is_err() && r_rd.is_ok() && r_wr.is_err();
        println!("[acltest] {}", if pass { "PASS" } else { "FAIL" });
    }

    // 用户态 (ring3) 基础设施：加载 GDT/TSS/IDT（本阶段不进 ring3）
    user::init();

    // 进入交互式串口 shell（真正的操作系统：可敲命令、看响应）
    // 显示层就绪后重新清屏白底，覆盖 VirtualBox 早期残留的洋红背景
    crate::fb::clear();
    shell::run();
}

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    println!("\nKERNEL PANIC! {:?}", info);
    loop {
        unsafe { core::arch::asm!("hlt") };
    }
}

#[global_allocator]
static GLOBAL_ALLOC: memory::KernelHeap = memory::KernelHeap::new();

#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    println!("Alloc OOM! layout {:?}", layout);
    loop {
        unsafe { core::arch::asm!("hlt") };
    }
}
