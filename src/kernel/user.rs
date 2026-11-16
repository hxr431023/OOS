//! 用户态 (ring3) 基础设施骨架：GDT + TSS + IDT(int 0x80 门) + 进 ring3 切换
use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

unsafe extern "C" { fn user_return_point(); }

#[unsafe(no_mangle)]
pub static mut KERNEL_RET: u64 = 0;
#[unsafe(no_mangle)]
pub static mut KERNEL_RSP: u64 = 0;
#[unsafe(no_mangle)]
pub static USER_EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);

// ---- GDT 布局（64 位）----
const KCODE: u16 = 0x08;
const KDATA: u16 = 0x10;
const UCODE: u16 = 0x18;
const UDATA: u16 = 0x20;
const TSS_SEL: u16 = 0x28;

static mut GDT: [u64; 7] = [
    0x0000000000000000,
    0x00AF9A000000FFFF, // kernel code, DPL0, L=1
    0x00AF92000000FFFF, // kernel data, DPL0
    0x00AFFA000000FFFF, // user code, DPL3, L=1
    0x00AFF2000000FFFF, // user data, DPL3
    0, // TSS low
    0, // TSS high
];

#[repr(C)]
struct TaskStateSegment {
    _res0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    _res1: u64,
    ist1: u64,
    ist2: u64,
    ist3: u64,
    ist4: u64,
    ist5: u64,
    ist6: u64,
    ist7: u64,
    _res2: u64,
    _res3: u16,
    iomap_base: u16,
}

static mut TSS: TaskStateSegment = TaskStateSegment {
    _res0: 0,
    rsp0: 0,
    rsp1: 0,
    rsp2: 0,
    _res1: 0,
    ist1: 0,
    ist2: 0,
    ist3: 0,
    ist4: 0,
    ist5: 0,
    ist6: 0,
    ist7: 0,
    _res2: 0,
    _res3: 0,
    iomap_base: 0xFFFF,
};

