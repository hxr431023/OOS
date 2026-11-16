//! UEFI Bootloader
//! 职责：从ESP加载 kernel.elf -> ELF解析按段加载 -> 退出启动服务 -> 跳转内核入口

#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::alloc::Layout;
use core::panic::PanicInfo;
use alloc::vec::Vec;

use uefi::prelude::*;
use uefi::proto::media::block::BlockIO;
use uefi::table::boot::MemoryType;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};

const KERNEL_PATH: &str = "\\system\\boot\\kernel.elf";

/// 与内核 memory::UefiMemoryMap 布局一致的 repr(C) 结构
#[repr(C)]
struct MemInfo {
    ram_start: u64,
    ram_end: u64,
    /// GOP framebuffer（物理地址），无 GOP 时 =0（BIOS/legacy 用 0xB8000）
    fb_base: u64,
    fb_width: u32,
    fb_height: u32,
    /// 每行字节数（stride * bpp）
    fb_stride: u32,
    /// 每像素字节数
    fb_bpp: u32,
    /// 像素格式：0=无，1=Rgb(0R1G2B)，2=Bgr(0B1G2R)
    fb_format: u32,
}

/// D2 修订：UEFI memory map 描述符数组（(base,len,type)，最多 64 项）
static mut MEM_DESCS: [(u64, u64, u32); 64] = [(0, 0, 0); 64];
static mut MEM_DESC_COUNT: usize = 0;

/// 全局 mem_info：用 static（引导器镜像内）而非栈，
/// 避免 exit_boot_services 之后 UEFI 覆盖引导器栈导致 mem_info 损坏。
static mut MEM_INFO: MemInfo = MemInfo {
    ram_start: 0,
    ram_end: 0,
    fb_base: 0,
    fb_width: 0,
    fb_height: 0,
    fb_stride: 0,
    fb_bpp: 0,
    fb_format: 0,
};

/// 手动初始化 COM1 (0x3F8) 串口为 115200 8N1，确保引导阶段串口输出稳定可靠
fn uart_init() {
    unsafe {
        core::arch::asm!(
            "mov dx, 0x3FB", "mov al, 0x80", "out dx, al",
            "mov dx, 0x3F8", "mov al, 0x01", "out dx, al",
            "mov dx, 0x3F9", "mov al, 0x00", "out dx, al",
            "mov dx, 0x3FB", "mov al, 0x03", "out dx, al",
            options(nostack)
        );
    }
}

// GOP 结果暂存全局，供 exit 后（串口可靠段）打印与内核传递
static mut G_FB_BASE: u64 = 0;
static mut G_FB_W: u32 = 0;
static mut G_FB_H: u32 = 0;
static mut G_FB_STRIDE: u32 = 0;
static mut G_FB_BPP: u32 = 0;
static mut G_FB_FMT: u32 = 0;

#[no_mangle]
pub extern "C" fn putc(c: u8) {    unsafe {
        core::arch::asm!(
            // 忙等待串口发送保持寄存器为空（LSR 0x3FD bit5），避免丢字符
            "2: mov dx, 0x3FD",
            "in al, dx",
            "test al, 0x20",
            "jz 2b",
            "mov dx, 0x3F8",
            "mov al, {0}",
            "out dx, al",
            in(reg_byte) c,
            options(nostack)
        );
    }
}
fn sprint(s: &str) {
    for b in s.bytes() {
        putc(b);
    }
}

/// 短延时：确保串口字符有足够时间刷新到 file 重定向（避免被后续快速输出覆盖）
fn spin_delay() {
    for _ in 0..500000 {
        core::hint::spin_loop();
    }
}

const HEX: &[u8; 16] = b"0123456789abcdef";
fn sprint_hex32(v: u32) {
    let mut x = v;
    for _ in 0..8 {
        let nib = (x >> 28) & 0xF;
        putc(HEX[nib as usize]);
        x <<= 4;
    }
}

