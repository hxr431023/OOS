#![no_std]
//! CJK 字形渲染器：加载 HYCF 位图字体（宿主机用思源黑体预渲染的 16×16 点阵），
//! 对中文 codepoint 直接查表返回位图。render 接口与 fb.rs 的 put_char 对接。
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use alloc::vec;
use crate::sync::{Lazy, Mutex};

/// 渲染字号（像素）。16×16 全角，占 2 个 8×16 ASCII 格宽。
const GLYPH_SIZE: u32 = 16;

struct Renderer {
    /// cp -> 16×16 位图（16 行 × 2 字节，每行 16 bit，bit7=最左，1=实）
    map: BTreeMap<u32, [u8; 32]>,
}

static R: Lazy<Mutex<Renderer>> = Lazy::new(|| Mutex::new(Renderer { map: BTreeMap::new() }));

/// 是否已加载可渲染的 CJK 字体
pub fn ready() -> bool {
    !R.lock().map.is_empty()
}

/// 加载 HYCF 位图字体。格式：magic "HYCF" + u32 LE 字数 + (u32 LE cp + 32B 位图)*。
/// 返回是否成功解析。
pub fn set_font(data: &[u8]) -> bool {
    if data.len() < 8 || &data[0..4] != b"HYCF" {
        return false;
    }
    let count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
    let mut map = BTreeMap::new();
    let mut p = 8usize;
    for _ in 0..count {
        if p + 36 > data.len() {
            break;
        }
        let cp = u32::from_le_bytes([data[p], data[p + 1], data[p + 2], data[p + 3]]);
        let mut bm = [0u8; 32];
        bm.copy_from_slice(&data[p + 4..p + 36]);
        map.insert(cp, bm);
        p += 36;
    }
    if map.is_empty() {
        return false;
    }
    *R.lock() = Renderer { map };
    true
}

/// 渲染一个 codepoint 为 (w, h, bitmap)，失败返回 None。
/// bitmap 每像素 1 字节（0=空，255=实），前景色由调用方决定。
pub fn render(cp: u32) -> Option<(u32, u32, Vec<u8>)> {
    let r = R.lock();
    let bm = r.map.get(&cp)?;
    let mut out = Vec::with_capacity(16 * 16);
    for row in 0..16 {
        let b0 = bm[row * 2];
        let b1 = bm[row * 2 + 1];
        for col in 0..16 {
            let (byte, bit) = if col < 8 { (b0, 7 - col) } else { (b1, 15 - col) };
            let on = ((byte >> bit) & 1) == 1;
            out.push(if on { 255 } else { 0 });
        }
    }
    Some((GLYPH_SIZE, GLYPH_SIZE, out))
}

/// 已加载字形数（诊断）
pub fn cache_count() -> usize {
    R.lock().map.len()
}
