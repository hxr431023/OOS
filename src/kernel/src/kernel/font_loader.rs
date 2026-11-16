//! 字体加载器（font.md 字体子系统——第一文件）
//! 负责：加载字体文件字节流 → 识别公开字体格式（font.md §2 完整清单）
//!        → 解析统一字形模型（font.md §1/§5），供字符映射器与 GUI 渲染使用。
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

/// 公开字体格式（font.md §2 完整清单）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFormat {
    TrueType,          // .ttf
    OpenType,          // .otf / .ttf (PostScript 轮廓封装)
    Woff,              // .woff
    Woff2,             // .woff2
    Eot,               // .eot
    TrueTypeCollection,// .ttc
    Type1,             // .pfb / .pfa
    Bitmap,            // .fon / .fnt
    Lerf,              // .lerf 语言资源字源（font.md §5）
    Unknown,
}

impl FontFormat {
    pub fn name(&self) -> &'static str {
        match self {
            FontFormat::TrueType => "TrueType",
            FontFormat::OpenType => "OpenType",
            FontFormat::Woff => "WOFF",
            FontFormat::Woff2 => "WOFF2",
            FontFormat::Eot => "EOT",
            FontFormat::TrueTypeCollection => "TrueTypeCollection",
            FontFormat::Type1 => "Type1/PostScript",
            FontFormat::Bitmap => "Bitmap",
            FontFormat::Lerf => "LERF",
            FontFormat::Unknown => "Unknown",
        }
    }
    pub fn ext(&self) -> &'static str {
        match self {
            FontFormat::TrueType => "ttf",
            FontFormat::OpenType => "otf",
            FontFormat::Woff => "woff",
            FontFormat::Woff2 => "woff2",
            FontFormat::Eot => "eot",
            FontFormat::TrueTypeCollection => "ttc",
            FontFormat::Type1 => "pfb/pfa",
            FontFormat::Bitmap => "fon/fnt",
            FontFormat::Lerf => "lerf",
            FontFormat::Unknown => "?",
        }
    }
}

/// 按文件头 magic 识别字体格式（font.md §2 兼容策略：全部原生识别）
pub fn detect_format(data: &[u8]) -> FontFormat {
    if data.len() < 4 {
        return FontFormat::Unknown;
    }
    let d = &data[..4];
    // .lerf 语言资源文件：首行魔数 "LERF."（; 分割编码）
    if data.len() >= 5 && &data[..5] == b"LERF." {
        return FontFormat::Lerf;
    }
    // TrueType: 0x00010000；OpenType: "OTTO"；TrueTypeCollection: "ttcf"
    if d == [0x00, 0x01, 0x00, 0x00] {
        return FontFormat::TrueType;
    }
    if d == b"OTTO" {
        return FontFormat::OpenType;
    }
    if d == b"ttcf" {
        return FontFormat::TrueTypeCollection;
    }
    if d == b"wOFF" {
        return FontFormat::Woff;
    }
    if d == b"wOF2" {
        return FontFormat::Woff2;
    }
    // EOT：第 34 字节起为 "EOT" 标记
    if data.len() >= 37 && &data[34..37] == b"EOT" {
        return FontFormat::Eot;
    }
    // Type1：PFA 以 "%!" 开头；PFB 以 0x80 0x01 开头
    if data[0] == b'%' && data[1] == b'!' {
        return FontFormat::Type1;
    }
    if data[0] == 0x80 && data[1] == 0x01 {
        return FontFormat::Type1;
    }
    // 位图字体：FON / FONT 资源头
    if data.len() >= 4 && (&data[..3] == b"FON" || &data[..4] == b"FONT") {
        return FontFormat::Bitmap;
    }
    FontFormat::Unknown
}

/// 统一字形模型（font.md §1/§5）：各格式解析后收敛到此结构，渲染管线不区分来源
#[derive(Debug, Clone)]
pub struct Glyph {
    pub codepoint: u32,      // Unicode 码点
    pub glyph_index: u32,    // 字体内 glyph 索引（轮廓字体经 cmap 映射）
    pub advance: u32,        // 前进宽度（文本测量）
    pub w: u32,              // 栅格化宽度
    pub h: u32,              // 栅格化高度
    pub bitmap: Option<Vec<u8>>, // 位图字形像素（Bitmap 字体/预栅格化）
}

/// 字体元数据（sfnt 表头解析）
#[derive(Debug, Clone)]
pub struct FontInfo {
    pub family: String,
    pub format: FontFormat,
    pub num_glyphs: u32,
    pub units_per_em: u32,
    pub num_tables: u32,
}

/// 已加载字体：数据 + 元数据 + 统一字形集合
pub struct LoadedFont {
    pub info: FontInfo,
    pub data: Vec<u8>,
    pub glyphs: Vec<Glyph>,
}

