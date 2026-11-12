#![no_std]
//! GOP framebuffer 显示层（方案A）：内核直接往 UEFI framebuffer 写像素绘制字符。
//! 无 GOP（base=0，BIOS/legacy）时由 vga.rs 回退 0xB8000 文本模式。
use crate::font8x8::FONT8X8;
use crate::memory::FrameBufferInfo;
use core::sync::atomic::{AtomicUsize, Ordering};

const FONT_W: usize = 8;
const FONT_H: usize = 8;
/// 8x16 点阵：水平不放大（8px 宽）、垂直放大 2x（16px 高），贴近 VGA 文本字体比例
const VX: usize = 1;
const VY: usize = 2;
const CHAR_W: usize = FONT_W * VX;
const CHAR_H: usize = FONT_H * VY;

static CURSOR: AtomicUsize = AtomicUsize::new(0);
static FB_ACTIVE: AtomicUsize = AtomicUsize::new(0);

fn put_pixel(
    base: *mut u8,
    x: usize,
    y: usize,
    stride: usize,
    bpp: usize,
    fmt: u32,
    r: u8,
    g: u8,
    b: u8,
) {
    unsafe {
        let p = base.add(y * stride + x * bpp);
        // 实测 VirtualBox framebuffer 布局为 [R, 0, B, G]：G 通道在 byte3
        // （写前3字节时 G 在 byte1 被 VirtualBox 忽略，绿色丢失）
        *p = r;          // byte0 = R
        *p.add(1) = 0;   // byte1 = 0
        *p.add(2) = b;   // byte2 = B
        *p.add(3) = g;   // byte3 = G
    }
}

/// 串口诊断：直接向 COM1 输出无符号整数十六进制（早于显示栈就绪，不能走 println）
fn ser_hex(mut v: usize) {
    unsafe {
        core::arch::asm!("mov dx, 0x3F8", "mov al, '#'", "out dx, al", options(nomem, nostack));
        for shift in (0..16).rev() {
            let d = (v >> (shift * 4)) & 0xF;
            let b = if d < 10 { b'0' + d as u8 } else { b'A' + (d as u8 - 10) };
            core::arch::asm!("mov dx, 0x3F8", "out dx, al", in("al") b, options(nomem, nostack));
        }
        core::arch::asm!("mov dx, 0x3F8", "mov al, '\n'", "out dx, al", options(nomem, nostack));
    }
}

pub fn init() {
    unsafe { core::arch::asm!("mov dx, 0x3F8", "mov al, 'F'", "out dx, al", options(nomem, nostack)); }
    let f = crate::memory::framebuffer();
    if f.base == 0 {
        return; // 无 GOP，回退 0xB8000
    }
    // 诊断：打印 GOP framebuffer 参数（base/w/h/stride/bpp/fmt）
    ser_hex(f.base as usize);
    ser_hex(f.width as usize);
    ser_hex(f.height as usize);
    ser_hex(f.stride as usize);
    ser_hex(f.bpp as usize);
    ser_hex(f.format as usize);
    let base = f.base as *mut u8;
    let stride = f.stride as usize;
    let bpp = f.bpp as usize;
    // 清屏黑底（Linux 终端风格黑底白字；黑色像素 VirtualBox 显示正常，白色易被当透明）
    for y in 0..f.height as usize {
        for x in 0..f.width as usize {
            put_pixel(base, x, y, stride, bpp, f.format, 0x00, 0x00, 0x00);
        }
    }
    CURSOR.store(0, Ordering::SeqCst);
    FB_ACTIVE.store(1, Ordering::SeqCst);
}

