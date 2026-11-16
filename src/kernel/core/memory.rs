use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
#[repr(C)]
pub struct UefiMemoryMap {
    pub ram_start: u64,
    pub ram_end: u64,
    /// GOP framebuffer（物理地址），无 GOP 时 =0（BIOS/legacy 用 0xB8000）
    pub fb_base: u64,
    pub fb_width: u32,
    pub fb_height: u32,
    /// 每行字节数
    pub fb_stride: u32,
    /// 每像素字节数
    pub fb_bpp: u32,
    /// 像素格式：0=无，1=Rgb(0R1G2B)，2=Bgr(0B1G2R)
    pub fb_format: u32,
}

/// 内核保存的 GOP framebuffer 信息（供 vga/fb 显示层使用）
#[derive(Debug, Clone, Copy)]
pub struct FrameBufferInfo {
    pub base: u64,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub bpp: u32,
    pub format: u32,
}
static mut FB_INFO: FrameBufferInfo = FrameBufferInfo {
    base: 0,
    width: 0,
    height: 0,
    stride: 0,
    bpp: 0,
    format: 0,
};
/// 读取当前 framebuffer 信息（base=0 表示无 GOP，回退 0xB8000）
pub fn framebuffer() -> FrameBufferInfo {
    unsafe { FB_INFO }
}

// ================= 内核堆：intrusive free-list 分配器 =================
const HEAP_SIZE: usize = 4 * 1024 * 1024;
static mut HEAP_REGION: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
static mut FREE_HEAD: *mut FreeBlock = ptr::null_mut();
static HEAP_INIT: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
struct FreeBlock {
    size: usize,
    next: *mut FreeBlock,
}

pub struct KernelHeap;
impl KernelHeap {
    pub const fn new() -> Self {
        Self
    }
}

unsafe impl GlobalAlloc for KernelHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // D11：按 layout.align() 对齐起始地址；对齐产生的 padding 从块首剔除
        let align = layout.align().max(core::mem::align_of::<FreeBlock>());
        let size = layout.size().max(align).max(core::mem::size_of::<FreeBlock>());
        let mut prev: *mut FreeBlock = ptr::null_mut();
        let mut cur = FREE_HEAD;
        while !cur.is_null() {
            let block = &mut *cur;
            let base = cur as usize;
            let start = (base + align - 1) & !(align - 1);
            let pad = start - base;
            if block.size >= pad + size {
                let remain = (start + size) as *mut FreeBlock;
                let remain_size = block.size - pad - size;
                let has_remain = remain_size >= core::mem::size_of::<FreeBlock>();
                let new_next = if has_remain { remain as *mut FreeBlock } else { block.next };
                if prev.is_null() { FREE_HEAD = new_next; } else { (*prev).next = new_next; }
                if has_remain {
                    (*remain).size = remain_size;
                    (*remain).next = block.next;
                }
                return start as *mut u8;
            }
            prev = cur;
            cur = block.next;
        }
        ptr::null_mut()
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // D11：归属校验 + 有序插入 + 相邻块合并
        let heap_base = core::ptr::addr_of!(HEAP_REGION) as usize;
        let p = ptr as usize;
        if p < heap_base || p >= heap_base + HEAP_SIZE {
            panic!("dealloc of foreign pointer");
        }
        let block = ptr as *mut FreeBlock;
        (*block).size = layout.size().max(layout.align()).max(core::mem::size_of::<FreeBlock>());
        let mut prev: *mut FreeBlock = ptr::null_mut();
        let mut cur = FREE_HEAD;
        while !cur.is_null() && (cur as usize) < p {
            prev = cur;
            cur = (*cur).next;
        }
        (*block).next = cur;
        if prev.is_null() { FREE_HEAD = block; } else { (*prev).next = block; }
        // 合并后继
        if !cur.is_null() && (block as usize + (*block).size) == cur as usize {
            (*block).size += (*cur).size;
            (*block).next = (*cur).next;
        }
        // 合并前驱
        if !prev.is_null() && (prev as usize + (*prev).size) == block as usize {
            (*prev).size += (*block).size;
            (*prev).next = (*block).next;
        }
    }
}

/// D11：堆统计 (已用字节, 空闲块数)
pub fn heap_stats() -> (usize, usize, usize) {
    unsafe {
        let mut free = 0usize;
        let mut blocks = 0usize;
        let mut cur = FREE_HEAD;
        while !cur.is_null() {
            free += (*cur).size;
            blocks += 1;
            cur = (*cur).next;
        }
        (HEAP_SIZE - free, 0, blocks)
    }
}

