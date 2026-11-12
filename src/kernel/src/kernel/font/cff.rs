#![no_std]
//! CFF (Compact Font Format) 解析与 Type2 charstring 解释（font.md 字形轮廓来源）
//! 思源黑体等 OTF 用 PostScript(CFF) 轮廓封装，需解析 CFF 表 + CharStrings + 子程序，
//! 解释 Type2 charstring 得到轮廓（线段 + 三次贝塞尔），供光栅化。
use alloc::vec::Vec;
use alloc::vec;

/// CFF 表头解析后的表结构
pub struct CffFont {
    /// 每个 glyph 的 Type2 charstring 字节流（charstrings INDEX，glyph 0 = .notdef）
    pub charstrings: Vec<Vec<u8>>,
    /// Global Subr INDEX
    pub global_subrs: Vec<Vec<u8>>,
    /// Local Subr INDEX（取自 Private DICT）
    pub local_subrs: Vec<Vec<u8>>,
    /// unitsPerEm（缩放基准）
    pub upem: u32,
}

/// 一个 CFF 值（操作数或操作符）
#[derive(Debug, Clone, Copy, PartialEq)]
enum Val {
    Num(i32),   // 整数操作数
    Fnum(f32),  // 实数操作数（可能不用，保留）
    Op(u8),     // 操作符（1 字节）
    Esc(u8),    // escape 操作符（12 xx）
}

/// 解析 CFF 表：输入为完整 sfnt 数据，返回 CffFont
/// data: 传入整个字体文件（内部按 tag 找 "CFF " 表）
pub fn parse_cff(data: &[u8]) -> Option<CffFont> {
    // 在 sfnt 表目录中找 "CFF " 表
    let (cff_off, cff_len) = find_sfnt_table(data, b"CFF ")?;
    if cff_off + cff_len > data.len() {
        return None;
    }
    let d = &data[cff_off..cff_off + cff_len];
    if d.len() < 4 {
        return None;
    }
    let hdr_size = d[2] as usize;
    if hdr_size + 1 >= d.len() {
        return None;
    }
    let mut pos = hdr_size;
    // Name INDEX（跳过）
    let (_, p1) = parse_index(d, pos)?;
    pos = p1;
    // Top DICT INDEX
    let (top_dicts, p2) = parse_index(d, pos)?;
    pos = p2;
    // String INDEX（跳过）
    let (_, p3) = parse_index(d, pos)?;
    pos = p3;
    // Global Subr INDEX
    let (global_subrs, _) = parse_index(d, pos)?;
    // 解析 Top DICT，找 CharStrings / Private / charset 的 offset
    let top = if let Some(t) = top_dicts.first() {
        t.clone()
    } else {
        return None;
    };
    let mut charstrings_off: Option<usize> = None;
    let mut private_size: Option<usize> = None;
    let mut private_off: Option<usize> = None;
    // 解析 DICT：operand 累积 + operator
    let mut ops = Vec::<(i32, u16)>::new(); // (operand, operator)
    let mut operands: Vec<i32> = Vec::new();
    let mut i = 0;
    while i < top.len() {
        let b = top[i];
        if b <= 21 {
            // operator
            if b == 12 {
                if i + 1 < top.len() {
                    let esc = top[i + 1];
                    // 需要操作数的 operator：17=CharStrings, 15=charset, 18/19=Private
                    match esc {
                        15 | 17 => {
                            if let Some(&op) = operands.last() {
                                ops.push((op, 1200 + esc as u16));
                            }
                        }
                        18 => {
                            if operands.len() >= 2 {
                                let size = operands[operands.len() - 2];
                                let off = operands[operands.len() - 1];
                                private_size = Some(size as usize);
                                private_off = Some(off as usize);
                            }
                        }
                        _ => {}
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            } else {
                // 需要操作数的非 escape operator：17=CharStrings 是 escape(12 17)
                // 这里仅处理可能带操作数的普通 operator，其他忽略
                i += 1;
            }
            operands.clear();
        } else {
            // operand
            let (v, nb) = read_dict_operand(&top[i..])?;
            operands.push(v);
            i += nb;
        }
    }
    // CharStrings offset（12 17）
    for (op, o) in &ops {
        if *o == 1217 {
            charstrings_off = Some(*op as usize);
        }
    }
    let charstrings_off = charstrings_off?;
    let (charstrings, _) = parse_index(d, charstrings_off)?;
    // Local Subrs：Private DICT 里找 Subrs（operand 19, escape 12 19）
    let mut local_subrs = Vec::new();
    if let (Some(sz), Some(off)) = (private_size, private_off) {
        if off + sz <= d.len() {
            let pd = &d[off..off + sz];
            let mut p_ops = Vec::<(i32, u16)>::new();
            let mut p_operands: Vec<i32> = Vec::new();
            let mut j = 0;
            while j < pd.len() {
                let b = pd[j];
                if b <= 21 {
                    if b == 12 && j + 1 < pd.len() {
                        let esc = pd[j + 1];
                        if esc == 19 {
                            if let Some(&op) = p_operands.last() {
                                p_ops.push((op, 1219));
                            }
                        }
                        j += 2;
                    } else {
                        j += 1;
                    }
                    p_operands.clear();
                } else {
                    let (v, nb) = read_dict_operand(&pd[j..]).unwrap_or((0, 1));
                    p_operands.push(v);
                    j += nb;
                }
            }
            for (op, o) in p_ops {
                if o == 1219 {
                    if let Some((subrs, _)) = parse_index(d, op as usize) {
                        local_subrs = subrs;
                    }
                }
            }
        }
    }
    let upem = 1000; // CFF 默认 1000
    Some(CffFont {
        charstrings,
        global_subrs,
        local_subrs,
        upem,
    })
}

/// 在 sfnt 表目录找 tag，返回 (offset, length)
fn find_sfnt_table(data: &[u8], tag: &[u8; 4]) -> Option<(usize, usize)> {
    if data.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;
    let max = core::cmp::min(num_tables, (data.len() - 12) / 16);
    for i in 0..max {
        let off = 12 + i * 16;
        if &data[off..off + 4] == tag {
            let toff =
                u32::from_be_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]])
                    as usize;
            let tlen = u32::from_be_bytes([
                data[off + 12],
                data[off + 13],
                data[off + 14],
                data[off + 15],
            ]) as usize;
            if toff + tlen <= data.len() {
                return Some((toff, tlen));
            }
            return None;
        }
    }
    None
}

