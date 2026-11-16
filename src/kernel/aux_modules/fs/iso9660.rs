//! ISO9660 只读文件系统（fs.md §5 后端1：磁盘文件系统）
//! 逻辑块固定 2048 字节。解析 PVD(逻辑块16)，遍历目录记录定位文件。
//! 块读取通过 `BlockRead` trait 抽象，由具体存储驱动（ATA/ATAPI）实现。
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use crate::glue_layer::SysErr;

/// 逻辑扇区大小（ISO9660 光盘）
pub const SECTOR: usize = 2048;

/// 块读取抽象：按 LBA(逻辑块号) 读扇区
pub trait BlockRead {
    fn read_sector(&self, lba: u32, buf: &mut [u8; SECTOR]) -> Result<(), SysErr>;
    /// 一次读取 count 个连续扇区（默认逐扇区，驱动可覆盖为批量以提升性能）
    fn read_blocks(&self, lba: u32, count: u16, buf: &mut [u8]) -> Result<(), SysErr> {
        for i in 0..count {
            let mut tmp = [0u8; SECTOR];
            self.read_sector(lba + i as u32, &mut tmp)?;
            let off = i as usize * SECTOR;
            buf[off..off + SECTOR].copy_from_slice(&tmp);
        }
        Ok(())
    }
}

/// 目录/文件条目
#[derive(Clone)]
pub struct DirEntry {
    pub name: String,
    pub lba: u32,   // 数据 extent 起始 LBA
    pub size: u32,  // 数据长度（字节）
    pub is_dir: bool,
}

pub struct Iso9660<B: BlockRead> {
    blk: B,
    root_lba: u32,
    root_size: u32,
}

impl<B: BlockRead> Iso9660<B> {
    pub fn new(blk: B) -> Result<Self, SysErr> {
        let mut pvd = [0u8; SECTOR];
        blk.read_sector(16, &mut pvd)?;
        // PVD：byte0=类型(1)，byte1-5=标识 "CD001"（43 44 30 30 31），byte6=版本(1)
        if pvd[0] != 0x01
            || pvd[1] != b'C'
            || pvd[2] != b'D'
            || pvd[3] != b'0'
            || pvd[4] != b'0'
            || pvd[5] != b'1'
        {
            return Err(SysErr::EACCES);
        }
        // PVD 根目录记录起始于偏移156：extent 在 158，data length 在 166
        let root_lba = read_both32(&pvd, 158);
        let root_size = read_both32(&pvd, 166);
        Ok(Iso9660 { blk, root_lba, root_size })
    }

    /// 按绝对路径查找（如 /system/media/CascadiaCode.ttf），返回文件/目录条目
    pub fn lookup(&self, path: &str) -> Result<DirEntry, SysErr> {
        let comps: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if comps.is_empty() {
            return Ok(DirEntry {
                name: String::from("/"),
                lba: self.root_lba,
                size: self.root_size,
                is_dir: true,
            });
        }
        let mut cur = DirEntry {
            name: String::new(),
            lba: self.root_lba,
            size: self.root_size,
            is_dir: true,
        };
        for c in comps {
            cur = self.lookup_in_dir(&cur, c)?;
        }
        Ok(cur)
    }

    fn lookup_in_dir(&self, dir: &DirEntry, name: &str) -> Result<DirEntry, SysErr> {
        let ents = self.read_dir(dir)?;
        for e in ents {
            // ISO9660 通常为大写，做不区分大小写匹配
            if e.name.eq_ignore_ascii_case(name) {
                return Ok(e);
            }
        }
        Err(SysErr::ENOENT)
    }

    /// 读取目录全部条目
    pub fn read_dir(&self, dir: &DirEntry) -> Result<Vec<DirEntry>, SysErr> {
        let mut out = Vec::new();
        let n = (dir.size as usize + SECTOR - 1) / SECTOR;
        let mut raw = vec![0u8; n * SECTOR];
        self.blk.read_blocks(dir.lba, n as u16, &mut raw)?;
        let total = dir.size as usize;
        let mut pos = 0usize;
        while pos < total {
            let sec = pos / SECTOR;
            let off = pos % SECTOR;
            if sec >= n {
                break;
            }
            let base = sec * SECTOR;
            let len = raw[base + off] as usize;
            if len == 0 {
                break;
            }
            if off + len > SECTOR {
                break;
            }
            let rec = &raw[base + off..base + off + len];
            let name_len = rec[32] as usize;
            if name_len == 0 || 33 + name_len > rec.len() {
                pos += len;
                continue;
            }
            let flags = rec[25];
            let elba = read_both32(rec, 2);
            let esize = read_both32(rec, 10);
            let name = sanitize_name(&rec[33..33 + name_len]);
            pos += len;
            if name == "." || name == ".." {
                continue;
            }
            out.push(DirEntry {
                name,
                lba: elba,
                size: esize,
                is_dir: flags & 0x02 != 0,
            });
        }
        Ok(out)
    }

    /// 读取文件全部字节
    pub fn read_file(&self, f: &DirEntry) -> Result<Vec<u8>, SysErr> {
        let n = (f.size as usize + SECTOR - 1) / SECTOR;
        let mut data = Vec::with_capacity(f.size as usize);
        let mut i = 0;
        while i < n {
            let batch = core::cmp::min(n - i, 64);
            let mut buf = vec![0u8; batch * SECTOR];
            self.blk.read_blocks(f.lba + i as u32, batch as u16, &mut buf)?;
            let take = if i + batch == n {
                f.size as usize - i * SECTOR
            } else {
                batch * SECTOR
            };
            data.extend_from_slice(&buf[..take]);
            i += batch;
        }
        Ok(data)
    }
}

/// ISO9660 both-endian 32 位字段为 8 字节：前 4 字节 little-endian，后 4 字节 big-endian。
/// 取前 4 字节 little-endian 即可得到数值。
fn read_both32(b: &[u8], off: usize) -> u32 {
    (b[off] as u32)
        | ((b[off + 1] as u32) << 8)
        | ((b[off + 2] as u32) << 16)
        | ((b[off + 3] as u32) << 24)
}

/// 清理 ISO9660 文件名：去尾部空字节、去 ;version 后缀
fn sanitize_name(b: &[u8]) -> String {
    let mut raw = Vec::new();
    for &x in b {
        if x == 0 {
            break;
        }
        raw.push(x);
    }
    let mut s = String::from_utf8_lossy(&raw).into_owned();
    if let Some(idx) = s.find(';') {
        s.truncate(idx);
    }
    s
}
