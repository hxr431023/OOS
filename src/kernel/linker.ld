/* x86_64 内核链接脚本：物理低地址（identity 映射，UEFI bootloader 直接加载跳转）
   必须收集所有 Rust 独立 section（.text.* / .rodata.* / .data.* / .bss.*），
   否则函数被默认散落导致 RIP 相对重定位错误。 */
ENTRY(_boot_rust_entry)

SECTIONS
{
    . = 0x100000;

    .text ALIGN(4K) : {
        *(.boot)
        *(.text)
        *(.text.*)
    }

    .rodata ALIGN(4K) : {
        *(.rodata)
        *(.rodata.*)
    }

    .data ALIGN(4K) : {
        *(.data)
        *(.data.*)
    }

    .bss ALIGN(4K) : {
        *(COMMON)
        *(.bss)
        *(.bss.*)
    }
}
