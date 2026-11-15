//! CPU 调度器（cpu.md）
//! 支持 P/M/E 异构大小核，按进程类别分配权重，硬配额节流
use crate::sync::Mutex;
use crate::process::{Process, ProcState, ProcCategory};
use alloc::collections::VecDeque;
use alloc::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CoreType {
    /// 性能核 P-core
    Performance,
    /// 能效核 M-core
    Efficient,
    /// 小核 E-core
    Tiny,
}

#[derive(Debug, Clone, Copy)]
pub struct CpuCore {
    pub id: u32,
    pub core_type: CoreType,
    pub current_pid: u64,
    pub load: u32,
}

/// 调度器：按核簇维护就绪队列
pub struct Scheduler {
    pub cores: Mutex<[Option<CpuCore>; 256]>,
    pub core_count: Mutex<usize>,
    /// P核就绪队列
    pub p_ready: Mutex<VecDeque<Arc<Process>>>,
    /// M核就绪队列
    pub m_ready: Mutex<VecDeque<Arc<Process>>>,
    /// E核就绪队列
    pub e_ready: Mutex<VecDeque<Arc<Process>>>,
    /// 全局时间片计数
    pub tick: Mutex<u64>,
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            cores: Mutex::new([None; 256]),
            core_count: Mutex::new(0),
            p_ready: Mutex::new(VecDeque::new()),
            m_ready: Mutex::new(VecDeque::new()),
            e_ready: Mutex::new(VecDeque::new()),
            tick: Mutex::new(0),
        }
    }

    /// 注册CPU核心
    pub fn register_core(&self, id: u32, core_type: CoreType) {
        let mut cores = self.cores.lock();
        cores[id as usize] = Some(CpuCore {
            id,
            core_type,
            current_pid: 0,
            load: 0,
        });
        *self.core_count.lock() += 1;
    }

    /// 根据进程类别选择核簇（cpu.md）
    fn select_queue(&self, category: ProcCategory) -> &Mutex<VecDeque<Arc<Process>>> {
        match category {
            ProcCategory::SystemCore => &self.p_ready,
            ProcCategory::SystemService => &self.m_ready,
            ProcCategory::UserForeground => &self.p_ready,
            ProcCategory::Background => &self.e_ready,
        }
    }

    /// 进程加入就绪队列
    pub fn enqueue(&self, proc: Arc<Process>) {
        *proc.state.lock() = ProcState::Ready;
        self.select_queue(proc.category).lock().push_back(proc);
    }

    /// 调度器 tick：选择下一个进程运行
    pub fn schedule(&self) -> Option<Arc<Process>> {
        *self.tick.lock() += 1;
        // 优先P核队列，然后M，然后E
        if let Some(p) = self.p_ready.lock().pop_front() {
            *p.state.lock() = ProcState::Running;
            return Some(p);
        }
        if let Some(p) = self.m_ready.lock().pop_front() {
            *p.state.lock() = ProcState::Running;
            return Some(p);
        }
        if let Some(p) = self.e_ready.lock().pop_front() {
            *p.state.lock() = ProcState::Running;
            return Some(p);
        }
        None
    }

    /// 进程阻塞（等待IPC事件等）
    pub fn block(&self, proc: &Arc<Process>) {
        *proc.state.lock() = ProcState::Blocked;
    }

    /// 唤醒阻塞进程
    pub fn wake(&self, proc: Arc<Process>) {
        self.enqueue(proc);
    }
}

use crate::sync::Once;
static SCHED: Once<Scheduler> = Once::new();

pub fn init() {
    let sched = Scheduler::new();
    // 假设注册4核：2P + 1M + 1E
    sched.register_core(0, CoreType::Performance);
    sched.register_core(1, CoreType::Performance);
    sched.register_core(2, CoreType::Efficient);
    sched.register_core(3, CoreType::Tiny);
    SCHED.call_once(|| sched);
    crate::println!("scheduler ready, 4 cores (2P+1M+1E)");
}

pub fn scheduler() -> &'static Scheduler {
    SCHED.get().expect("scheduler not initialized")
}