#[entry]
fn efi_main(_image: Handle, mut st: SystemTable<Boot>) -> Status {
    // 不走 sprint 的内联汇编串口标记，确认 VM 加载的是最新引导器
    unsafe {
        core::arch::asm!(
            "mov dx, 0x3FD", "2: in al, dx", "test al, 0x20", "jz 2b",
            "mov dx, 0x3F8", "mov al, 'Z'", "out dx, al",
            options(nostack)
        );
    }
    sprint("[HYB100]\n");
    sprint("[BOOT]\n");
    sprint("[DBG1]\n");
    uart_init();
    uefi_services::init(&mut st).unwrap();
    sprint("[INIT]\n");

    // 0. 提前获取 GOP framebuffer（boot services 干净、串口可捕获）
    sprint("[PRE=]\n");
    spin_delay();
    let (mut fb_base, mut fb_w, mut fb_h, mut fb_stride, mut fb_bpp, mut fb_fmt) =
        (0u64, 0u32, 0u32, 0u32, 0u32, 0u32);
    let _bs = st.boot_services();
    sprint("[PRE2=]\n");
    spin_delay();
    // 标准 uefi crate 方式获取 GOP（find_handles + open_protocol）
    let handles = _bs.find_handles::<GraphicsOutput>();
    match handles {
        Ok(hs) => {
            if let Some(&handle) = hs.first() {
                let params = uefi::table::boot::OpenProtocolParams {
                    handle,
                    agent: _bs.image_handle(),
                    controller: None,
                };
                match unsafe {
                    _bs.open_protocol::<GraphicsOutput>(
                        params,
                        uefi::table::boot::OpenProtocolAttributes::GetProtocol,
                    )
                } {
                    Ok(mut scoped) => {
                        let gop: &mut GraphicsOutput = &mut scoped;
                        let mode = gop.current_mode_info();
                        let (w, hgt) = mode.resolution();
                        let fmt = mode.pixel_format();
                        let bpp: u32 = 4; // Rgb/Bgr 均为 32bit
                        let stride = (mode.stride() as u32) * bpp;
                        fb_w = w as u32;
                        fb_h = hgt as u32;
                        fb_stride = stride;
                        fb_bpp = bpp;
                        fb_fmt = match fmt {
                            PixelFormat::Bgr => 2,
                            _ => 1,
                        };
                        let mut fb = gop.frame_buffer();
                        fb_base = fb.as_mut_ptr() as u64;
                        sprint("[FB=");
                        sprint_hex32((fb_base >> 32) as u32);
                        sprint_hex32(fb_base as u32);
                        sprint("]\n");
                        // 把 GOP 结果存入全局，供 exit 后（串口可靠段）打印与内核传递
                        unsafe {
                            G_FB_BASE = fb_base;
                            G_FB_W = fb_w;
                            G_FB_H = fb_h;
                            G_FB_STRIDE = fb_stride;
                            G_FB_BPP = fb_bpp;
                            G_FB_FMT = fb_fmt;
                        }
                    }
                    Err(_) => {
                        sprint("[GOPEN_ERR]\n");
                    }
                }
            } else {
                sprint("[NO_GOP_HANDLE]\n");
            }
        }
        Err(_) => {
            sprint("[FINDH_ERR]\n");
        }
    }
    let found = fb_base != 0;
    let _ = found;
    // 1. 从 ESP 加载内核 ELF
    let kernel_data = match load_kernel(&st) {
        Ok(d) => { sprint("[KOK]\n"); d }
        Err(_) => { sprint("[KFAIL]\n"); loop { core::hint::spin_loop(); } }
    };

    // 2. ELF 解析，按 PT_LOAD 段写入物理内存，返回入口
    let entry = load_elf_segments(&kernel_data);
    sprint("[ELF]\n");

    // 2.8 在退出启动服务前，构建 bootloader 自己的 identity 分页（0-4GB RWX，2MB 大页）
    let cr3 = setup_identity_paging();

    // 2.9 退出启动服务前获取真实 UEFI memory map，记录可用物理区间（D2）
    //     exit 后 boot services 内存被释放，所以必须在此之前读完 descriptor。
    let mut ram_start = 0x100000u64;
    let mut ram_end = 0u64;
    let mut mm_buf = alloc::vec![0u8; 64 * 1024];
    unsafe { MEM_DESC_COUNT = 0; }
    if let Ok(mm) = st.boot_services().memory_map(&mut mm_buf) {
        for d in mm.entries() {
            let s = d.phys_start;
            let e = s.saturating_add(d.page_count.saturating_mul(4096));
            // D2 修订：全部描述符交给内核，由内核跳过非 CONVENTIONAL 与 MMIO
            unsafe {
                if MEM_DESC_COUNT < 64 {
                    MEM_DESCS[MEM_DESC_COUNT] = (s, e.saturating_sub(s), d.ty.0 as u32);
                    MEM_DESC_COUNT += 1;
                }
            }
            if d.ty != MemoryType::CONVENTIONAL { continue; }
            if s >= 0x100000 && s < ram_start { ram_start = s; }
            if e > ram_end && e <= 0x1_0000_0000 { ram_end = e; }
        }
    }
    if ram_end <= ram_start {
        ram_end = 0x4000_0000;
    }
    unsafe {
        MEM_INFO.ram_start = ram_start;
        MEM_INFO.ram_end = ram_end;
    }
    sprint("[RMEM]\n");

    // 3. 退出启动服务
    let _ = st.exit_boot_services(MemoryType::LOADER_DATA);
    sprint("[EXIT]\n");

    // 4. 切换到自建 identity 分页，摆脱 UEFI 分页（其把低地址标 NX / 页表状态在 exit 后不确定）
    unsafe {
        core::arch::asm!("mov cr3, {}", in(reg) cr3, options(nomem, nostack, preserves_flags));
    }
    //    清 CR4.NXE，允许从低地址取指执行内核
    unsafe {
        let cr4: u64;
        core::arch::asm!(
            "mov {}, cr4",
            out(reg) cr4,
            options(nomem, nostack, preserves_flags)
        );
        core::arch::asm!(
            "mov cr4, {}",
            in(reg) (cr4 & !(1u64 << 11)),
            options(nomem, nostack, preserves_flags)
        );
    }
    sprint("[NXOFF]\n");
    // GOP 结果（exit 后串口可靠段打印，volatile 读防优化）
    unsafe {
        let b = core::ptr::read_volatile(&G_FB_BASE as *const u64);
        sprint("[GB=");
        sprint_hex32((b >> 32) as u32);
        sprint_hex32(b as u32);
        sprint("]\n");
    }

    // 阻止 LLVM 把寄存器传递/跳转重排到 exit_boot_services 之前
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

    // 4.5 exit 后把 fb 信息写到固定物理地址 0x800000（identity 映射，exit 后保留），
    // 内核入口从此处读取。用 Rust read_volatile/write_volatile——它们生成正确的
    // 64 位 movabs 绝对寻址，不会像内联汇编 sym 那样符号扩展成未映射的高地址。
    unsafe {
        let b = core::ptr::read_volatile(&G_FB_BASE as *const u64);
        core::ptr::write_volatile(0x800000 as *mut u64, b);
        core::ptr::write_volatile(0x800008 as *mut u64, core::ptr::read_volatile(&G_FB_W as *const u32) as u64);
        core::ptr::write_volatile(0x800010 as *mut u64, core::ptr::read_volatile(&G_FB_H as *const u32) as u64);
        core::ptr::write_volatile(0x800018 as *mut u64, core::ptr::read_volatile(&G_FB_STRIDE as *const u32) as u64);
        core::ptr::write_volatile(0x800020 as *mut u64, core::ptr::read_volatile(&G_FB_BPP as *const u32) as u64);
        core::ptr::write_volatile(0x800028 as *mut u64, core::ptr::read_volatile(&G_FB_FMT as *const u32) as u64);
        // D2：真实可用物理区间（0x800030 / 0x800038，exit 后保留）
        core::ptr::write_volatile(0x800030 as *mut u64, core::ptr::read_volatile(&MEM_INFO.ram_start as *const u64));
        core::ptr::write_volatile(0x800038 as *mut u64, core::ptr::read_volatile(&MEM_INFO.ram_end as *const u64));
        // D2 修订：描述符数组（count @0x800040，数组 @0x800048，每项 24 字节）
        core::ptr::write_volatile(0x800040 as *mut u64, MEM_DESC_COUNT as u64);
        let mut i = 0usize;
        while i < MEM_DESC_COUNT {
            let (bb, ll, tt) = MEM_DESCS[i];
            let p = 0x800048 + i * 24;
            core::ptr::write_volatile(p as *mut u64, bb);
            core::ptr::write_volatile((p + 8) as *mut u64, ll);
            core::ptr::write_volatile((p + 16) as *mut u32, tt);
            i += 1;
        }
    }
    unsafe {
        // 回读确认 0x800000 写入生效（保留真实 G_FB_* 值，不覆盖）
        let chk = core::ptr::read_volatile(0x800000 as *const u64);
        sprint("[W8=");
        sprint_hex32((chk >> 32) as u32);
        sprint_hex32(chk as u32);
        sprint("]\n");
    }
    unsafe {
        // 单独内联汇编直接 jmp 内核入口（仅传 entry，无 sym，不触发符号扩展）
        core::arch::asm!(
            "mov rax, {e}",
            "jmp rax",
            e = in(reg) entry,
            options(nostack)
        );
    }
    // 不可达（exit 后直接 jmp 内核入口）
    Status::SUCCESS
}