// ================= 物理页分配器（HPA）=================
static HPA_NEXT: AtomicUsize = AtomicUsize::new(0);
static mut PHYS_START: usize = 0;
static mut PHYS_END: usize = 0;

// 链接脚本导出的内核镜像物理结束地址（linker.ld: PROVIDE(__kernel_end = .)）
unsafe extern "C" {
    static __kernel_end: u8;
}

/// 内核镜像物理结束地址（未对齐）
pub fn kernel_end_addr() -> usize {
    core::ptr::addr_of!(__kernel_end) as usize
}

/// D2 修订：内核维护的 free ranges（(start,end)）
pub const MAX_FREE_RANGES: usize = 32;
static mut FREE_RANGES: [(usize, usize); MAX_FREE_RANGES] = [(0, 0); MAX_FREE_RANGES];
static mut FREE_RANGE_COUNT: usize = 0;

pub fn free_ranges() -> &'static [(usize, usize)] {
    unsafe { &FREE_RANGES[..FREE_RANGE_COUNT] }
}

/// 当前可分配物理内存区间 [start, end)
pub fn phys_range() -> (usize, usize) {
    unsafe { (PHYS_START, PHYS_END) }
}

/// 分配一个 4KB 物理页
pub fn hpa_alloc_page() -> Option<*mut u8> {
    unsafe {
        let off = HPA_NEXT.fetch_add(4096, Ordering::SeqCst);
        let base = PHYS_START + off;
        if base + 4096 > PHYS_END {
            None
        } else {
            Some(base as *mut u8)
        }
    }
}

/// 内存统计（具名字段，避免调用方误读次序）
pub struct MemStats {
    pub total: u64,
    pub allocated: u64,
    pub free: u64,
}

/// 供 /system/proc/meminfo 与 shell meminfo 使用
pub fn mem_stats() -> MemStats {
    unsafe {
        let total = (PHYS_END - PHYS_START) as u64;
        let allocated = HPA_NEXT.load(Ordering::SeqCst) as u64;
        MemStats { total, allocated, free: total.saturating_sub(allocated) }
    }
}

pub fn init(map: &UefiMemoryMap, descs: &[(u64, u64, u32)]) {
    unsafe {
        // D1：把内核镜像自身物理区间从可分配内存中排除，避免页分配器落进内核堆/栈
        let kend = kernel_end_addr();
        let reserve_end = (kend + 0xFFF) & !0xFFF;
        // D2 修订：按描述符数组建 free ranges；跳过非 CONVENTIONAL 与 MMIO
        FREE_RANGE_COUNT = 0;
        for &(b, l, t) in descs {
            // 7 = CONVENTIONAL；MMIO/reserved 等一律跳过
            if t != 7 { continue; }
            if l == 0 { continue; }
            if b < 0x100000 { continue; }
            let mut st = ((b as usize) + 0xFFF) & !0xFFF;
            let en = ((b.saturating_add(l)) as usize) & !0xFFF;
            if en <= st { continue; }
            if st < reserve_end { st = reserve_end; }
            // 0x800000 页为 bootloader 通信页，列为保留
            if st <= 0x800000 && en > 0x800000 {
                st = 0x801000;
            }
            if st >= en { continue; }
            if FREE_RANGE_COUNT < MAX_FREE_RANGES {
                FREE_RANGES[FREE_RANGE_COUNT] = (st, en);
                FREE_RANGE_COUNT += 1;
            }
        }
        if FREE_RANGE_COUNT > 0 {
            PHYS_START = FREE_RANGES[0].0;
            PHYS_END = FREE_RANGES[FREE_RANGE_COUNT - 1].1;
        } else {
            PHYS_START = 0;
            PHYS_END = 0;
        }
        // 保存 GOP framebuffer 信息（base=0 表示无 GOP，显示层回退 0xB8000）
        FB_INFO = FrameBufferInfo {
            base: map.fb_base,
            width: map.fb_width,
            height: map.fb_height,
            stride: map.fb_stride,
            bpp: map.fb_bpp,
            format: map.fb_format,
        };
        // 初始化内核堆
        if HEAP_INIT.swap(1, Ordering::SeqCst) == 0 {
            let heap_ptr = core::ptr::addr_of_mut!(HEAP_REGION) as *mut FreeBlock;
            (*heap_ptr).size = HEAP_SIZE;
            (*heap_ptr).next = ptr::null_mut();
            FREE_HEAD = heap_ptr;
        }
    }
}