fn draw_glyph(f: &FrameBufferInfo, base: *mut u8, x0: usize, y0: usize, glyph: &[u8; 8]) {
    let stride = f.stride as usize;
    let bpp = f.bpp as usize;
    // 先清字符格背景（黑色），避免滚动/重绘残留
    for dy in 0..CHAR_H {
        for dx in 0..CHAR_W {
            put_pixel(base, x0 + dx, y0 + dy, stride, bpp, f.format, 0x00, 0x00, 0x00);
        }
    }
    // 逐字体像素放大绘制（绿色前景，Linux 终端经典风格；避开 VirtualBox 洋红叠加对纯白的染色）
    for (row, line) in glyph.iter().enumerate() {
        for col in 0..8 {
            if line & (1 << (7 - col)) != 0 {
                for sy in 0..VY {
                    for sx in 0..VX {
                        put_pixel(base, x0 + col * VX + sx, y0 + row * VY + sy, stride, bpp, f.format, 0x00, 0xFF, 0x00);
                    }
                }
            }
        }
    }
}

fn scroll(f: &FrameBufferInfo, base: *mut u8) {
    let stride = f.stride as usize;
    let bpp = f.bpp as usize;
    let w = f.width as usize;
    let h = f.height as usize;
    // 上移一行字符（CHAR_H 像素）
    for y in 0..(h - CHAR_H) {
        for x in 0..w {
            let src = (y + CHAR_H) * stride + x * bpp;
            let dst = y * stride + x * bpp;
            for k in 0..bpp {
                unsafe {
                    *base.add(dst + k) = *base.add(src + k);
                }
            }
        }
    }
    // 清最后一行字符（黑底）
    for y in (h - CHAR_H)..h {
        for x in 0..w {
            put_pixel(base, x, y, stride, bpp, f.format, 0x00, 0x00, 0x00);
        }
    }
}

/// 重新清屏（Linux 终端黑底；在显示层就绪后调用，覆盖 VirtualBox 早期残留背景）
pub fn clear() {
    if FB_ACTIVE.load(Ordering::SeqCst) == 0 {
        return;
    }
    let f = crate::memory::framebuffer();
    let base = f.base as *mut u8;
    let stride = f.stride as usize;
    let bpp = f.bpp as usize;
    // 清屏黑底（Linux 终端黑底绿字）
    for y in 0..f.height as usize {
        for x in 0..f.width as usize {
            put_pixel(base, x, y, stride, bpp, f.format, 0x00, 0x00, 0x00);
        }
    }
    CURSOR.store(0, Ordering::SeqCst);
}

/// 在 idx 字符格画整块光标（绿色方块，明显可见；覆盖空格位置不影响字符）
fn draw_cursor(f: &FrameBufferInfo, base: *mut u8, idx: usize) {
    let cols = (f.width as usize) / CHAR_W;
    let row = idx / cols;
    let col = idx % cols;
    let stride = f.stride as usize;
    let bpp = f.bpp as usize;
    for dy in 0..CHAR_H {
        for dx in 0..CHAR_W {
            put_pixel(base, col * CHAR_W + dx, row * CHAR_H + dy, stride, bpp, f.format, 0x00, 0xFF, 0x00);
        }
    }
}

/// 把 CJK 位图（w×h，每像素 1 字节 0/255）绘制到 (x0,y0) 起，绿色前景、黑底
fn draw_bitmap(f: &FrameBufferInfo, base: *mut u8, x0: usize, y0: usize, bmp: &[u8], w: usize, h: usize) {
    let stride = f.stride as usize;
    let bpp = f.bpp as usize;
    for dy in 0..h {
        for dx in 0..w {
            let on = bmp[dy * w + dx] != 0;
            if on {
                put_pixel(base, x0 + dx, y0 + dy, stride, bpp, f.format, 0x00, 0xFF, 0x00);
            } else {
                put_pixel(base, x0 + dx, y0 + dy, stride, bpp, f.format, 0x00, 0x00, 0x00);
            }
        }
    }
}

/// 擦除 idx 字符格（清成黑底）
fn erase_cursor(f: &FrameBufferInfo, base: *mut u8, idx: usize) {
    let cols = (f.width as usize) / CHAR_W;
    let row = idx / cols;
    let col = idx % cols;
    let stride = f.stride as usize;
    let bpp = f.bpp as usize;
    for dy in 0..CHAR_H {
        for dx in 0..CHAR_W {
            put_pixel(base, col * CHAR_W + dx, row * CHAR_H + dy, stride, bpp, f.format, 0x00, 0x00, 0x00);
        }
    }
}