// ---------- ELF64 解析 ----------
fn load_elf_segments(data: &[u8]) -> u64 {
    let entry = read_u64(data, 0x18);
    let phoff = read_u64(data, 0x20) as usize;
    let phentsize = read_u16(data, 0x36) as usize;
    let phnum = read_u16(data, 0x38) as usize;

    for i in 0..phnum {
        let off = phoff + i * phentsize;
        let p_type = read_u32(data, off + 0);
        if p_type == 1 {
            // PT_LOAD
            let p_offset = read_u64(data, off + 8) as usize;
            let p_vaddr = read_u64(data, off + 16) as usize;
            let p_filesz = read_u64(data, off + 32) as usize;
            let p_memsz = read_u64(data, off + 40) as usize;
            let dst = p_vaddr as *mut u8;
            unsafe {
                core::ptr::copy_nonoverlapping(data.as_ptr().add(p_offset), dst, p_filesz);
                if p_memsz > p_filesz {
                    core::ptr::write_bytes(dst.add(p_filesz), 0, p_memsz - p_filesz);
                }
            }
        }
    }
    entry
}

fn read_u16(d: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([d[o], d[o + 1]])
}
fn read_u32(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}
fn read_u64(d: &[u8], o: usize) -> u64 {
    u64::from_le_bytes([
        d[o], d[o + 1], d[o + 2], d[o + 3], d[o + 4], d[o + 5], d[o + 6], d[o + 7],
    ])
}

