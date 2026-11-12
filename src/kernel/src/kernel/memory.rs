#![no_std]
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
const HEAP_SIZE: usize = 64 * 1024 * 1024;
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
        let size = layout.size().max(layout.align());
        let mut prev: *mut FreeBlock = ptr::null_mut();
        let mut cur = FREE_HEAD;
        while !cur.is_null() {
            let block = &mut *cur;
            if block.size >= size {
                // 分裂
                if block.size > size + core::mem::size_of::<FreeBlock>() {
                    let remain = cur.add(size);
                    (*remain).size = block.size - size;
                    (*remain).next = block.next;
                    if prev.is_null() {
                        FREE_HEAD = remain;
                    } else {
                        (*prev).next = remain;
                    }
                } else {
                    if prev.is_null() {
                        FREE_HEAD = block.next;
                    } else {
                        (*prev).next = block.next;
                    }
                }
                return cur as *mut u8;
            }
            prev = cur;
            cur = block.next;
        }
        ptr::null_mut()
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let block = ptr as *mut FreeBlock;
        (*block).size = layout.size().max(layout.align());
        (*block).next = FREE_HEAD;
        FREE_HEAD = block;
    }
}

// ================= 物理页分配器（HPA）=================
static HPA_NEXT: AtomicUsize = AtomicUsize::new(0);
static mut PHYS_START: usize = 0;
static mut PHYS_END: usize = 0;

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

/// 返回 (总物理内存, 已分配) 字节数，供 /system/proc/meminfo 虚拟文件
pub fn mem_stats() -> (u64, u64) {
    unsafe {
        let total = (PHYS_END - PHYS_START) as u64;
        let allocated = HPA_NEXT.load(Ordering::SeqCst) as u64;
        (total, allocated)
    }
}

pub fn init(map: &UefiMemoryMap) {
    unsafe {
        PHYS_START = (map.ram_start & !0xFFF) as usize;
        PHYS_END = map.ram_end as usize;
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
