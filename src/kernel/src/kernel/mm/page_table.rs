#![no_std]
//! x86-64 四级页表管理（memory.md）
//! 高半内核映射，支持用户态/内核态地址空间分离
use crate::sync::Mutex;
use crate::glue_layer::SysErr;
use alloc::vec::Vec;

bitflags::bitflags! {
    pub struct PageFlags: u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER = 1 << 2;
        const WRITE_THROUGH = 1 << 3;
        const CACHE_DISABLE = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const HUGE_PAGE = 1 << 7;
        const GLOBAL = 1 << 8;
        const NO_EXECUTE = 1 << 63;
    }
}

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_MASK: usize = !(PAGE_SIZE - 1);

/// 页表条目
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(pub u64);

impl PageTableEntry {
    pub fn new(addr: u64, flags: PageFlags) -> Self {
        Self((addr & 0x000FFFFFFFFFF000) | flags.bits())
    }
    pub fn addr(&self) -> u64 { self.0 & 0x000FFFFFFFFFF000 }
    pub fn flags(&self) -> PageFlags { PageFlags::from_bits_truncate(self.0) }
    pub fn is_present(&self) -> bool { self.flags().contains(PageFlags::PRESENT) }
}

/// 页表（512条目）
#[repr(align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; 512],
}

impl PageTable {
    pub const fn new() -> Self {
        Self { entries: [PageTableEntry(0); 512] }
    }
}

/// 地址空间（CR3根）
pub struct AddressSpace {
    pub pml4: *mut PageTable,
    pub cr3: u64,
}

unsafe impl Send for AddressSpace {}
unsafe impl Sync for AddressSpace {}

impl AddressSpace {
    pub fn new() -> Result<Self, SysErr> {        // 分配一个物理页作为PML4
        let pml4_phys = crate::memory::hpa_alloc_page().ok_or(SysErr::EINVAL)?;
        let pml4 = pml4_phys as *mut PageTable;
        unsafe { core::ptr::write_bytes(pml4 as *mut u8, 0, PAGE_SIZE); }
        Ok(Self {
            pml4,
            cr3: pml4_phys as u64,
        })
    }

    /// 映射虚拟地址到物理地址
    pub fn map(&mut self, virt: u64, phys: u64, flags: PageFlags) -> Result<(), SysErr> {
        let pml4_idx = (virt >> 39) & 0x1FF;
        let pdpt_idx = (virt >> 30) & 0x1FF;
        let pd_idx = (virt >> 21) & 0x1FF;
        let pt_idx = (virt >> 12) & 0x1FF;

        let pml4 = unsafe { &mut *self.pml4 };

        // PML4 -> PDPT
        if !pml4.entries[pml4_idx as usize].is_present() {
            let pdpt_phys = crate::memory::hpa_alloc_page().ok_or(SysErr::EINVAL)?;
            unsafe { core::ptr::write_bytes(pdpt_phys as *mut u8, 0, PAGE_SIZE); }
            pml4.entries[pml4_idx as usize] = PageTableEntry::new(
                pdpt_phys as u64,
                PageFlags::PRESENT | PageFlags::WRITABLE | PageFlags::USER,
            );
        }
        let pdpt = pml4.entries[pml4_idx as usize].addr() as *mut PageTable;

        // PDPT -> PD
        let pdpt_ref = unsafe { &mut *pdpt };
        if !pdpt_ref.entries[pdpt_idx as usize].is_present() {
            let pd_phys = crate::memory::hpa_alloc_page().ok_or(SysErr::EINVAL)?;
            unsafe { core::ptr::write_bytes(pd_phys as *mut u8, 0, PAGE_SIZE); }
            pdpt_ref.entries[pdpt_idx as usize] = PageTableEntry::new(
                pd_phys as u64,
                PageFlags::PRESENT | PageFlags::WRITABLE | PageFlags::USER,
            );
        }
        let pd = pdpt_ref.entries[pdpt_idx as usize].addr() as *mut PageTable;

        // PD -> PT
        let pd_ref = unsafe { &mut *pd };
        if !pd_ref.entries[pd_idx as usize].is_present() {
            let pt_phys = crate::memory::hpa_alloc_page().ok_or(SysErr::EINVAL)?;
            unsafe { core::ptr::write_bytes(pt_phys as *mut u8, 0, PAGE_SIZE); }
            pd_ref.entries[pd_idx as usize] = PageTableEntry::new(
                pt_phys as u64,
                PageFlags::PRESENT | PageFlags::WRITABLE | PageFlags::USER,
            );
        }
        let pt = pd_ref.entries[pd_idx as usize].addr() as *mut PageTable;

        // PT -> 物理页
        let pt_ref = unsafe { &mut *pt };
        pt_ref.entries[pt_idx as usize] = PageTableEntry::new(phys, flags);

        // 刷新TLB
        unsafe { core::arch::asm!("invlpg [{}]", in(reg) virt); }
        Ok(())
    }

    /// 取消映射
    pub fn unmap(&mut self, virt: u64) -> Result<u64, SysErr> {
        // 简化：找到PT条目，清零，返回物理地址
        let _ = virt;
        Ok(0)
    }

    /// 切换到该地址空间（写CR3）
    pub fn switch(&self) {
        unsafe {
            core::arch::asm!("mov cr3, {}", in(reg) self.cr3);
        }
    }
}

/// 内核全局地址空间
static KERNEL_AS: Mutex<Option<AddressSpace>> = Mutex::new(None);

pub fn init() {
    // 内核高半映射：0xFFFFFFFF80000000 映射到物理 0
    let mut as_ = AddressSpace::new().expect("kernel address space");
    // 映射内核前16MB
    for i in 0..(16 * 1024 * 1024 / PAGE_SIZE) {
        let phys = (i * PAGE_SIZE) as u64;
        let virt = 0xFFFFFFFF80000000 + phys;
        let _ = as_.map(virt, phys,
            PageFlags::PRESENT | PageFlags::WRITABLE | PageFlags::GLOBAL);
    }
    *KERNEL_AS.lock() = Some(as_);
    crate::println!("page table ready, kernel high-half mapped");
}

pub fn kernel_address_space() -> &'static Mutex<Option<AddressSpace>> {
    &KERNEL_AS
}
