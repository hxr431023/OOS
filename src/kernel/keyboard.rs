//! PS/2 键盘输入（8042 控制器，端口 0x60/0x64，扫描码集 2，轮询模式）
//! 完整按键映射：字母大小写、数字、全部标点、Shift 组合、回车/退格/空格/Tab
use core::sync::atomic::{AtomicBool, Ordering};

static KEYBOARD_READY: AtomicBool = AtomicBool::new(false);
/// 左右 Shift 按下状态
static SHIFT: AtomicBool = AtomicBool::new(false);
/// 扫描码集2 的 break 前缀标志（收到 0xF0 后，下一个扫描码是释放码）
static BREAK_PENDING: AtomicBool = AtomicBool::new(false);

fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe {
        core::arch::asm!("in al, dx", out("al") v, in("dx") port, options(nomem, nostack));
    }
    v
}

fn outb(port: u16, val: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
    }
}

pub fn init() {
    // 8042 控制器自检 + 启用键盘接口
    let mut guard = 0;
    while (inb(0x64) & 0x02) != 0 && guard < 10000 {
        guard += 1;
    }
    outb(0x64, 0x20);
    while (inb(0x64) & 0x01) == 0 {}
    let _cfg = inb(0x60);
    outb(0x64, 0x60);
    while (inb(0x64) & 0x02) != 0 {}
    outb(0x60, 0x00);
    outb(0x60, 0xFF);
    // 清空 8042 输出缓冲里的自检 ACK(0xFA)/结果(0xAA)，避免误读为按键
    guard = 0;
    while (inb(0x64) & 0x01) != 0 && guard < 100 {
        let _ = inb(0x60);
        guard += 1;
    }
    KEYBOARD_READY.store(true, Ordering::SeqCst);
}

/// 扫描码集 2 → ASCII（支持 Shift 组合与全部标点）
fn scancode_to_ascii(sc: u8, shift: bool) -> Option<u8> {
    let c = match sc {
        0x16 => if shift { b'!' } else { b'1' },
        0x1E => if shift { b'@' } else { b'2' },
        0x26 => if shift { b'#' } else { b'3' },
        0x25 => if shift { b'$' } else { b'4' },
        0x2E => if shift { b'%' } else { b'5' },
        0x36 => if shift { b'^' } else { b'6' },
        0x3D => if shift { b'&' } else { b'7' },
        0x3E => if shift { b'*' } else { b'8' },
        0x46 => if shift { b'(' } else { b'9' },
        0x45 => if shift { b')' } else { b'0' },
        0x4E => if shift { b'_' } else { b'-' },
        0x55 => if shift { b'+' } else { b'=' },
        0x66 => 0x08, // backspace
        0x0D => b'\t',
        0x15 => if shift { b'Q' } else { b'q' },
        0x1D => if shift { b'W' } else { b'w' },
        0x24 => if shift { b'E' } else { b'e' },
        0x2D => if shift { b'R' } else { b'r' },
        0x2C => if shift { b'T' } else { b't' },
        0x35 => if shift { b'Y' } else { b'y' },
        0x3C => if shift { b'U' } else { b'u' },
        0x43 => if shift { b'I' } else { b'i' },
        0x44 => if shift { b'O' } else { b'o' },
        0x4D => if shift { b'P' } else { b'p' },
        0x54 => if shift { b'{' } else { b'[' },
        0x5B => if shift { b'}' } else { b']' },
        0x5A => b'\r', // enter
        0x1C => if shift { b'A' } else { b'a' },
        0x1B => if shift { b'S' } else { b's' },
        0x23 => if shift { b'D' } else { b'd' },
        0x2B => if shift { b'F' } else { b'f' },
        0x34 => if shift { b'G' } else { b'g' },
        0x33 => if shift { b'H' } else { b'h' },
        0x3B => if shift { b'J' } else { b'j' },
        0x42 => if shift { b'K' } else { b'k' },
        0x4B => if shift { b'L' } else { b'l' },
        0x4C => if shift { b':' } else { b';' },
        0x52 => if shift { b'"' } else { b'\'' },
        0x0E => if shift { b'~' } else { b'`' },
        0x5D => if shift { b'|' } else { b'\\' },
        0x1A => if shift { b'Z' } else { b'z' },
        0x22 => if shift { b'X' } else { b'x' },
        0x21 => if shift { b'C' } else { b'c' },
        0x2A => if shift { b'V' } else { b'v' },
        0x32 => if shift { b'B' } else { b'b' },
        0x31 => if shift { b'N' } else { b'n' },
        0x3A => if shift { b'M' } else { b'm' },
        0x41 => if shift { b'<' } else { b',' },
        0x49 => if shift { b'>' } else { b'.' },
        0x4A => if shift { b'?' } else { b'/' },
        0x29 => b' ',
        _ => return None,
    };
    Some(c)
}

/// 轮询读取 PS/2 键盘一个字符（无按键返回 None；维护 Shift 与 break 状态）
pub fn poll_char() -> Option<u8> {
    if !KEYBOARD_READY.load(Ordering::SeqCst) {
        return None;
    }
    let status = inb(0x64);
    if status & 0x01 != 0 {
        let sc = inb(0x60);
        // 扫描码集2：0xF0 是 break 前缀，其后跟随的是释放码
        if sc == 0xF0 {
            BREAK_PENDING.store(true, Ordering::SeqCst);
            return None;
        }
        let break_sc = BREAK_PENDING.load(Ordering::SeqCst);
        BREAK_PENDING.store(false, Ordering::SeqCst);
        // 左右 Shift（集2 make：0x12 / 0x59）
        if sc == 0x12 || sc == 0x59 {
            SHIFT.store(!break_sc, Ordering::SeqCst);
            return None;
        }
        if break_sc {
            return None; // 键释放，忽略
        }
        scancode_to_ascii(sc, SHIFT.load(Ordering::SeqCst))
    } else {
        None
    }
}
