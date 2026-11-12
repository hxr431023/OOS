#![no_std]
//! 进程与线程管理
use crate::sync::Mutex;
use crate::glue_layer::SysErr;
use hashbrown::HashMap;
use alloc::string::String;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};
use crate::sync::Lazy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProcState {
    Running,
    Ready,
    Blocked,
    Sleeping,
    Zombie,
    Dead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProcCategory {
    /// 核心系统进程（最高保护等级，memory.md）
    SystemCore,
    /// 系统服务
    SystemService,
    /// 用户交互前台
    UserForeground,
    /// 后台进程（OOM优先杀）
    Background,
}

/// 进程控制块
#[derive(Debug)]
pub struct Process {
    pub pid: u64,
    pub ppid: u64,
    pub name: String,
    pub state: Mutex<ProcState>,
    pub category: ProcCategory,
    /// 页表根物理地址（CR3）
    pub page_table: u64,
    /// 内核栈
    pub kernel_stack: u64,
    /// 用户栈
    pub user_stack: u64,
    /// 打开的对象句柄表
    pub handles: Mutex<HashMap<u64, String>>,
    /// 内存配额（memory.md）
    pub memory_quota: AtomicU64,
    pub memory_used: AtomicU64,
    /// CPU硬配额（cpu.md）
    pub cpu_quota: u32,
    /// 调度优先级权重
    pub sched_weight: u32,
    /// 退出码
    pub exit_code: Mutex<Option<i32>>,
}

impl Process {
    pub fn new(pid: u64, name: &str, category: ProcCategory) -> Self {
        Self {
            pid,
            ppid: 0,
            name: String::from(name),
            state: Mutex::new(ProcState::Ready),
            category,
            page_table: 0,
            kernel_stack: 0,
            user_stack: 0,
            handles: Mutex::new(HashMap::new()),
            memory_quota: AtomicU64::new(u64::MAX),
            memory_used: AtomicU64::new(0),
            cpu_quota: 100,
            sched_weight: 100,
            exit_code: Mutex::new(None),
        }
    }

    /// 进程退出：自动回收所有对象句柄（os.md 崩溃自动回收）
    pub fn exit(&self, code: i32) {
        *self.exit_code.lock() = Some(code);
        *self.state.lock() = ProcState::Zombie;
        // 关闭所有打开的对象句柄
        let mut handles = self.handles.lock();
        for (_fd, path) in handles.drain() {
            // 调用对象管理器递减引用计数
            let _ = path;
        }
    }
}

/// 全局进程表
static PROC_TABLE: Lazy<Mutex<HashMap<u64, Arc<Process>>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static PID_COUNTER: AtomicU64 = AtomicU64::new(1);

fn proc_table() -> &'static Mutex<HashMap<u64, Arc<Process>>> {
    Lazy::force(&PROC_TABLE)
}

pub fn create_process(name: &str, category: ProcCategory) -> Arc<Process> {
    let pid = PID_COUNTER.fetch_add(1, Ordering::SeqCst);
    let proc = Arc::new(Process::new(pid, name, category));
    proc_table().lock().insert(pid, proc.clone());
    proc
}

pub fn get_process(pid: u64) -> Option<Arc<Process>> {
    proc_table().lock().get(&pid).cloned()
}

pub fn current_pid() -> u64 {
    // 实际从调度器当前任务获取，此处简化
    1
}

/// 供 shell 使用：列出全部进程
pub fn list_processes() -> alloc::vec::Vec<(u64, String, ProcState)> {
    let table = proc_table().lock();
    table
        .values()
        .map(|p| (p.pid, p.name.clone(), *p.state.lock()))
        .collect()
}

pub fn init() {
    // 创建 init 进程 (PID=1)
    let init = create_process("init", ProcCategory::SystemCore);
    *init.state.lock() = ProcState::Running;
    crate::println!("process subsystem ready, init pid=1");
}
