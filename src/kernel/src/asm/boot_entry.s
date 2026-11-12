bits 64
section .boot

global _boot_rust_entry
extern _kmain

_boot_rust_entry:
    mov rsp, kernel_stack_top
    mov rdi, rcx
    call _kmain

.hang:
    hlt
    jmp .hang

section .bss
align 16
kernel_stack: resb 16384
kernel_stack_top: equ $
