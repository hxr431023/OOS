#![no_std]
//! Display layer: GOP framebuffer (UEFI graphics mode) preferred, else fall back
//! to 0xB8000 VGA text mode (BIOS/legacy).
use core::fmt;
use core::sync::atomic::{AtomicUsize, Ordering};

const VGA_BUF: *mut u8 = 0xB8000 as *mut u8;
const COLS: usize = 80;
const ROWS: usize = 25;
static CURSOR: AtomicUsize = AtomicUsize::new(0);
/// 1 = GOP framebuffer, 0 = 0xB8000 text mode
static FB_MODE: AtomicUsize = AtomicUsize::new(0);

pub fn init() {
    let f = crate::memory::framebuffer();
    crate::println!("[vga] base=0x{:X} {}x{} stride={} bpp={} fmt={}", f.base, f.width, f.height, f.stride, f.bpp, f.format);
    // 有效性防御：fb_base 必须落在合理物理内存区（>=16MB 且 <4GB），且 bpp=4、分辨率有效，
    // 否则视为无 GOP，回退 0xB8000，避免垃圾值误判触发 framebuffer 崩溃。
    if f.base >= 0x1000000
        && f.base <= 0xFFFF_FFFF
        && f.bpp == 4
        && f.width > 0
        && f.height > 0
        && f.stride >= f.width * f.bpp
    {
        crate::fb::init();
        // 读回验证：清屏后 base 处应为白色 255,255,255；若为洋红 255,0,255 说明 base 写错/未生效
        let b = f.base as *const u8;
        let v0 = unsafe { core::ptr::read_volatile(b) };
        let v1 = unsafe { core::ptr::read_volatile(b.add(1)) };
        let v2 = unsafe { core::ptr::read_volatile(b.add(2)) };
        crate::println!("[fbchk] base字节=({},{},{})", v0, v1, v2);
        // 扫描候选 framebuffer base，找 VirtualBox 真实渲染地址（未动应为洋红 255,0,255）
        for cand in [0xE0000000u64, 0xE000000A, 0xE8000000, 0xF0000000, 0xFD000000] {
            let cb = cand as *const u8;
            let c0 = unsafe { core::ptr::read_volatile(cb) };
            let c1 = unsafe { core::ptr::read_volatile(cb.add(1)) };
            let c2 = unsafe { core::ptr::read_volatile(cb.add(2)) };
            crate::println!("[fbscan] 0x{:X}=({},{},{})", cand, c0, c1, c2);
        }
        FB_MODE.store(1, Ordering::SeqCst);
        return;
    }
    // 0xB8000 text mode
    unsafe {
        for i in 0..(COLS * ROWS) {
            *VGA_BUF.add(i * 2) = 0;
            *VGA_BUF.add(i * 2 + 1) = 0x07;
        }
    }
    CURSOR.store(0, Ordering::SeqCst);
    FB_MODE.store(0, Ordering::SeqCst);
}

/// Write one byte to display (auto-select GOP framebuffer or 0xB8000)
pub fn put_char(c: u8) {
    if FB_MODE.load(Ordering::SeqCst) == 1 {
        crate::fb::put_char(c);
        return;
    }
    let mut idx = CURSOR.load(Ordering::SeqCst);
    match c {
        b'\n' => {
            let row = idx / COLS;
            if row + 1 >= ROWS {
                scroll();
            } else {
                CURSOR.store((row + 1) * COLS, Ordering::SeqCst);
            }
        }
        0x08 => {
            if idx > 0 {
                CURSOR.store(idx - 1, Ordering::SeqCst);
            }
        }
        _ => {
            unsafe {
                *VGA_BUF.add(idx * 2) = c;
                *VGA_BUF.add(idx * 2 + 1) = 0x07;
            }
            idx += 1;
            CURSOR.store(idx, Ordering::SeqCst);
            if idx / COLS >= ROWS {
                scroll();
            }
        }
    }
}

fn scroll() {
    unsafe {
        for row in 1..ROWS {
            for col in 0..COLS {
                let src = row * COLS + col;
                *VGA_BUF.add((src - COLS) * 2) = *VGA_BUF.add(src * 2);
                *VGA_BUF.add((src - COLS) * 2 + 1) = *VGA_BUF.add(src * 2 + 1);
            }
        }
        for col in 0..COLS {
            let last = (ROWS - 1) * COLS * 2;
            *VGA_BUF.add(last + col * 2) = 0;
            *VGA_BUF.add(last + col * 2 + 1) = 0x07;
        }
    }
    CURSOR.store((ROWS - 1) * COLS, Ordering::SeqCst);
}

/// 输出字符串不换行（提示符用）
pub fn put_str(s: &str) {
    if FB_MODE.load(Ordering::SeqCst) == 1 {
        crate::fb::put_str(s);
        return;
    }
    for b in s.as_bytes() {
        put_char(*b);
    }
}

pub struct VgaOut;
impl fmt::Write for VgaOut {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.as_bytes() {
            put_char(*b);
        }
        Ok(())
    }
}
