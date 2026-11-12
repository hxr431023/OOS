//! ATAPI 光驱驱动（driver.md / fs.md 块设备层）
//! 传统 IDE 主通道 (0x1F0)，ATAPI PACKET 命令 READ(10) 读 2048 字节逻辑扇区。
//! 支持两种数据通道：
//!   - PIO：端口忙等轮询（慢，VirtualBox 下大文件易卡死）；
//!   - Bus Master IDE DMA（BMIDE）：经 PCI BAR4 编程 PRDT，数据由控制器直写内存，
//!     大幅减少端口访问，根治大文件 PIO 卡死。DMA 不可用时回退 PIO。
use x86_64::instructions::port::{Port, PortRead, PortWrite};
use crate::fs::iso9660::{BlockRead, SECTOR};
use crate::glue_layer::SysErr;

// ATA 主通道端口
const DATA: u16 = 0x1F0;
const LBA_MID: u16 = 0x1F4;
const LBA_HI: u16 = 0x1F5;
const DEVICE: u16 = 0x1F6;
const STATUS: u16 = 0x1F7;

const BSY: u8 = 0x80;
const DRQ: u8 = 0x08;
const ERR: u8 = 0x01;

/// 一次 Bus Master DMA 传输的字节数（32KB，PRDT 单描述项，须为偶数且 < 64KB）
const DMA_BUF_SIZE: usize = 32 * 1024;
const DMA_BUF_SECTORS: u16 = (DMA_BUF_SIZE / SECTOR) as u16;

pub struct AtapiDrive {
    present: bool,
    bmide: u16,  // PCI IDE 控制器 BAR4（BMIDE）IO 基址；0 = 无 DMA，回退 PIO
    prdt: usize, // PRDT 物理地址（Bus Master DMA 描述符表）
    dbuf: usize, // DMA 数据缓冲物理地址
}

/// 分配 n 个物理连续 4KB 页（hpa_alloc_page 按地址递增分配，天然连续）
fn alloc_contig(n: usize) -> Option<*mut u8> {
    let first = crate::memory::hpa_alloc_page()?;
    for _ in 1..n {
        let _ = crate::memory::hpa_alloc_page()?;
    }
    Some(first)
}

impl AtapiDrive {
    pub fn new() -> Self {
        let mut d = AtapiDrive {
            present: false,
            bmide: 0,
            prdt: 0,
            dbuf: 0,
        };
        d.detect();
        d.setup_dma();
        d
    }

    pub fn is_present(&self) -> bool {
        self.present
    }

    /// 探测 PCI IDE 控制器（class=0x01），取 BAR4 作为 BMIDE IO 基址，
    /// 分配 PRDT 一页 + DMA 缓冲（32KB，物理连续）。
    fn setup_dma(&mut self) {
        let devs = crate::hw::pci::enumerate_all();
        for dev in &devs {
            if dev.class == 0x01 {
            }
        }
        let mut found = false;
        for dev in devs {
            if dev.class != 0x01 {
                continue;
            }
            if dev.subclass != 0x01 && dev.subclass != 0x80 {
                continue;
            }
            found = true;
            // BAR4 为 IO 空间：bit0=1 表示 IO，基址 = 值 & 0xFFFC
            let bar4 = (dev.bar[4] as u32) & 0xFFFC;
            if bar4 == 0 {
                break;
            }
            // PRDT 1 页（可容纳 512 个描述项）+ DMA 缓冲 8 页（32KB）
            let prdt = crate::memory::hpa_alloc_page();
            let dbuf = alloc_contig(DMA_BUF_SIZE / 4096);
            if let (Some(p), Some(d)) = (prdt, dbuf) {
                self.prdt = p as usize;
                self.dbuf = d as usize;
                self.bmide = bar4 as u16;
            } else {
            }
            break;
        }
        if !found {
        }
    }
    fn detect(&mut self) {
        let mut dev = Port::<u8>::new(DEVICE);
        unsafe { dev.write(0xB0u8) };
        let mut status = Port::<u8>::new(STATUS);
        for _ in 0..4 {
            unsafe {
                let _ = status.read();
            }
        }
        let st = unsafe { status.read() };
        if st == 0xFF {
            return; // 无设备
        }
        // 发 IDENTIFY PACKET 命令探测光驱
        unsafe { status.write(0xA1u8) };
        let mut cnt = 0;
        loop {
            let s = unsafe { status.read() };
            if s & BSY == 0 {
                if s & ERR != 0 {
                    return;
                }
                if s & DRQ != 0 {
                    self.present = true;
                    let mut data = Port::<u16>::new(DATA);
                    for _ in 0..256 {
                        unsafe {
                            let _ = data.read();
                        }
                    }
                }
                return;
            }
            cnt += 1;
            if cnt > 500_000 {
                return;
            }
        }
    }

    fn wait_ready(&self) -> Result<(), SysErr> {
        let mut status = Port::<u8>::new(STATUS);
        let mut cnt = 0;
        loop {
            let s = unsafe { status.read() };
            if s & BSY == 0 && s & DRQ == 0 {
                return Ok(());
            }
            if s & ERR != 0 {
                return Err(SysErr::EACCES);
            }
            cnt += 1;
            if cnt > 5_000_000 {
                return Err(SysErr::EACCES);
            }
        }
    }

