#![no_std]
//! 图形子系统（ui.md）
//! 显卡硬件模块在内核Ring0；窗口管理器、合成器运行用户态
//! 窗口是纯控制内核对象；应用提交缓冲区句柄由合成器统一合成
use crate::sync::Mutex;
use crate::glue_layer::SysErr;
use crate::ipc::WaitQueue;
use hashbrown::HashMap;
use alloc::string::String;
use alloc::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WindowState {
    Hidden,
    Normal,
    Minimized,
    Maximized,
    Fullscreen,
}

/// 图形缓冲区（应用渲染后提交给合成器）
#[derive(Debug, Clone)]
pub struct GfxBuffer {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    /// 物理地址（应用通过共享内存映射）
    pub phys_addr: u64,
    pub size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PixelFormat {
    Rgb888,
    Rgba8888,
    Bgr888,
}

/// 窗口对象（ui.md：纯控制内核对象）
#[derive(Debug)]
pub struct Window {
    pub id: u64,
    pub title: Mutex<String>,
    pub rect: Mutex<Rect>,
    pub state: Mutex<WindowState>,
    /// 当前提交的缓冲区句柄
    pub current_buffer: Mutex<Option<u64>>,
    /// 输入事件IPC队列（键盘/鼠标）
    pub input_queue: WaitQueue,
    /// 所属进程PID
    pub owner_pid: Mutex<u64>,
    /// z-order层级
    pub z_order: Mutex<u32>,
}

impl Window {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            title: Mutex::new(String::new()),
            rect: Mutex::new(Rect { x: 0, y: 0, width: 800, height: 600 }),
            state: Mutex::new(WindowState::Normal),
            current_buffer: Mutex::new(None),
            input_queue: WaitQueue::new(),
            owner_pid: Mutex::new(0),
            z_order: Mutex::new(0),
        }
    }

    pub fn object_path(&self) -> String {
        alloc::format!("/object/ui/window/{}", self.id)
    }

    /// 应用提交渲染缓冲区（ui.md：不能直接写帧缓冲，提交句柄给合成器）
    pub fn submit_buffer(&self, buffer_id: u64) -> Result<(), SysErr> {
        *self.current_buffer.lock() = Some(buffer_id);
        // 通知合成器有新帧需要合成
        // self.compositor_wake();
        Ok(())
    }

    /// 合成器读取当前缓冲区（合成输出到屏幕）
    pub fn get_current_buffer(&self) -> Option<u64> {
        *self.current_buffer.lock()
    }
}

/// 图形管理器
pub struct GfxManager {
    pub windows: Mutex<HashMap<u64, Arc<Window>>>,
    pub buffers: Mutex<HashMap<u64, GfxBuffer>>,
    pub next_window_id: Mutex<u64>,
    pub next_buffer_id: Mutex<u64>,
    /// 帧缓冲物理地址（显卡硬件模块设置）
    pub framebuffer: Mutex<Option<u64>>,
    pub screen_width: u32,
    pub screen_height: u32,
}

impl GfxManager {
    pub fn new() -> Self {
        Self {
            windows: Mutex::new(HashMap::new()),
            buffers: Mutex::new(HashMap::new()),
            next_window_id: Mutex::new(0),
            next_buffer_id: Mutex::new(0),
            framebuffer: Mutex::new(None),
            screen_width: 1920,
            screen_height: 1080,
        }
    }

    /// 创建窗口（返回纯控制内核对象）
    pub fn create_window(&self, title: &str) -> Arc<Window> {
        let mut id = self.next_window_id.lock();
        *id += 1;
        let win_id = *id;
        let win = Arc::new(Window::new(win_id));
        *win.title.lock() = String::from(title);
        self.windows.lock().insert(win_id, win.clone());
        let _ = crate::object::create_object(&win.object_path());
        win
    }

    /// 分配图形缓冲区
    pub fn alloc_buffer(&self, width: u32, height: u32, format: PixelFormat) -> GfxBuffer {
        let mut id = self.next_buffer_id.lock();
        *id += 1;
        let buf_id = *id;
        let size = (width * height * 4) as usize;
        let buf = GfxBuffer {
            id: buf_id,
            width,
            height,
            format,
            phys_addr: 0, // 实际分配物理页
            size,
        };
        self.buffers.lock().insert(buf_id, buf.clone());
        buf
    }

    /// 销毁窗口
    pub fn destroy_window(&self, id: u64) -> Result<(), SysErr> {
        self.windows.lock().remove(&id).ok_or(SysErr::ENOENT)?;
        Ok(())
    }
}

use crate::sync::Once;
static GFX_MGR: Once<GfxManager> = Once::new();

pub fn init() {
    let mgr = GfxManager::new();
    GFX_MGR.call_once(|| mgr);
    let _ = crate::object::create_object("/object/ui");
    let _ = crate::object::create_object("/object/ui/window");
    crate::println!("gfx subsystem ready, /object/ui/window created");
}

pub fn manager() -> &'static GfxManager {
    GFX_MGR.get().expect("gfx manager not initialized")
}