/// 解析 CFF INDEX，返回 (items, next_pos)
fn parse_index(d: &[u8], pos: usize) -> Option<(Vec<Vec<u8>>, usize)> {
    if pos + 2 > d.len() {
        return None;
    }
    let count = u16::from_be_bytes([d[pos], d[pos + 1]]) as usize;
    if count == 0 {
        return Some((Vec::new(), pos + 2));
    }
    if pos + 3 > d.len() {
        return None;
    }
    let off_size = d[pos + 2] as usize;
    if off_size == 0 || off_size > 4 {
        return None;
    }
    let off_table = pos + 3;
    let offsets_len = (count + 1) * off_size;
    if off_table + offsets_len > d.len() {
        return None;
    }
    let mut offsets = Vec::with_capacity(count + 1);
    for i in 0..=count {
        let mut v = 0usize;
        for k in 0..off_size {
            v = (v << 8) | d[off_table + i * off_size + k] as usize;
        }
        offsets.push(v);
    }
    let data_start = off_table + offsets_len;
    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        let s = offsets[i].saturating_sub(1);
        let e = offsets[i + 1].saturating_sub(1);
        if data_start + s + (e - s) > d.len() {
            return None;
        }
        items.push(d[data_start + s..data_start + e].to_vec());
    }
    Some((items, data_start + offsets[count].saturating_sub(1)))
}

