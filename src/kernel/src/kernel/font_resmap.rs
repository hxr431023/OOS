#![no_std]
//! 字体资源文件映射器（font.md 字体子系统——第二文件）
//! 负责：维护 **序号(seq) ↔ 字体资源** 双向映射表（font.md §4），
//!       每行记录一个字体资源的 序号/字体标识(font_id)/位置(path)/格式(format)；
//!       字体资源只读 + 加载前校验，对外经 font.api（.api 桥接层）暴露。
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use crate::font_loader::{FontFormat, detect_format};
use crate::sync::{Lazy, Mutex};

/// 一条字体资源记录（font.md §4.1 resmap 表的一行）
#[derive(Debug, Clone)]
pub struct FontResource {
    pub seq: u32,       // 序号：字体资源在系统中的唯一序号（font.md §4.1）
    pub font_id: String, // 字体语义标识（名称/字体族）
    pub path: String,   // 位置（在哪）：/system/media/... 或 .lerf 内嵌路径
    pub format: FontFormat, // 格式（什么格式）：ttf/otf/woff/woff2/lerf 等
    pub data: Vec<u8>,  // 映射后的字体字节流（供加载器解析）
}

/// 主映射表：font_id → 资源记录（序号在记录内，另建 seq→font_id 反查表）
static RES_MAP: Lazy<Mutex<BTreeMap<String, FontResource>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));
/// 序号 → font_id 反查表（font.md §4.1 双向映射）
static SEQ_MAP: Lazy<Mutex<BTreeMap<u32, String>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));
/// 递增序号分配器
static NEXT_SEQ: AtomicU32 = AtomicU32::new(1);

fn resmap() -> &'static Mutex<BTreeMap<String, FontResource>> {
    Lazy::force(&RES_MAP)
}
fn seqmap() -> &'static Mutex<BTreeMap<u32, String>> {
    Lazy::force(&SEQ_MAP)
}

/// 注册字体资源（font.md §3：字体按 font_id 加载；识别失败拒绝登记）
/// 自动分配递增序号，并建立 seq↔font_id 双向映射。返回 true 表示登记成功。
pub fn register(font_id: &str, path: &str, data: Vec<u8>) -> bool {
    let format = detect_format(&data);
    if format == FontFormat::Unknown {
        return false;
    }
    let seq = NEXT_SEQ.fetch_add(1, Ordering::Relaxed);
    resmap().lock().insert(
        font_id.to_string(),
        FontResource {
            seq,
            font_id: font_id.to_string(),
            path: path.to_string(),
            format,
            data,
        },
    );
    seqmap().lock().insert(seq, font_id.to_string());
    true
}

/// 按 font_id 查询字体资源（font_id → 资源，返回克隆避免持锁）
pub fn lookup(font_id: &str) -> Option<FontResource> {
    resmap().lock().get(font_id).cloned()
}

/// 按序号查询字体资源（seq → 资源，font.md §4.1 双向映射）
pub fn by_seq(seq: u32) -> Option<FontResource> {
    let id = seqmap().lock().get(&seq)?.clone();
    resmap().lock().get(&id).cloned()
}

/// 给定 font_id 返回对应序号（资源 → seq，font.md §4.1 双向映射）
pub fn seq_of(font_id: &str) -> Option<u32> {
    resmap().lock().get(font_id).map(|r| r.seq)
}

/// 解析单个字体资源的**位置与格式**（font.api 的 font: resolve）
/// seq 与 font_id 可任选其一：seq 优先，其次 font_id。
pub fn resolve(seq: Option<u32>, font_id: Option<&str>) -> Option<FontResource> {
    if let Some(s) = seq {
        if let Some(r) = by_seq(s) {
            return Some(r);
        }
    }
    if let Some(id) = font_id {
        return lookup(id);
    }
    None
}

/// 注销字体资源（含双向映射清理）
pub fn unregister(font_id: &str) -> bool {
    let mut m = resmap().lock();
    if let Some(r) = m.remove(font_id) {
        seqmap().lock().remove(&r.seq);
        return true;
    }
    false
}

/// 列出全部已登记字体 id（font.api 的 font: list 读条目的数据源）
pub fn list() -> Vec<String> {
    resmap().lock().keys().cloned().collect()
}

/// 完整 resmap 表（font.api 的 font: resmap 透传数据源）
pub fn resmap_table() -> Vec<FontResource> {
    resmap().lock().values().cloned().collect()
}

/// 已登记字体数量
pub fn count() -> usize {
    resmap().lock().len()
}