// UTF-8 多字节解码状态
static UTF8_N: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
static UTF8_CP: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// 喂一个字节到 UTF-8 状态机，返回完整 codepoint（None=中间字节/待续）
fn utf8_feed(c: u8) -> Option<u32> {
    use core::sync::atomic::Ordering;
    if c < 0x80 {
        UTF8_N.store(0, Ordering::SeqCst);
        UTF8_CP.store(0, Ordering::SeqCst);
        return Some(c as u32);
    }
    let n = UTF8_N.load(Ordering::SeqCst);
    let mut cp = UTF8_CP.load(Ordering::SeqCst);
    if n == 0 {
        if c >= 0xC0 && c < 0xE0 {
            cp = (c & 0x1F) as usize;
            UTF8_N.store(1, Ordering::SeqCst);
        } else if c >= 0xE0 && c < 0xF0 {
            cp = (c & 0x0F) as usize;
            UTF8_N.store(2, Ordering::SeqCst);
        } else if c >= 0xF0 && c < 0xF8 {
            cp = (c & 0x07) as usize;
            UTF8_N.store(3, Ordering::SeqCst);
        } else {
            UTF8_N.store(0, Ordering::SeqCst);
        }
        UTF8_CP.store(cp, Ordering::SeqCst);
        None
    } else {
        cp = (cp << 6) | (c & 0x3F) as usize;
        let n2 = n - 1;
        UTF8_N.store(n2, Ordering::SeqCst);
        UTF8_CP.store(cp, Ordering::SeqCst);
        if n2 == 0 {
            Some(cp as u32)
        } else {
            None
        }
    }
}

pub fn put_char(c: u8) {
    if FB_ACTIVE.load(Ordering::SeqCst) == 0 {
        return;
    }
    let f = crate::memory::framebuffer();
    let base = f.base as *mut u8;
    let cols = (f.width as usize) / CHAR_W;
    let rows = (f.height as usize) / CHAR_H;
    let mut idx = CURSOR.load(Ordering::SeqCst);
    // 控制字符立即处理（清 UTF-8 状态）
    if c == b'\n' || c == b'\r' || c == 0x08 {
        UTF8_N.store(0, Ordering::SeqCst);
        UTF8_CP.store(0, Ordering::SeqCst);
        erase_cursor(&f, base, idx);
        match c {
            b'\n' => {
                let row = idx / cols;
                if row + 1 >= rows {
                    scroll(&f, base);
                    idx = (rows - 1) * cols;
                } else {
                    idx = (row + 1) * cols;
                }
            }
            b'\r' => {
                idx = (idx / cols) * cols;
            }
            _ => {
                if idx > 0 {
                    idx -= 1;
                }
            }
        }
        CURSOR.store(idx, Ordering::SeqCst);
        draw_cursor(&f, base, idx);
        return;
    }
    // UTF-8 解码，得到完整 codepoint 才绘制
    if let Some(cp) = utf8_feed(c) {
        erase_cursor(&f, base, idx);
        if cp < 0x80 {
            let row = idx / cols;
            let col = idx % cols;
            if cp >= 0x20 {
                draw_glyph(&f, base, col * CHAR_W, row * CHAR_H, &FONT8X8[(cp as usize - 0x20)]);
            }
            idx += 1;
        } else {
            // CJK/多字节：用思源黑体光栅化位图，占 2 个 ASCII 格宽（16px）
            let row = idx / cols;
            let col = idx % cols;
            if let Some((w, h, bmp)) = crate::cjk_render::render(cp) {
                draw_bitmap(&f, base, col * CHAR_W, row * CHAR_H, &bmp, w as usize, h as usize);
            }
            idx += 2; // 全角
        }
        if idx / cols >= rows {
            scroll(&f, base);
            idx = (rows - 1) * cols;
        }
        CURSOR.store(idx, Ordering::SeqCst);
        draw_cursor(&f, base, idx);
    }
}

/// 输出字符串不换行（提示符用，避免 println 的 \n 把光标带到下一行）
pub fn put_str(s: &str) {
    for b in s.as_bytes() {
        put_char(*b);
    }
}