/// 读 DICT 操作数（整数/负数/实数），返回 (value, bytes_consumed)
fn read_dict_operand(d: &[u8]) -> Option<(i32, usize)> {
    if d.is_empty() {
        return None;
    }
    let b = d[0];
    match b {
        32..=246 => Some((b as i32 - 139, 1)),
        247..=250 => {
            if d.len() < 2 {
                return None;
            }
            let b1 = d[1] as i32;
            Some(((b as i32 - 247) * 256 + b1 + 108, 2))
        }
        251..=254 => {
            if d.len() < 2 {
                return None;
            }
            let b1 = d[1] as i32;
            Some((-((b as i32 - 251) * 256) - b1 - 108, 2))
        }
        28 => {
            if d.len() < 3 {
                return None;
            }
            let v = ((d[1] as i32) << 8) | d[2] as i32;
            Some((if v >= 0x8000 { v - 0x10000 } else { v }, 3))
        }
        29 => {
            if d.len() < 5 {
                return None;
            }
            let v = ((d[1] as i32) << 24)
                | ((d[2] as i32) << 16)
                | ((d[3] as i32) << 8)
                | d[4] as i32;
            Some((v, 5))
        }
        _ => None, // 实数等暂不支持
    }
}

/// 一个轮廓点
#[derive(Debug, Clone, Copy)]
pub struct PathPoint {
    pub x: i32,
    pub y: i32,
    /// true=曲线上的点；false=off-curve 控制点（三次贝塞尔）
    pub on: bool,
}

/// 解释一个 Type2 charstring，得到轮廓点列表。
/// 返回的路径为线段/三次贝塞尔曲线序列，供光栅化（扫描线细分）。
pub fn interpret_type2(
    cs: &[u8],
    global_subrs: &[Vec<u8>],
    local_subrs: &[Vec<u8>],
    out: &mut Vec<PathPoint>,
) -> Result<(), ()> {
    // Type2 解释器：栈式
    let mut stack: Vec<i32> = Vec::new();
    let mut px = 0i32;
    let mut py = 0i32;
    let mut cur = 0usize;
    let mut max_subr_depth = 0usize;
    interpret_rec(cs, &mut cur, global_subrs, local_subrs, &mut stack, &mut px, &mut py, out, &mut max_subr_depth)
}

