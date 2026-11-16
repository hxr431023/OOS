//! CJK 字形渲染器（按需加载）：set_font 只保存 HYCF 原始数据，不预建全量字形表；
//! render(cp) 时才在该数据中查找对应字符的位图——用到哪个字符才取哪个，不全部加载。
use alloc::vec::Vec;
use crate::sync::{Lazy, Mutex};

/// 渲染字号（像素）。16×16 全角。
const GLYPH_SIZE: u32 = 16;

struct Renderer {
    data: Vec<u8>, // HYCF 原始字节：magic "HYCF" + u32 count + (u32 cp + 32B 位图)*
    count: u32,
}

static R: Lazy<Mutex<Renderer>> =
    Lazy::new(|| Mutex::new(Renderer { data: Vec::new(), count: 0 }));

/// 是否已加载字体数据
pub fn ready() -> bool {
    R.lock().count > 0
}

/// 加载字体数据（只保存原始字节，不建全量映射）。返回是否成功。
pub fn set_font(data: &[u8]) -> bool {
    if data.len() < 8 || &data[0..4] != b"HYCF" {
        return false;
    }
    let count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    if data.len() < 8 + count as usize * 36 {
        return false;
    }
    let mut r = R.lock();
    r.data = data.to_vec();
    r.count = count;
    true
}

/// 按需渲染一个 codepoint：扫描数据找到该字符的 32B 位图再展开。
/// 不预建全量映射，只取当前请求的这一字符（font.md：用到才加载那一部分）。
pub fn render(cp: u32) -> Option<(u32, u32, Vec<u8>)> {
    let r = R.lock();
    let d = &r.data;
    let n = r.count as usize;
    let mut p = 8usize;
    for _ in 0..n {
        if p + 36 > d.len() {
            break;
        }
        let c = u32::from_le_bytes([d[p], d[p + 1], d[p + 2], d[p + 3]]);
        if c == cp {
            let mut bm = [0u8; 32];
            bm.copy_from_slice(&d[p + 4..p + 36]);
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
            return Some((GLYPH_SIZE, GLYPH_SIZE, out));
        }
        p += 36;
    }
    None
}

/// 可用字形数（诊断，不展开位图）
pub fn cache_count() -> usize {
    R.lock().count as usize
}

/// 全部码点（"查看全部字符"时才扫描一次，平时不加载）
pub fn codepoints() -> Vec<u32> {
    let r = R.lock();
    let d = &r.data;
    let n = r.count as usize;
    let mut v = Vec::with_capacity(n);
    let mut p = 8usize;
    for _ in 0..n {
        if p + 36 > d.len() {
            break;
        }
        v.push(u32::from_le_bytes([d[p], d[p + 1], d[p + 2], d[p + 3]]));
        p += 36;
    }
    v
}

/// 释放字体数据（不再使用时清空）
pub fn unload() {
    let mut r = R.lock();
    r.data = Vec::new();
    r.count = 0;
}