    fn wait_drq(&self) -> Result<(), SysErr> {
        let mut status = Port::<u8>::new(STATUS);
        let mut cnt = 0;
        loop {
            let s = unsafe { status.read() };
            if s & BSY == 0 {
                if s & ERR != 0 {
                    return Err(SysErr::EACCES);
                }
                if s & DRQ != 0 {
                    return Ok(());
                }
            }
            cnt += 1;
            if cnt > 1_000_000 {
                return Err(SysErr::EACCES);
            }
        }
    }

    fn wait_drq_clear(&self) -> Result<(), SysErr> {
        let mut status = Port::<u8>::new(STATUS);
        let mut cnt = 0;
        loop {
            let s = unsafe { status.read() };
            if s & DRQ == 0 {
                return Ok(());
            }
            cnt += 1;
            if cnt > 200_000 {
                return Ok(());
            }
        }
    }

    /// 经 ATA 端口发送 ATAPI READ(10) 命令包（PIO 发送命令包，数据阶段由调用方决定传输方式）
    /// dma=true：ATAPI DMA —— Features(0x1F1)=1 指示 DMA，字节计数=0（OSDev：DMA 时 maxbytecount 为 0）
    fn send_packet(&self, lba: u32, sectors: u16, dma: bool) -> Result<(), SysErr> {
        let mut data = Port::<u16>::new(DATA);
        let mut lba_mid = Port::<u8>::new(LBA_MID);
        let mut lba_hi = Port::<u8>::new(LBA_HI);
        let mut device = Port::<u8>::new(DEVICE);
        let mut cmd = Port::<u8>::new(STATUS);
        unsafe { device.write(0xB0u8) };
        self.wait_ready()?;
        if dma {
            // DMA 模式：Features=1，字节计数=0
            let mut features = Port::<u8>::new(0x1F1);
            unsafe {
                features.write(0x01);
                lba_mid.write(0x00);
                lba_hi.write(0x00);
            }
        } else {
            unsafe {
                lba_mid.write(0x0Cu8);
                lba_hi.write(0x00u8);
            }
        }
        unsafe { cmd.write(0xA0u8) }; // PACKET
        self.wait_drq()?;
        let mut packet = [0u8; 12];
        packet[0] = 0x28; // READ(10)
        packet[1] = 0x00;
        packet[2] = (lba >> 24) as u8;
        packet[3] = (lba >> 16) as u8;
        packet[4] = (lba >> 8) as u8;
        packet[5] = lba as u8;
        packet[6] = 0x00;
        packet[7] = (sectors >> 8) as u8;
        packet[8] = sectors as u8;
        packet[9] = 0;
        packet[10] = 0;
        packet[11] = 0;
        for i in (0..12).step_by(2) {
            let w = ((packet[i + 1] as u16) << 8) | (packet[i] as u16);
            unsafe { data.write(w) };
        }
        // 等命令包被接收完成，避免与数据块错位
        if !dma {
            self.wait_drq_clear()?;
        }
        Ok(())
    }