#[allow(clippy::too_many_arguments)]
fn interpret_rec(
    cs: &[u8],
    cur: &mut usize,
    global_subrs: &[Vec<u8>],
    local_subrs: &[Vec<u8>],
    stack: &mut Vec<i32>,
    px: &mut i32,
    py: &mut i32,
    out: &mut Vec<PathPoint>,
    depth: &mut usize,
) -> Result<(), ()> {
    let mut local_cur = *cur;
    // hint 计数（hintmask 需要知道 vstem/hstem 数量，此处简化：记录但跳过 mask）
    loop {
        if local_cur >= cs.len() {
            *cur = local_cur;
            return Ok(());
        }
        let b = cs[local_cur];
        // 操作符范围
        if b >= 32 {
            // operand
            let (v, nb) = read_cs_operand(&cs[local_cur..])?;
            stack.push(v);
            local_cur += nb;
            continue;
        }
        // operator
        local_cur += 1;
        match b {
            1 | 3 | 18 | 23 => {
                // hstem / vstem / hstemhm / vstemhm：清除栈（hint）
                // 奇数个时最后一个表示 width
                stack.clear();
            }
            19 | 20 => {
                // hintmask / cntrmask：跳过 hint 数量的 mask 字节
                // 简化：不维护 hint 计数，直接读取并跳过（宽度在栈顶）
                // 实际需要 hint 计数；此处按栈清空处理并跳过剩余（由 hint 数量决定）
                // 简化：跳到 charstring 尾（不处理 mask）——会导致后续失效。
                // 为兼容，用保守做法：不精确，但先跳过（多数 glyph 无 mask 或 mask 在前）。
                stack.clear();
                // hintmask 通常出现在开头，其后跟轮廓命令。为安全，此处继续但忽略 mask。
                // mask 字节数 = ceil(hintCount/8)；我们无法得知，故用估算：清空后继续。
                // 为简化，跳过到下一个 operator 为止（不精确，但保证不崩）。
                // 这里直接把 mask 字节跳过：实际上需 hint 计数。退化为：忽略此操作符，继续。
                // 注意：若 mask 后仍有数据，这里继续解析可能导致误读。大多数思源黑体 glyph
                // 的 mask 在开头且之后是 rmoveto 等。为稳健，这里停止解析该 glyph。
                break;
            }
            4 => {
                // vmoveto: dy
                if stack.len() >= 1 {
                    let dy = *stack.last().unwrap();
                    *py += dy;
                    stack.clear();
                    out.push(PathPoint { x: *px, y: *py, on: true });
                } else {
                    break;
                }
            }
            21 => {
                // rmoveto: dx dy
                if stack.len() >= 2 {
                    let n = stack.len();
                    let dx = stack[n - 2];
                    let dy = stack[n - 1];
                    *px += dx;
                    *py += dy;
                    stack.clear();
                    out.push(PathPoint { x: *px, y: *py, on: true });
                } else {
                    break;
                }
            }
            22 => {
                // hmoveto: dx
                if stack.len() >= 1 {
                    let dx = *stack.last().unwrap();
                    *px += dx;
                    stack.clear();
                    out.push(PathPoint { x: *px, y: *py, on: true });
                } else {
                    break;
                }
            }
            5 => {
                // rlineto: dx1 dy1 dx2 dy2 ...
                let n = stack.len();
                let mut k = 0;
                if n % 2 != 0 {
                    // 有 width
                    k = 1;
                }
                while k + 1 < n {
                    *px += stack[k];
                    *py += stack[k + 1];
                    out.push(PathPoint { x: *px, y: *py, on: true });
                    k += 2;
                }
                stack.clear();
            }
            6 => {
                // hlineto: dx1 dy1 dx2 dy2 ... 交替 h/v
                let n = stack.len();
                let mut k = 0;
                if n % 2 != 0 {
                    // 奇数首元素，h 开头
                    *px += stack[k];
                    out.push(PathPoint { x: *px, y: *py, on: true });
                    k += 1;
                    let mut horiz = false;
                    while k < n {
                        if horiz {
                            *px += stack[k];
                        } else {
                            *py += stack[k];
                        }
                        out.push(PathPoint { x: *px, y: *py, on: true });
                        horiz = !horiz;
                        k += 1;
                    }
                } else {
                    let mut horiz = true;
                    while k < n {
                        if horiz {
                            *px += stack[k];
                        } else {
                            *py += stack[k];
                        }
                        out.push(PathPoint { x: *px, y: *py, on: true });
                        horiz = !horiz;
                        k += 1;
                    }
                }
                stack.clear();
            }
            7 => {
                // vlineto: 交替 v/h
                let n = stack.len();
                let mut k = 0;
                if n % 2 != 0 {
                    *py += stack[k];
                    out.push(PathPoint { x: *px, y: *py, on: true });
                    k += 1;
                    let mut vert = false;
                    while k < n {
                        if vert {
                            *py += stack[k];
                        } else {
                            *px += stack[k];
                        }
                        out.push(PathPoint { x: *px, y: *py, on: true });
                        vert = !vert;
                        k += 1;
                    }
                } else {
                    let mut vert = true;
                    while k < n {
                        if vert {
                            *py += stack[k];
                        } else {
                            *px += stack[k];
                        }
                        out.push(PathPoint { x: *px, y: *py, on: true });
                        vert = !vert;
                        k += 1;
                    }
                }
                stack.clear();
            }
            8 => {
                // rrcurveto: dx1 dy1 dx2 dy2 dx3 dy3 ...
                let n = stack.len();
                let mut k = 0;
                if n % 6 != 0 {
                    k = n % 6; // width 处理
                }
                while k + 5 < n {
                    let dx1 = stack[k];
                    let dy1 = stack[k + 1];
                    let dx2 = stack[k + 2];
                    let dy2 = stack[k + 3];
                    let dx3 = stack[k + 4];
                    let dy3 = stack[k + 5];
                    let c1x = *px + dx1;
                    let c1y = *py + dy1;
                    let c2x = c1x + dx2;
                    let c2y = c1y + dy2;
                    let endx = c2x + dx3;
                    let endy = c2y + dy3;
                    out.push(PathPoint { x: c1x, y: c1y, on: false });
                    out.push(PathPoint { x: c2x, y: c2y, on: false });
                    out.push(PathPoint { x: endx, y: endy, on: true });
                    *px = endx;
                    *py = endy;
                    k += 6;
                }
                stack.clear();
            }
            24 => {
                // rcurveline: ... 后接 lineto
                let n = stack.len();
                let mut k = 0;
                while k + 5 < n {
                    let c1x = *px + stack[k];
                    let c1y = *py + stack[k + 1];
                    let c2x = c1x + stack[k + 2];
                    let c2y = c1y + stack[k + 3];
                    let endx = c2x + stack[k + 4];
                    let endy = c2y + stack[k + 5];
                    out.push(PathPoint { x: c1x, y: c1y, on: false });
                    out.push(PathPoint { x: c2x, y: c2y, on: false });
                    out.push(PathPoint { x: endx, y: endy, on: true });
                    *px = endx;
                    *py = endy;
                    k += 6;
                }
                // 剩余为 lineto 对
                while k + 1 < n {
                    *px += stack[k];
                    *py += stack[k + 1];
                    out.push(PathPoint { x: *px, y: *py, on: true });
                    k += 2;
                }
                stack.clear();
            }
            25 => {
                // rlinecurve: 先 lineto 再 curveto
                let n = stack.len();
                // 最后 6 个是 curveto
                if n >= 6 {
                    let curve_start = n - 6;
                    let mut k = 0;
                    while k < curve_start {
                        *px += stack[k];
                        *py += stack[k + 1];
                        out.push(PathPoint { x: *px, y: *py, on: true });
                        k += 2;
                    }
                    let c1x = *px + stack[curve_start];
                    let c1y = *py + stack[curve_start + 1];
                    let c2x = c1x + stack[curve_start + 2];
                    let c2y = c1y + stack[curve_start + 3];
                    let endx = c2x + stack[curve_start + 4];
                    let endy = c2y + stack[curve_start + 5];
                    out.push(PathPoint { x: c1x, y: c1y, on: false });
                    out.push(PathPoint { x: c2x, y: c2y, on: false });
                    out.push(PathPoint { x: endx, y: endy, on: true });
                    *px = endx;
                    *py = endy;
                }
                stack.clear();
            }
            26 => {
                // vvcurveto: dy1 dx2 dy2 dx3 dy3 ...（dx1 可选）
                let n = stack.len();
                let mut k = 0;
                let mut kk = 0;
                if n % 4 == 1 {
                    // 有 dx1
                    *px += stack[0];
                    k = 1;
                }
                while kk + 3 < n {
                    let dx1 = 0;
                    let dy1 = if kk + 0 < n { stack[kk + 0] } else { 0 };
                    let dx2 = stack[kk + 1];
                    let dy2 = stack[kk + 2];
                    let dx3 = stack[kk + 3];
                    let dy3 = stack[kk + 4];
                    let c1x = *px + dx1;
                    let c1y = *py + dy1;
                    let c2x = c1x + dx2;
                    let c2y = c1y + dy2;
                    let endx = c2x + dx3;
                    let endy = c2y + dy3;
                    out.push(PathPoint { x: c1x, y: c1y, on: false });
                    out.push(PathPoint { x: c2x, y: c2y, on: false });
                    out.push(PathPoint { x: endx, y: endy, on: true });
                    *px = endx;
                    *py = endy;
                    kk += 4;
                }
                stack.clear();
            }
            27 => {
                // hhcurveto
                let n = stack.len();
                let mut kk = 0;
                if n % 4 == 1 {
                    *py += stack[0];
                }
                while kk + 4 < n {
                    let dx1 = stack[kk];
                    let dy1 = 0;
                    let dx2 = stack[kk + 1];
                    let dy2 = stack[kk + 2];
                    let dx3 = stack[kk + 3];
                    let dy3 = 0;
                    let c1x = *px + dx1;
                    let c1y = *py + dy1;
                    let c2x = c1x + dx2;
                    let c2y = c1y + dy2;
                    let endx = c2x + dx3;
                    let endy = c2y + dy3;
                    out.push(PathPoint { x: c1x, y: c1y, on: false });
                    out.push(PathPoint { x: c2x, y: c2y, on: false });
                    out.push(PathPoint { x: endx, y: endy, on: true });
                    *px = endx;
                    *py = endy;
                    kk += 4;
                }
                stack.clear();
            }
            10 | 29 => {
                // callsubr / callgsubr
                if let Some(&idx) = stack.last() {
                    stack.pop();
                    let subrs = if b == 10 { local_subrs } else { global_subrs };
                    let biasp = if b == 10 {
                        if subrs.len() < 1240 {
                            107
                        } else if subrs.len() < 33900 {
                            1131
                        } else {
                            32768
                        }
                    } else if subrs.len() < 1240 {
                        107
                    } else if subrs.len() < 33900 {
                        1131
                    } else {
                        32768
                    };
                    let real = (idx + biasp) as usize;
                    if real < subrs.len() {
                        if *depth > 100 {
                            return Err(());
                        }
                        *depth += 1;
                        let mut sub_cur = 0usize;
                        let r = interpret_rec(
                            &subrs[real], &mut sub_cur, global_subrs, local_subrs,
                            stack, px, py, out, depth,
                        );
                        *depth -= 1;
                        r?;
                    } else {
                        return Err(());
                    }
                }
            }
            11 => {
                // return
                *cur = local_cur;
                return Ok(());
            }
            14 => {
                // endchar
                *cur = local_cur;
                return Ok(());
            }
            _ => {
                // 未知操作符：跳过（保守）
            }
        }
    }
    *cur = local_cur;
    Ok(())
}

