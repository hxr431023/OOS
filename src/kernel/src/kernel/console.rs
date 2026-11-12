#![no_std]
use core::fmt;

pub struct StdOut;
impl fmt::Write for StdOut {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.as_bytes() {
            // 等待发送保持寄存器空（LSR bit5 THRE），否则快速连续写会丢字符
            loop {
                let lsr: u8;
                unsafe {
                    core::arch::asm!(
                        "in al, dx",
                        out("al") lsr,
                        in("dx") 0x3FDu16,
                        options(nomem, nostack)
                    );
                }
                if lsr & 0x20 != 0 {
                    break;
                }
            }
            unsafe {
                // x86 串口 COM1 是 I/O 端口（0x3F8），必须用 out 指令，不能内存映射
                core::arch::asm!(
                    "out dx, al",
                    in("dx") 0x3F8u16,
                    in("al") *b,
                    options(nomem, nostack)
                );
            }
            // 同步输出到 VGA 文本屏幕（VirtualBox 窗口可见）
            crate::vga::put_char(*b);
        }
        Ok(())
    }
}

pub fn init() {}

/// 轮询读取 COM1 一个字节（无数据返回 None）
pub fn poll_char() -> Option<u8> {
    unsafe {
        let lsr: u8;
        core::arch::asm!(
            "in al, dx",
            out("al") lsr,
            in("dx") 0x3FDu16,
            options(nomem, nostack)
        );
        if lsr & 1 != 0 {
            // 接收数据寄存器 RBR 0x3F8
            let ch: u8;
            core::arch::asm!(
                "in al, dx",
                out("al") ch,
                in("dx") 0x3F8u16,
                options(nomem, nostack)
            );
            Some(ch)
        } else {
            None
        }
    }
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        let mut out = $crate::console::StdOut{};
        let _ = core::fmt::write(&mut out, format_args!($($arg)*));
        let _ = core::fmt::write(&mut out, format_args!("\n"));
    }};
}