// ---------- ISO9660 (BlockIO) 加载内核 ----------
// 固件只把 El Torito boot image 作为唯一 SimpleFileSystem 卷，因此直接用
// BlockIO 块协议读取光盘上的 ISO9660 文件系统，解析出 \EFI\HYBRID\KERNEL.ELF。
use alloc::string::String;

fn read_sector(blk: &mut BlockIO, media_id: u32, lba: u64, buf: &mut [u8; 2048]) -> Result<(), ()> {
    blk.read_blocks(media_id, lba, buf).map_err(|_| ())
}

fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

// 返回目录中的匹配项 (extent_lba, size, is_dir)
fn find_in_dir(blk: &mut BlockIO, media_id: u32, dir_ext: u32, dir_size: u32, wanted: &str) -> Option<(u32, u32, bool)> {
    let nsec = ((dir_size as usize) + 2047) / 2048;
    for s in 0..nsec {
        let mut buf = [0u8; 2048];
        read_sector(blk, media_id, dir_ext as u64 + s as u64, &mut buf).ok()?;
        let mut off = 0usize;
        while off < 2048 {
            let l = buf[off] as usize;
            if l == 0 {
                break;
            }
            let name_len = buf[off + 32] as usize;
            let name = String::from_utf8_lossy(&buf[off + 33..off + 33 + name_len]).into_owned();
            let extent = le32(&buf[off + 2..off + 6]);
            let size = le32(&buf[off + 10..off + 14]);
            let is_dir = buf[off + 25] & 0x02 != 0;
            // 去掉 ISO9660 版本号 ";1"
            let plain = match name.find(';') {
                Some(p) => &name[..p],
                None => &name[..],
            };
            if plain == wanted {
                return Some((extent, size, is_dir));
            }
            off += l;
        }
    }
    None
}

