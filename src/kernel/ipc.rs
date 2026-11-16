//! IPC 子系统（D7）
//!
//! 已实现：消息队列 `/object/ipc/mq`、事件对象 `/object/ipc/event`。
//! 未实现：字节流 IO-IPC 分片收发（docs/ipc.md 字节流章节），
//! 启动日志明确声明 pending，不静默缺失。

use crate::glue_layer::SysErr;
use crate::sync::Mutex;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
pub struct WaitQueue {
    pub waiters: Mutex<VecDeque<u64>>,
}

impl WaitQueue {
    pub const fn new() -> Self {
        Self {
            waiters: Mutex::new(VecDeque::new()),
        }
    }

    pub fn push(&self, pid: u64) {
        self.waiters.lock().push_back(pid);
    }

    pub fn wake_one(&self) -> Option<u64> {
        self.waiters.lock().pop_front()
    }

    /// 唤醒全部等待者，返回被唤醒数量（D5：不再静默抽干丢弃）
    pub fn wake_all(&self) -> usize {
        let mut q = self.waiters.lock();
        let n = q.len();
        q.clear();
        n
    }

    pub fn len(&self) -> usize {
        self.waiters.lock().len()
    }
}

/// 消息队列对象：先进先出队列，容量满时 send 返回 EAGAIN
pub struct MessageQueue {
    msgs: Mutex<VecDeque<Vec<u8>>>,
    capacity: usize,
    pub send_waiters: WaitQueue,
    pub recv_waiters: WaitQueue,
}

impl MessageQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            msgs: Mutex::new(VecDeque::new()),
            capacity,
            send_waiters: WaitQueue::new(),
            recv_waiters: WaitQueue::new(),
        }
    }

    pub fn send(&self, data: &[u8]) -> Result<(), SysErr> {
        {
            let mut q = self.msgs.lock();
            if q.len() >= self.capacity {
                return Err(SysErr::EAGAIN);
            }
            q.push_back(Vec::from(data));
        }
        let n = self.recv_waiters.wake_all();
        if n > 0 {
            crate::println!("[ipc] mq woke {} recv waiter(s)", n);
        }
        Ok(())
    }

    pub fn recv(&self) -> Result<Vec<u8>, SysErr> {
        let msg = self.msgs.lock().pop_front();
        match msg {
            Some(m) => {
                let n = self.send_waiters.wake_all();
                if n > 0 {
                    crate::println!("[ipc] mq woke {} send waiter(s)", n);
                }
                Ok(m)
            }
            None => Err(SysErr::EAGAIN),
        }
    }

    pub fn peek(&self) -> Result<usize, SysErr> {
        Ok(self
            .msgs
            .lock()
            .front()
            .map(|v| v.len())
            .ok_or(SysErr::EAGAIN)?)
    }

    pub fn len(&self) -> usize {
        self.msgs.lock().len()
    }

    pub fn close(&self) {
        self.msgs.lock().clear();
    }
}

/// 事件对象：set/reset/wait/pulse，多等待者复用 WaitQueue
pub struct Event {
    flag: AtomicBool,
    pub waiters: WaitQueue,
}

impl Event {
    pub const fn new() -> Self {
        Self {
            flag: AtomicBool::new(false),
            waiters: WaitQueue::new(),
        }
    }

    pub fn set(&self) -> usize {
        self.flag.store(true, Ordering::SeqCst);
        self.waiters.wake_all()
    }

    pub fn reset(&self) {
        self.flag.store(false, Ordering::SeqCst);
    }

    pub fn wait(&self, pid: u64) -> bool {
        if self.flag.load(Ordering::SeqCst) {
            true
        } else {
            self.waiters.push(pid);
            false
        }
    }

    pub fn pulse(&self) -> usize {
        let n = self.set();
        self.reset();
        n
    }

    pub fn is_set(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

/// 自检：建队列 → 发 3 条 → 收 3 条 → 校验顺序与内容 → 事件 set/wait
pub fn selftest() {
    let mq = MessageQueue::new(4);
    let payloads: [&[u8]; 3] = [b"msg-0", b"msg-1", b"msg-2"];

    for p in payloads.iter() {
        if mq.send(p).is_err() {
            crate::println!("[ipctest] FAIL: send rejected");
            return;
        }
    }

    for (i, expect) in payloads.iter().enumerate() {
        match mq.recv() {
            Ok(got) => {
                if got.as_slice() != *expect {
                    crate::println!("[ipctest] FAIL: order/content mismatch at {}", i);
                    return;
                }
            }
            Err(_) => {
                crate::println!("[ipctest] FAIL: recv empty at {}", i);
                return;
            }
        }
    }

    if mq.len() != 0 {
        crate::println!("[ipctest] FAIL: queue not drained");
        return;
    }

    let ev = Event::new();
    ev.wait(1);
    let woke = ev.set();
    if !ev.is_set() || woke != 1 {
        crate::println!("[ipctest] FAIL: event set/wake mismatch");
        return;
    }
    ev.reset();
    if ev.is_set() {
        crate::println!("[ipctest] FAIL: event reset failed");
        return;
    }

    crate::println!("[ipctest] PASS");
}

pub fn init() {
    let mq_path = "/object/ipc/mq";
    match crate::object::create_object(mq_path) {
        Ok(o) => {
            for name in ["create", "send", "recv", "peek", "close"] {
                let _ = o.create_entry(name, crate::acl::AclMask::default_system());
            }
        }
        Err(_) => crate::println!("[ipc] object {} already present", mq_path),
    }

    let ev_path = "/object/ipc/event";
    match crate::object::create_object(ev_path) {
        Ok(o) => {
            for name in ["set", "reset", "wait", "pulse"] {
                let _ = o.create_entry(name, crate::acl::AclMask::default_system());
            }
        }
        Err(_) => crate::println!("[ipc] object {} already present", ev_path),
    }

    crate::println!("[ipc] mq+event ready, byte-stream IO-IPC pending (docs/ipc.md byte-stream)");
}
