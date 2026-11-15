bits 64
section .text

global _syscall_entry
extern syscall_dispatch

; 用户态通过 syscall 指令进入，内核栈切换在 Rust 侧处理
_syscall_entry:
    ; 保存用户态寄存器
    push rcx
    push r11
    push rdi
    push rsi
    push rdx
    push r10
    push r8
    push r9

    ; rdi = syscall number, rsi = arg1, rdx = arg2, r10 = arg3, r8 = arg4, r9 = arg5
    mov rdi, rax
    call syscall_dispatch

    ; 恢复寄存器，rax 保存返回值
    pop r9
    pop r8
    pop r10
    pop rdx
    pop rsi
    pop rdi
    pop r11
    pop rcx

    sysret
