#![no_std]
use crate::sync::Mutex;
use alloc::collections::VecDeque;

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
}

pub fn init() {
    crate::println!("ipc subsystem ready");
}
