#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

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
    .intel_syntax noprefix

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

    .att_syntax prefix
    "#
);

core::arch::global_asm!(
    r#"
    .intel_syntax noprefix
    .section .text
    .global _syscall_entry
    _syscall_entry:
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
        sysret
    .att_syntax prefix
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
    // 引导器 exit 后把 fb 信息写到固定物理地址 0x800000（identity 映射，exit 后保留），
    // _kmain 在此直接读取（已验证 read_volatile 可读到正确值）。ram 区固定 0x1000000..0x40000000。
    let mem = unsafe {
        memory::UefiMemoryMap {
            ram_start: 0x1000000,
            ram_end: 0x40000000,
            fb_base: core::ptr::read_volatile(0x800000 as *const u64),
            fb_width: core::ptr::read_volatile(0x800008 as *const u64) as u32,
            fb_height: core::ptr::read_volatile(0x800010 as *const u64) as u32,
            fb_stride: core::ptr::read_volatile(0x800018 as *const u64) as u32,
            fb_bpp: core::ptr::read_volatile(0x800020 as *const u64) as u32,
            fb_format: core::ptr::read_volatile(0x800028 as *const u64) as u32,
        }
    };
    let mem_map_ptr = &mem as *const memory::UefiMemoryMap;
    unsafe {
        core::arch::asm!(
            "mov dx, 0x3F8",
            "mov al, 'K'",
            "out dx, al",
            options(nomem, nostack)
        );
    }
    // DEBUG: 直接读引导器写入的固定地址，确认传递机制
    unsafe {
        sio_str("\n[K800=");
        sio_hex64(core::ptr::read_volatile(0x800000 as *const u64));
        sio_str(" K900=");
        sio_hex64(core::ptr::read_volatile(0x900000 as *const u64));
        sio_str("]\n");
    }
    // DEBUG: 打印 mem_map 指针及原始 48 字节，定位传参/布局
    sio_str("[PTR=");
    sio_hex64(mem_map_ptr as u64);
    sio_str("]\n[MEM=");
    let p = mem_map_ptr as *const u8;
    for i in 0..48usize {
        let b = unsafe { *p.add(i) };
        sio_putc(SIO_HEX[(b >> 4) as usize]);
        sio_putc(SIO_HEX[(b & 0xF) as usize]);
    }
    sio_str("]\n");
    let mem_map = &mem;
    memory::init(mem_map);
    // 必须在 memory::init 之后初始化显示层，否则 FB_INFO 仍为 0，vga 会误判无 GOP 走 0xB8000
    console::init();
    // DEBUG: 纯串口打印内核读到的 mem_map 字段（不经 vga，避免 fb 崩溃前丢失）
    sio_str("[MM start=");
    sio_hex64(mem_map.ram_start);
    sio_str(" end=");
    sio_hex64(mem_map.ram_end);
    sio_str(" fb=");
    sio_hex64(mem_map.fb_base);
    sio_str(" w=");
    sio_hex64(mem_map.fb_width as u64);
    sio_str(" h=");
    sio_hex64(mem_map.fb_height as u64);
    sio_str(" s=");
    sio_hex64(mem_map.fb_stride as u64);
    sio_str(" b=");
    sio_hex64(mem_map.fb_bpp as u64);
    sio_str(" f=");
    sio_hex64(mem_map.fb_format as u64);
    sio_str("]\n");
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