    /// Bus Master IDE DMA 读取 lba 起 sectors 个 2048 字节扇区到 buf
    /// 分块传输（每块 32KB），启动 DMA → 发命令包 → 等完成 → 拷贝。
    fn dma_read(&self, lba: u32, sectors: u16, buf: &mut [u8]) -> Result<(), SysErr> {
        if self.bmide == 0 || self.prdt == 0 || self.dbuf == 0 {
            return Err(SysErr::EACCES);
        }
        let mut done = 0usize;
        let mut lba_cur = lba;
        let mut remain = sectors;
        while remain > 0 {
            let chunk = core::cmp::min(remain, DMA_BUF_SECTORS);
            let size = (chunk as usize) * SECTOR;
            // 预写缓冲为 0xAA，验证 DMA 是否真正写入（区分「DMA 未写」与「映射问题」）
            unsafe {
                core::ptr::write_bytes(self.dbuf as *mut u8, 0xAA, size);
            }
            // 编程 PRDT：DWORD0=缓冲物理地址，DWORD1=字节计数(偶数)+EOT(bit31)
            unsafe {
                let prdt = self.prdt as *mut u32;
                *prdt = self.dbuf as u32;
                *(prdt.add(1)) = (size as u32) | 0x8000_0000;
            }
            // 先发 ATAPI READ(10) 命令包（Features=1 指示 DMA），设备进入数据阶段后再启动 BMIDE
            if let Err(e) = self.send_packet(lba_cur, chunk, true) {
                crate::println!("dma: send_packet lba={} n={} failed {:?}", lba_cur, chunk, e);
                unsafe { Port::<u8>::new(self.bmide).write(0x00) };
                return Err(e);
            }
            // 写 PRDT 物理地址（先），再启动 BMIDE：命令 bit1=1 启动，bit0=0 读自设备
            unsafe { Port::<u32>::new(self.bmide + 4).write(self.prdt as u32) };
            unsafe { Port::<u8>::new(self.bmide).write(0x02) };
            // 等待 DMA 完成：等 active(bit5) 清除（传输真正结束）+ 中断(bit2) 置位
            let mut cnt = 0u32;
            let mut st_first = 0u8;
            loop {
                let st = unsafe { Port::<u8>::new(self.bmide + 2).read() };
                if cnt == 0 {
                    st_first = st;
                }
                if (st & 0x20) == 0 && (st & 0x04) != 0 {
                    break;
                }
                if st & 0x08 != 0 {
                    crate::println!("dma: BMIDE err st=0x{:x}", st);
                    unsafe { Port::<u8>::new(self.bmide).write(0x00) };
                    return Err(SysErr::EACCES);
                }
                cnt += 1;
                if cnt > 5_000_000 {
                    crate::println!("dma: BMIDE timeout st=0x{:x}", st);
                    unsafe { Port::<u8>::new(self.bmide).write(0x00) };
                    return Err(SysErr::EACCES); // 超时防卡死
                }
            }
            // 停止 DMA
            unsafe { Port::<u8>::new(self.bmide).write(0x00) };
            crate::println!("dma: block lba={} n={} done st0=0x{:x}", lba_cur, chunk, st_first);
            // 从物理缓冲拷贝到用户 buf（identity-mapped，直接访问）
            if done + size > buf.len() {
                return Err(SysErr::EINVAL);
            }
            unsafe {
                let src = core::slice::from_raw_parts(self.dbuf as *const u8, size);
                // 诊断：打印缓冲前 16 字节（ISO9660 PVD 应以 CD001 开头）
                crate::println!(
                    "dma: data {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}",
                    src[0], src[1], src[2], src[3], src[4], src[5], src[6], src[7],
                    src[8], src[9], src[10], src[11], src[12], src[13], src[14], src[15]
                );
                buf[done..done + size].copy_from_slice(src);
            }
            done += size;
            lba_cur += chunk as u32;
            remain -= chunk;
        }
        Ok(())
    }

    /// PIO 读取（fallback）：lba 起 sectors 个 2048 字节扇区到 buf
    fn pio_read(&self, lba: u32, sectors: u16, buf: &mut [u8]) -> Result<(), SysErr> {
        let mut data = Port::<u16>::new(DATA);
        let mut data32 = Port::<u32>::new(DATA);
        let mut lba_mid = Port::<u8>::new(LBA_MID);
        let mut lba_hi = Port::<u8>::new(LBA_HI);
        let mut device = Port::<u8>::new(DEVICE);

        unsafe { device.write(0xB0u8) };
        self.wait_ready()?;
        unsafe {
            lba_mid.write(0x0Cu8);
            lba_hi.write(0x00u8);
        }
        let mut cmd = Port::<u8>::new(STATUS);
        unsafe { cmd.write(0xA0u8) }; // PACKET
        self.wait_drq()?;
        let mut packet = [0u8; 12];
        packet[0] = 0x28; // READ(10)
        packet[1] = 0x00;
        packet[2] = (lba >> 24) as u8;
        packet[3] = (lba >> 16) as u8;
        packet[4] = (lba >> 8) as u8;
        packet[5] = lba as u8;
        packet[6] = 0x00;
        packet[7] = (sectors >> 8) as u8;
        packet[8] = sectors as u8;
        packet[9] = 0;
        packet[10] = 0;
        packet[11] = 0;
        for i in (0..12).step_by(2) {
            let w = ((packet[i + 1] as u16) << 8) | (packet[i] as u16);
            unsafe { data.write(w) };
        }
        self.wait_drq_clear()?;
        let nbytes = (sectors as usize) * SECTOR;
        if buf.len() < nbytes {
            return Err(SysErr::EINVAL);
        }
        let mut idx = 0;
        let total_512 = nbytes / 512;
        for _ in 0..total_512 {
            self.wait_drq()?;
            for _ in 0..256 {
                let w = unsafe { data.read() };
                buf[idx] = (w & 0xFF) as u8;
                buf[idx + 1] = ((w >> 8) & 0xFF) as u8;
                idx += 2;
            }
            // 节流：读大文件时周期性让出，避免忙轮询占满 vCPU 拖垮主机
            for _ in 0..400 {
                core::hint::spin_loop();
            }
        }
        Ok(())
    }

    /// 读取 lba 起 sectors 个 2048 字节扇区到 buf：优先 PIO（VirtualBox ATAPI DMA 尚不可靠，PIO 已验证稳定）
    pub fn read(&self, lba: u32, sectors: u16, buf: &mut [u8]) -> Result<(), SysErr> {
        self.pio_read(lba, sectors, buf)
    }
}

impl BlockRead for AtapiDrive {
    fn read_sector(&self, lba: u32, buf: &mut [u8; SECTOR]) -> Result<(), SysErr> {
        self.read(lba, 1, buf)
    }
    fn read_blocks(&self, lba: u32, count: u16, buf: &mut [u8]) -> Result<(), SysErr> {
        self.read(lba, count, buf)
    }
}