#[repr(C, packed)]
struct DescriptorPointer {
    limit: u16,
    base: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct IdtEntry {
    base_lo: u16,
    sel: u16,
    ist: u8,
    flags: u8,
    base_mid: u16,
    base_hi: u32,
    _zero: u32,
}

static mut IDT: [IdtEntry; 256] = [IdtEntry {
    base_lo: 0,
    sel: 0,
    ist: 0,
    flags: 0,
    base_mid: 0,
    base_hi: 0,
    _zero: 0,
}; 256];

unsafe extern "C" {
    fn _int80_entry();
    fn _exc_ud();
    fn _exc_gp();
    fn _exc_pf();
}

/// 异常转储（打印异常号/错误码/RIP/CR2，然后停）
#[unsafe(no_mangle)]
pub extern "C" fn exc_dump(frame: *const u64) {
    unsafe {
        let num = *frame.add(9);
        let err = *frame.add(10);
        let rip = *frame.add(11);
        let cr2: u64;
        core::arch::asm!("mov rax, cr2", out("rax") cr2, options(nomem, nostack));
        crate::println!(
            "\n!!! EXCEPTION #{} err=0x{:x} RIP=0x{:x} CR2=0x{:x}",
            num,
            err,
            rip,
            cr2
        );
    }
}

fn set_idt_gate(vec: usize, offset: u64, sel: u16, flags: u8) {
    unsafe {
        let e = &mut IDT[vec];
        e.base_lo = (offset & 0xFFFF) as u16;
        e.base_mid = ((offset >> 16) & 0xFFFF) as u16;
        e.base_hi = ((offset >> 32) & 0xFFFFFFFF) as u32;
        e.sel = sel;
        e.ist = 0;
        e.flags = flags;
        e._zero = 0;
    }
}

fn load_gdt() {
    unsafe {
        let base = core::ptr::addr_of!(GDT) as u64;
        let gdtr = DescriptorPointer { limit: 55u16, base };
        asm!("lgdt [{}]", in(reg) &gdtr, options(nostack));
        asm!(
            "lea rax, [rip + 2f]",
            "push 0x08",
            "push rax",
            "retfq",
            "2:",
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov ss, ax",
            options(nostack)
        );
    }
}

fn setup_tss() {
    unsafe {
        unsafe extern "C" {
            static kernel_stack_top: u8;
        }
        TSS.rsp0 = &raw const kernel_stack_top as u64;
        TSS.iomap_base = 0xFFFF;

        let base = &raw const TSS as u64;
        let limit: u64 = 0x67;
        let low = (limit & 0xFFFF)
            | ((base & 0xFFFF) << 16)
            | (((base >> 16) & 0xFF) << 32)
            | (0x09 << 40)
            | (1 << 47)
            | (((limit >> 16) & 0xF) << 48)
            | (((base >> 24) & 0xFF) << 56);
        let high = base >> 32;
        GDT[5] = low;
        GDT[6] = high;

        asm!("ltr ax", in("ax") TSS_SEL, options(nostack));
    }
}

fn load_idt() {
    unsafe {
        set_idt_gate(0x80, _int80_entry as u64, KCODE, 0xEE);
        // 异常门（DPL=0，供 ring3 出错时定位，避免 triple fault 无声冻结）
        set_idt_gate(0x06, _exc_ud as u64, KCODE, 0x8E); // #UD
        set_idt_gate(0x0D, _exc_gp as u64, KCODE, 0x8E); // #GP
        set_idt_gate(0x0E, _exc_pf as u64, KCODE, 0x8E); // #PF
        let base = core::ptr::addr_of!(IDT) as u64;
        let limit = (core::mem::size_of::<IdtEntry>() * 256 - 1) as u16;
        let idtr = DescriptorPointer { limit, base };
        asm!("lidt [{}]", in(reg) &idtr, options(nostack));
    }
}

/// 初始化用户态基础设施（加载 GDT/TSS/IDT，不进 ring3）
pub fn init() {
    load_gdt();
    setup_tss();
    load_idt();
    // 诊断：打印 IDT 门 offset，确认门设置正确（不触发任何中断）
    let (off80, off6) = unsafe {
        (
            ((IDT[0x80].base_hi as u64) << 32)
                | ((IDT[0x80].base_mid as u64) << 16)
                | (IDT[0x80].base_lo as u64),
            ((IDT[0x06].base_hi as u64) << 32)
                | ((IDT[0x06].base_mid as u64) << 16)
                | (IDT[0x06].base_lo as u64),
        )
    };
    crate::println!(
        "IDT diag: 0x80->{:#x} (_int80={:#x})  0x06->{:#x} (_exc_ud={:#x})",
        off80,
        _int80_entry as u64,
        off6,
        _exc_ud as u64
    );
    crate::println!("user-mode infra: GDT + TSS + IDT ready");
}

/// 打印用户态基础设施关键地址（shell `diag` 命令触发，便于连接后捕获）
pub unsafe fn dump_user_diag() {
    unsafe extern "C" {
        static kernel_stack_top: u8;
        static user_stack_top: u8;
    }
    let rsp0 = TSS.rsp0;
    let kstack = &raw const kernel_stack_top as u64;
    let ustack = &raw const user_stack_top as u64;
    crate::println!(
        "TSS.rsp0={:#x}  kernel_stack_top={:#x}  user_stack_top={:#x}",
        rsp0,
        kstack,
        ustack
    );
    crate::println!(
        "user_demo={:#x}  _int80_entry={:#x}",
        user_demo as u64,
        _int80_entry as u64
    );
    crate::println!(
        "GDT[3](UCODE)={:#x}  GDT[4](UDATA)={:#x}  GDT[5](TSSlo)={:#x}",
        GDT[3],
        GDT[4],
        GDT[5]
    );
    // sidt 读回 IDTR，确认 lidt 是否设置正确
    let mut buf: [u8; 10] = [0; 10];
    asm!("sidt [{}]", in(reg) buf.as_mut_ptr(), options(nostack));
    let idt_limit = u16::from_le_bytes([buf[0], buf[1]]);
    let idt_base = u64::from_le_bytes([
        buf[2], buf[3], buf[4], buf[5], buf[6], buf[7], buf[8], buf[9],
    ]);
    crate::println!(
        "IDTR: base={:#x} limit={}  (IDT addr={:#x})",
        idt_base,
        idt_limit,
        &raw const IDT as u64
    );
}

// ---- int 0x80 系统调用入口（ring3 -> ring0）----
core::arch::global_asm!(
    r#"
    .section .text
    .global user_return_point
user_return_point:
    ret

    .macro PUSHALL
        push rax
        push rcx
        push rdx
        push rdi
        push rsi
        push r8
        push r9
        push r10
        push r11
    .endm
    .section .text
    .extern syscall_dispatch
    .global _int80_entry
    _int80_entry:
        push rcx
        push r11
        push rdi
        push rsi
        push rdx
        push r10
        push r8
        push r9
        mov r9, r8
        mov r8, r10
        mov rcx, rdx
        mov rdx, rsi
        mov rsi, rdi
        mov rdi, rax
        call syscall_dispatch
        pop r9
        pop r8
        pop r10
        pop rdx
        pop rsi
        pop rdi
        cmp byte ptr [rip + USER_EXIT_REQUESTED], 0
        jne 1f
        pop r11
        pop rcx
        iretq
    1:
        mov rsp, [rip + KERNEL_RSP]
        jmp qword ptr [rip + KERNEL_RET]

    .global _exc_ud
    _exc_ud:
        cli
        mov dx, 0x3F8
        mov al, 'U'
        out dx, al
        push 0
        push 6
        jmp exc_common
    .global _exc_gp
    _exc_gp:
        cli
        mov dx, 0x3F8
        mov al, 'G'
        out dx, al
        push 13
        jmp exc_common
    .global _exc_pf
    _exc_pf:
        cli
        mov dx, 0x3F8
        mov al, 'P'
        out dx, al
        push 14
        jmp exc_common

    exc_common:
        PUSHALL
        mov rdi, rsp
        mov dx, 0x3F8
        mov al, 'X'
        out dx, al
        hlt
        jmp exc_common
    "#
);

/// 用户态演示代码（运行在 ring3）：调用 syscall #0 然后死循环
#[unsafe(no_mangle)]
#[inline(never)]
pub unsafe extern "C" fn user_demo() {
    unsafe {
        // 真实 ring3 系统调用：eax=99 触发内核分发，随后由 int80 返回内核
        asm!(
            "mov eax, 99",
            "xor edi, edi",
            "xor esi, esi",
            "xor edx, edx",
            "xor r10d, r10d",
            "xor r8d, r8d",
            "int 0x80",
            "ud2",
            options(nomem, nostack)
        );
    }
}

/// 把某地址所在页（含上级表项）置 U/S+RW，兼容 1GB/2MB/4KB 映射
pub unsafe fn make_user_page(addr: u64) {
    let cr3: u64;
    asm!("mov rax, cr3", out("rax") cr3, options(nomem, nostack));
    let pml4 = (cr3 & !0xFFF) as *mut u64;
    let i4 = ((addr >> 39) & 0x1FF) as usize;
    let e4 = *pml4.add(i4);
    *pml4.add(i4) = e4 | 0x6;

    let pdpt = (e4 & 0x000F_FFFF_FFFF_F000) as *mut u64;
    let i3 = ((addr >> 30) & 0x1FF) as usize;
    let e3 = *pdpt.add(i3);
    if e3 & 0x80 != 0 { // 1GB page
        *pdpt.add(i3) = e3 | 0x6;
        asm!("invlpg [{}]", in(reg) addr & !0xFFF, options(nostack));
        return;
    }
    *pdpt.add(i3) = e3 | 0x6;

    let pd = (e3 & 0x000F_FFFF_FFFF_F000) as *mut u64;
    let i2 = ((addr >> 21) & 0x1FF) as usize;
    let e2 = *pd.add(i2);
    if e2 & 0x80 != 0 { // 2MB page
        *pd.add(i2) = e2 | 0x6;
    } else {
        *pd.add(i2) = e2 | 0x6;
        let pt = (e2 & 0x000F_FFFF_FFFF_F000) as *mut u64;
        let i1 = ((addr >> 12) & 0x1FF) as usize;
        let e1 = *pt.add(i1);
        *pt.add(i1) = e1 | 0x6;
    }
    asm!("invlpg [{}]", in(reg) addr & !0xFFF, options(nostack));
}

/// 准备用户态：给用户代码页和用户栈页设 U/S
pub unsafe fn prepare_user() {
    make_user_page(user_demo as u64);
    unsafe extern "C" {
        static user_stack_top: u8;
    }
    make_user_page(&raw const user_stack_top as u64);
    crate::println!("user pages set to U/S (ring3 accessible)");
}

/// 从内核切到 ring3（naked：入口 rsp 即返回地址，桩 ret 可精确回落）
#[unsafe(naked)]
pub unsafe extern "C" fn switch_to_user(stack: u64, rip: u64) {
    core::arch::naked_asm!(
        "mov [rip + KERNEL_RSP], rsp",
        "lea rax, [rip + {ret}]",
        "mov [rip + KERNEL_RET], rax",
        "push 0x23",
        "push rdi",
        "push 0x202",
        "push 0x1B",
        "push rsi",
        "iretq",
        ret = sym user_return_point,
    )
}

/// 进入 ring3 执行一次演示程序
pub unsafe fn run_user() {
    unsafe extern "C" { static user_stack_top: u8; }
    USER_EXIT_REQUESTED.store(false, Ordering::SeqCst);
    switch_to_user(&raw const user_stack_top as u64, user_demo as u64);
}

// 用户栈（ring3）
core::arch::global_asm!(
    r#"
    .section .bss
    .align 16
    user_stack:
        .space 8192
    .global user_stack_top
    user_stack_top:
    "#
);