/// 加载字体：识别格式 + 解析 sfnt 表目录，产出统一字形模型骨架
pub fn load_font(data: &[u8]) -> Result<LoadedFont, &'static str> {
    let format = detect_format(data);
    if format == FontFormat::Unknown {
        return Err("unknown font format");
    }
    if format == FontFormat::Lerf {
        // .lerf 程序标识符区（第 2–3 行）：图标/字样/哈希，取非空行作为字源标识
        let text = core::str::from_utf8(data).unwrap_or("");
        let mut lines = text.lines();
        let _magic = lines.next();
        let id1 = lines.next().unwrap_or("");
        let id2 = lines.next().unwrap_or("");
        let ident = if !id1.trim().is_empty() { id1.trim() } else { id2.trim() };
        return Ok(LoadedFont {
            info: FontInfo {
                family: ident.to_string(),
                format,
                num_glyphs: 0,
                units_per_em: 0,
                num_tables: 0,
            },
            data: data.to_vec(),
            glyphs: Vec::new(),
        });
    }
    let (num_tables, upem) = parse_sfnt(data);
    let family = read_name_table(data).unwrap_or_else(|| format!("font-{:?}", format));
    // 解析 cmap，构建 字符→glyph 映射（统一字形模型 + 字符映射器数据源）
    let mut glyphs = Vec::new();
    let cmap = parse_cmap(data);
    for (cp, gi) in &cmap {
        glyphs.push(Glyph {
            codepoint: *cp,
            glyph_index: *gi,
            advance: 0,
            w: 0,
            h: 0,
            bitmap: None,
        });
    }
    Ok(LoadedFont {
        info: FontInfo {
            family,
            format,
            num_glyphs: glyphs.len() as u32,
            units_per_em: upem,
            num_tables,
        },
        data: data.to_vec(),
        glyphs,
    })
}

/// 在 sfnt 表目录中按 4 字节 tag 查找表，返回 (offset, length)
fn find_table(data: &[u8], tag: &[u8; 4]) -> Option<(usize, usize)> {
    if data.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;
    let max = core::cmp::min(num_tables, (data.len() - 12) / 16);
    for i in 0..max {
        let off = 12 + i * 16;
        if &data[off..off + 4] == tag {
            let toff = u32::from_be_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]) as usize;
            let tlen = u32::from_be_bytes([data[off + 12], data[off + 13], data[off + 14], data[off + 15]]) as usize;
            if toff + tlen <= data.len() {
                return Some((toff, tlen));
            }
        }
    }
    None
}

/// 解析 cmap 表，产出 (codepoint, glyph_index) 映射集合。
/// 优先 format 12（UCS-4 完整 Unicode，CJK 思源黑体用），回退 format 4（BMP）。
fn parse_cmap(data: &[u8]) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let (cmap_off, _) = match find_table(data, b"cmap") {
        Some(x) => x,
        None => return out,
    };
    let d = &data[cmap_off..];
    if d.len() < 4 {
        return out;
    }
    let num_tables = u16::from_be_bytes([d[2], d[3]]) as usize;
    let mut fmt12: Option<usize> = None;
    let mut fmt4: Option<usize> = None;
    for i in 0..num_tables {
        let rec = 4 + i * 8;
        if rec + 8 > d.len() {
            break;
        }
        let platform = u16::from_be_bytes([d[rec], d[rec + 1]]);
        let encoding = u16::from_be_bytes([d[rec + 2], d[rec + 3]]);
        let sub_off = u32::from_be_bytes([d[rec + 4], d[rec + 5], d[rec + 6], d[rec + 7]]) as usize;
        if sub_off + 2 > d.len() {
            continue;
        }
        let fmt = u16::from_be_bytes([d[sub_off], d[sub_off + 1]]);
        // format 12 (3,10) 覆盖全 Unicode，优先
        if fmt == 12 && platform == 3 && encoding == 10 {
            fmt12 = Some(sub_off);
            break;
        }
        if fmt == 4 && platform == 3 && encoding == 1 && fmt4.is_none() {
            fmt4 = Some(sub_off);
        }
    }
    if let Some(off) = fmt12 {
        parse_fmt12(&d[off..], &mut out);
    } else if let Some(off) = fmt4 {
        parse_fmt4(&d[off..], &mut out);
    }
    out
}

/// cmap format 12：UCS-4 字符到 glyph 的分组映射
fn parse_fmt12(s: &[u8], out: &mut Vec<(u32, u32)>) {
    if s.len() < 16 {
        return;
    }
    let num_groups = u32::from_be_bytes([s[12], s[13], s[14], s[15]]);
    for i in 0..num_groups {
        let go = 16 + (i as usize) * 12;
        if go + 12 > s.len() {
            break;
        }
        let start = u32::from_be_bytes([s[go], s[go + 1], s[go + 2], s[go + 3]]);
        let end = u32::from_be_bytes([s[go + 4], s[go + 5], s[go + 6], s[go + 7]]);
        let sg = u32::from_be_bytes([s[go + 8], s[go + 9], s[go + 10], s[go + 11]]);
        for cp in start..=end {
            out.push((cp, sg + (cp - start)));
        }
    }
}