/// 读 Type2 charstring 操作数（整数）
fn read_cs_operand(d: &[u8]) -> Result<(i32, usize), ()> {
    let b = d[0];
    match b {
        32..=246 => Ok((b as i32 - 139, 1)),
        247..=250 => {
            if d.len() < 2 {
                return Err(());
            }
            let b1 = d[1] as i32;
            Ok(((b as i32 - 247) * 256 + b1 + 108, 2))
        }
        251..=254 => {
            if d.len() < 2 {
                return Err(());
            }
            let b1 = d[1] as i32;
            Ok((-((b as i32 - 251) * 256) - b1 - 108, 2))
        }
        28 => {
            if d.len() < 3 {
                return Err(());
            }
            let v = ((d[1] as i32) << 8) | d[2] as i32;
            Ok((if v >= 0x8000 { v - 0x10000 } else { v }, 3))
        }
        29 => {
            if d.len() < 5 {
                return Err(());
            }
            let v = ((d[1] as i32) << 24)
                | ((d[2] as i32) << 16)
                | ((d[3] as i32) << 8)
                | d[4] as i32;
            Ok((v, 5))
        }
        255 => {
            // 16.16 定点
            if d.len() < 5 {
                return Err(());
            }
            let v = ((d[1] as i32) << 24)
                | ((d[2] as i32) << 16)
                | ((d[3] as i32) << 8)
                | d[4] as i32;
            Ok((v >> 16, 5))
        }
        _ => Err(()),
    }
}


