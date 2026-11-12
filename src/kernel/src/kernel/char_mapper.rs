#![no_std]
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