/// cmap format 4：BMP（2 字节字符）分段映射
fn parse_fmt4(s: &[u8], out: &mut Vec<(u32, u32)>) {
    if s.len() < 16 {
        return;
    }
    let seg_count_x2 = u16::from_be_bytes([s[6], s[7]]) as usize;
    let seg_count = seg_count_x2 / 2;
    let end_off = 14;
    let start_off = end_off + seg_count_x2 + 2; // +2 reservedPad
    let delta_off = start_off + seg_count_x2;
    let range_off = delta_off + seg_count_x2;
    let glyph_id_off = range_off + seg_count_x2;
    for i in 0..seg_count {
        let end = u16::from_be_bytes([s[end_off + i * 2], s[end_off + i * 2 + 1]]);
        if end == 0xFFFF {
            continue;
        }
        let start = u16::from_be_bytes([s[start_off + i * 2], s[start_off + i * 2 + 1]]);
        let delta = u16::from_be_bytes([s[delta_off + i * 2], s[delta_off + i * 2 + 1]]);
        let range = u16::from_be_bytes([s[range_off + i * 2], s[range_off + i * 2 + 1]]);
        for cp in start..=end {
            if cp == 0xFFFF {
                continue;
            }
            let gi = if range != 0 {
                let idx = (range as usize) + (cp as usize - start as usize);
                if glyph_id_off + idx * 2 + 2 <= s.len() {
                    u16::from_be_bytes([s[glyph_id_off + idx * 2], s[glyph_id_off + idx * 2 + 1]])
                } else {
                    0
                }
            } else {
                0
            };
            let final_gi = if gi != 0 {
                (gi as u32 + delta as u32) & 0xFFFF
            } else {
                (cp as u32 + delta as u32) & 0xFFFF
            };
            out.push((cp as u32, final_gi));
        }
    }
}

/// 解析 sfnt（ttf/otf）表目录：返回 (表数量, unitsPerEm)
/// sfnt 头：version(4) + numTables(2) + 表目录（每条 16 字节：tag/checksum/offset/length）
fn parse_sfnt(data: &[u8]) -> (u32, u32) {
    if data.len() < 12 {
        return (0, 0);
    }
    let num_tables = u16::from_be_bytes([data[4], data[5]]) as u32;
    let mut upem = 0u32;
    let max_tables = core::cmp::min(num_tables as usize, (data.len() - 12) / 16);
    for i in 0..max_tables {
        let off = 12 + i * 16;
        let tag = &data[off..off + 4];
        if tag == b"head" {
            let toff = u32::from_be_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]) as usize;
            // head 表：unitsPerEm 在 offset 18 (u16)
            if toff + 20 <= data.len() {
                upem = u16::from_be_bytes([data[toff + 18], data[toff + 19]]) as u32;
            }
        }
    }
    (num_tables, upem)
}

/// 从 name 表读取字体族名（font-family 记录，nameID=1）
fn read_name_table(data: &[u8]) -> Option<String> {
    if data.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;
    let max_tables = core::cmp::min(num_tables, (data.len() - 12) / 16);
    for i in 0..max_tables {
        let off = 12 + i * 16;
        let tag = &data[off..off + 4];
        if tag != b"name" {
            continue;
        }
        let toff = u32::from_be_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]]) as usize;
        if toff + 6 > data.len() {
            return None;
        }
        let count = u16::from_be_bytes([data[toff + 2], data[toff + 3]]) as usize;
        let storage_off = u16::from_be_bytes([data[toff + 4], data[toff + 5]]) as usize;
        for r in 0..count {
            let rec = toff + 6 + r * 12;
            if rec + 12 > data.len() {
                break;
            }
            let name_id = u16::from_be_bytes([data[rec + 6], data[rec + 7]]);
            let len = u16::from_be_bytes([data[rec + 8], data[rec + 9]]) as usize;
            let str_off = u16::from_be_bytes([data[rec + 10], data[rec + 11]]) as usize;
            // nameID=1 且为 0/3 平台（Unicode），取首条
            if name_id == 1 {
                let base = toff + storage_off + str_off;
                if base + len <= data.len() {
                    // 逐字节转 ASCII（拉丁字体族名通常为 ASCII）
                    let raw = &data[base..base + len];
                    let s: String = raw.iter().filter(|b| **b >= 0x20 && **b < 0x7F).map(|b| *b as char).collect();
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
            }
        }
    }
    None
}