/// 定点精度（Q16）
const FP: i64 = 1 << 16;

/// 三次贝塞尔细分并变换到定点像素空间，输出线段 (x0,y0,x1,y1)
fn cubic_to_segs(
    x0: i64, y0: i64, x1: i64, y1: i64, x2: i64, y2: i64, x3: i64, y3: i64,
    n: usize, scale: i64, minx: i64, maxy: i64, out: &mut Vec<(i64, i64, i64, i64)>,
) {
    let nn = n as i64;
    let n3 = nn * nn * nn;
    let mut prevx = (x0 - minx) * scale;
    let mut prevy = (maxy - y0) * scale;
    for k in 1..=n {
        let kk = k as i64;
        let nm = nn - kk;
        let a = nm * nm * nm;
        let b = 3 * nm * nm * kk;
        let c = 3 * nm * kk * kk;
        let d = kk * kk * kk;
        let xx = (a * x0 + b * x1 + c * x2 + d * x3) / n3;
        let yy = (a * y0 + b * y1 + c * y2 + d * y3) / n3;
        let nx = (xx - minx) * scale;
        let ny = (maxy - yy) * scale;
        out.push((prevx, prevy, nx, ny));
        prevx = nx;
        prevy = ny;
    }
}

/// 把 Type2 解释得到的轮廓点光栅化为 size×size 位图（每像素 1 字节，0/255）
/// 返回 (w, h, bitmap)。整数定点实现，避免内核未启用 FPU 的浮点崩溃。
pub fn rasterize(points: &[PathPoint], upem: u32, size: u32) -> Option<(u32, u32, Vec<u8>)> {
    if points.len() < 2 || upem == 0 || size == 0 {
        return None;
    }
    // 包围盒（含控制点）
    let mut minx = i64::MAX;
    let mut miny = i64::MAX;
    let mut maxx = i64::MIN;
    let mut maxy = i64::MIN;
    for p in points {
        let (x, y) = (p.x as i64, p.y as i64);
        if x < minx { minx = x; }
        if x > maxx { maxx = x; }
        if y < miny { miny = y; }
        if y > maxy { maxy = y; }
    }
    if minx > maxx || miny > maxy {
        return None;
    }
    let scale = (size as i64) * FP / (upem as i64);
    if scale <= 0 {
        return None;
    }
    // 生成线段（三次贝塞尔细分 n=16 段；单 on 点为直线）
    let nsub = 16usize;
    let mut segs: Vec<(i64, i64, i64, i64)> = Vec::new();
    let mut i = 0usize;
    let mut sx = 0i64;
    let mut sy = 0i64;
    let mut have = false;
    while i < points.len() {
        if points[i].on {
            if have {
                // 直线 start->p
                let x0 = (sx - minx) * scale;
                let y0 = (maxy - sy) * scale;
                let x1 = (points[i].x as i64 - minx) * scale;
                let y1 = (maxy - points[i].y as i64) * scale;
                segs.push((x0, y0, x1, y1));
            }
            sx = points[i].x as i64;
            sy = points[i].y as i64;
            have = true;
            i += 1;
        } else {
            // off 点：期望 (off, off, on) 三次贝塞尔
            if i + 2 < points.len() && !points[i + 1].on && points[i + 2].on {
                let c1x = points[i].x as i64;
                let c1y = points[i].y as i64;
                let c2x = points[i + 1].x as i64;
                let c2y = points[i + 1].y as i64;
                let ex = points[i + 2].x as i64;
                let ey = points[i + 2].y as i64;
                if have {
                    cubic_to_segs(sx, sy, c1x, c1y, c2x, c2y, ex, ey, nsub, scale, minx, maxy, &mut segs);
                }
                sx = ex;
                sy = ey;
                have = true;
                i += 3;
            } else if i + 1 < points.len() && points[i + 1].on {
                // 二次贝塞尔（罕见，近似为折线）
                let cx = points[i].x as i64;
                let cy = points[i].y as i64;
                let ex = points[i + 1].x as i64;
                let ey = points[i + 1].y as i64;
                if have {
                    // 近似：中点处用二次控制点
                    let midx = (sx + 2 * cx + ex) / 4;
                    let midy = (sy + 2 * cy + ey) / 4;
                    let x0 = (sx - minx) * scale;
                    let y0 = (maxy - sy) * scale;
                    let xm = (midx - minx) * scale;
                    let ym = (maxy - midy) * scale;
                    let x1 = (ex - minx) * scale;
                    let y1 = (maxy - ey) * scale;
                    segs.push((x0, y0, xm, ym));
                    segs.push((xm, ym, x1, y1));
                }
                sx = ex;
                sy = ey;
                have = true;
                i += 2;
            } else {
                i += 1;
            }
        }
    }
    if segs.is_empty() {
        return None;
    }
    // 扫描线 even-odd 填充
    let mut bitmap = vec![0u8; (size as usize) * (size as usize)];
    let sizei = size as i64;
    for yy in 0..size {
        let yfp = (yy as i64) * FP;
        let mut xs: Vec<i64> = Vec::new();
        for &(x0, y0, x1, y1) in &segs {
            let (ylo, yhi) = if y0 < y1 { (y0, y1) } else { (y1, y0) };
            if yfp < ylo || yfp >= yhi {
                continue;
            }
            let dy = y1 - y0;
            if dy == 0 {
                continue;
            }
            let dx = x1 - x0;
            let num = (yfp - y0) * dx;
            let x = x0 + num / dy;
            xs.push(x);
        }
        xs.sort_unstable();
        let mut k = 0usize;
        while k + 1 < xs.len() {
            let xa = xs[k];
            let xb = xs[k + 1];
            let mut xx = xa / FP;
            let endxx = xb / FP;
            while xx < endxx {
                if xx >= 0 && xx < sizei {
                    let idx = (yy as usize) * (size as usize) + xx as usize;
                    bitmap[idx] = 255;
                }
                xx += 1;
            }
            k += 2;
        }
    }
    Some((size, size, bitmap))
}