fn load_kernel(st: &SystemTable<Boot>) -> Result<Vec<u8>, ()> {
    let bt = st.boot_services();
    let handles = bt.find_handles::<BlockIO>().map_err(|_| ())?;
    sprint("[B=");
    putc(b'0' + (handles.len() as u8));
    sprint("]\n");
    for handle in handles {
        let mut blk = bt.open_protocol_exclusive::<BlockIO>(handle).map_err(|_| ())?;
        let media_id = {
            let m = blk.media();
            if !m.is_media_present() || m.block_size() != 2048 {
                continue;
            }
            m.media_id()
        };
        // PVD 在逻辑块 16
        let mut pvd = [0u8; 2048];
        read_sector(&mut blk, media_id, 16, &mut pvd)?;
        sprint("[PV");
        putc(b'0' + pvd[0]);
        sprint("]");
        if pvd[0] != 1 {
            continue;
        }
        let root_ext = le32(&pvd[158..162]);
        let root_size = le32(&pvd[166..170]);
        // boot 目录已挪到 system 下：/system/boot/KERNEL.ELF
        let sysdir = match find_in_dir(&mut blk, media_id, root_ext, root_size, "SYSTEM") {
            Some((e, s, true)) => (e, s),
            _ => continue,
        };
        let bootdir = match find_in_dir(&mut blk, media_id, sysdir.0, sysdir.1, "BOOT") {
            Some((e, s, true)) => (e, s),
            _ => continue,
        };
        let kern = match find_in_dir(&mut blk, media_id, bootdir.0, bootdir.1, "KERNEL.ELF") {
            Some((e, s, false)) => (e, s),
            _ => continue,
        };
        // 读内核文件内容
        let fsize = kern.1 as usize;
        let mut data = alloc::vec![0u8; fsize];
        let nsec = (fsize + 2047) / 2048;
        for s in 0..nsec {
            let mut buf = [0u8; 2048];
            read_sector(&mut blk, media_id, kern.0 as u64 + s as u64, &mut buf)?;
            let off = s * 2048;
            let len = core::cmp::min(2048, fsize - off);
            data[off..off + len].copy_from_slice(&buf[..len]);
        }
        return Ok(data);
    }
    Err(())
}

// ---------- 自建 identity 分页 ----------
// 9 个 4K 页：PML4 + 4×PDP + 4×PD，2MB 大页映射 0-4GB 为 RWX。
#[repr(align(4096))]
struct PageTables([u8; 4096 * 9]);
static mut PAGE_TABLES: PageTables = PageTables([0u8; 4096 * 9]);

fn setup_identity_paging() -> u64 {
    unsafe {
        let base = PAGE_TABLES.0.as_mut_ptr() as usize;
        let pml4 = base as *mut u64;
        let pdp = (base + 0x1000) as *mut u64;
        let pd = [
            (base + 0x2000) as *mut u64,
            (base + 0x3000) as *mut u64,
            (base + 0x4000) as *mut u64,
            (base + 0x5000) as *mut u64,
        ];
        let flags = 0x003u64; // P | RW
        let big = 0x083u64; // P | RW | PS (2MB 大页)
        *pml4.add(0) = (pdp as u64) | flags;
        for (i, p) in pd.iter().enumerate() {
            *pdp.add(i) = (*p as u64) | flags;
        }
        for (gi, p) in pd.iter().enumerate() {
            for i in 0..512u64 {
                let phys = (gi as u64 * 0x4000_0000) + (i * 0x200_000);
                *p.add(i as usize) = phys | big;
            }
        }
        pml4 as u64
    }
}

/// UEFI 环境没有 C 运行时，提供 ucs2/uefi 内部依赖的 wcslen stub
#[no_mangle]
pub extern "C" fn wcslen(s: *const u16) -> usize {
    let mut n = 0;
    unsafe {
        while *s.add(n) != 0 {
            n += 1;
        }
    }
    n
}

#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
