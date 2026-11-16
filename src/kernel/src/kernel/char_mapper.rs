//! 字符映射器（font.md 字体子系统——第三文件）
//! 负责：维护 Unicode 字符 → 字形索引（glyph index）的映射，
//!       由字体加载器解析出的 cmap 结果构建，供文本渲染查询（font.md §5）。
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use crate::font_loader::Glyph;
use crate::sync::{Lazy, Mutex};

/// 全局字符映射表：codepoint → glyph index
static CHAR_MAP: Lazy<Mutex<BTreeMap<u32, u32>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

fn charmap() -> &'static Mutex<BTreeMap<u32, u32>> {
    Lazy::force(&CHAR_MAP)
}

/// 注册单个 字符→glyph 映射
pub fn map_char(codepoint: u32, glyph_index: u32) {
    charmap().lock().insert(codepoint, glyph_index);
}

/// 查询字符的 glyph 索引（返回 None 表示字符未映射/缺字）
pub fn char_to_glyph(codepoint: u32) -> Option<u32> {
    charmap().lock().get(&codepoint).copied()
}

/// 从一组统一字形模型批量构建映射（加载器解析 cmap 后调用）
pub fn build_from_glyphs(glyphs: &[Glyph]) {
    let mut m = charmap().lock();
    for g in glyphs {
        m.insert(g.codepoint, g.glyph_index);
    }
}

/// 清空映射（重载字体时使用）
pub fn clear() {
    charmap().lock().clear();
}

/// 已映射字符数量
pub fn mapped_count() -> usize {
    charmap().lock().len()
}

/// 列出已映射码点（调试/审计用）
pub fn mapped_codepoints() -> Vec<u32> {
    charmap().lock().keys().copied().collect()
}

// ===== font.md §1/§6：字符映射器负责字形栅格化、查看全部字符、选中复制、文本测量 =====

/// 字形栅格化：中文走 cjk_render 的 16×16 位图；ASCII 走 FONT8X8 的 8×16（垂直 2x 放大）。
/// 返回 (宽, 高, 位图)，位图每像素 1 字节（0=空，255=实），与 fb.rs 位图格式一致。
/// 这是 font.md §6 渲染管线里"字符映射器按码点取字形并栅格化渲染"的统一入口。
pub fn render_glyph(cp: u32) -> Option<(u32, u32, Vec<u8>)> {
    if let Some(b) = crate::cjk_render::render(cp) {
        return Some(b);
    }
    if (0x20..=0x7E).contains(&cp) {
        let g = &crate::font8x8::FONT8X8[(cp as usize) - 0x20];
        let mut out = Vec::with_capacity(8 * 16);
        for row in 0..8 {
            let bits = g[row];
            for _dup in 0..2 {
                for col in 0..8 {
                    let on = ((bits >> (7 - col)) & 1) == 1;
                    out.push(if on { 255 } else { 0 });
                }
            }
        }
        return Some((8, 16, out));
    }
    None
}

/// 查看当前字体全部字符（font.md §1：打开字符映射器查看全部字符）。
/// 返回 (码点, 字符) 列表：先中文字形，再 ASCII 可打印区。
pub fn all_chars() -> Vec<(u32, char)> {
    let mut v: Vec<(u32, char)> = Vec::new();
    for cp in crate::cjk_render::codepoints() {
        if let Some(ch) = char::from_u32(cp) {
            v.push((cp, ch));
        }
    }
    for cp in 0x20..=0x7Eu32 {
        if let Some(ch) = char::from_u32(cp) {
            v.push((cp, ch));
        }
    }
    v
}

/// 选中单个字符并返回（font.md §1：选中字符后点击复制）。
/// 返回对应字符文本，供上层写入剪贴板 / 回显。
pub fn select_char(cp: u32) -> Option<char> {
    let ch = char::from_u32(cp)?;
    if ch.is_control() {
        return None;
    }
    Some(ch)
}

/// 文本测量（font.md §6.2）：ASCII 半角 8px，全角/中文 16px。
pub fn text_width(text: &str) -> u32 {
    let mut w = 0u32;
    for ch in text.chars() {
        let cp = ch as u32;
        if (0x20..=0x7E).contains(&cp) {
            w += 8;
        } else {
            w += 16;
        }
    }
    w
}
