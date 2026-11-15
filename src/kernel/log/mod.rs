//! 日志子系统（log.md）
//! 内核环形内存缓冲 + 用户态守护进程落盘；分级日志 + 独立审计日志
use crate::sync::Mutex;
use crate::hw::time;
use alloc::string::String;
use alloc::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
    /// 审计日志（独立，不可被普通进程清除）
    Audit = 4,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp_ms: u64,
    pub level: LogLevel,
    pub message: String,
}

/// 内核环形日志缓冲区
pub struct KernelLog {
    /// 普通日志环形缓冲
    pub buffer: Mutex<VecDeque<LogEntry>>,
    /// 独立审计日志（不可清除）
    pub audit_buffer: Mutex<VecDeque<LogEntry>>,
    /// 最大缓冲条数
    pub max_entries: usize,
}

impl KernelLog {
    pub const fn new() -> Self {
        Self {
            buffer: Mutex::new(VecDeque::new()),
            audit_buffer: Mutex::new(VecDeque::new()),
            max_entries: 4096,
        }
    }

    fn push(&self, level: LogLevel, msg: &str) {
        let entry = LogEntry {
            timestamp_ms: time::now_ms(),
            level,
            message: String::from(msg),
        };
        let target = if matches!(level, LogLevel::Audit) {
            &self.audit_buffer
        } else {
            &self.buffer
        };
        let mut buf = target.lock();
        if buf.len() >= self.max_entries {
            buf.pop_front();
        }
        buf.push_back(entry);
    }

    pub fn debug(&self, msg: &str) { self.push(LogLevel::Debug, msg); }
    pub fn info(&self, msg: &str) { self.push(LogLevel::Info, msg); }
    pub fn warn(&self, msg: &str) { self.push(LogLevel::Warn, msg); }
    pub fn error(&self, msg: &str) { self.push(LogLevel::Error, msg); }

    /// 审计日志（独立缓冲，不可被普通进程清除）
    pub fn audit(&self, msg: &str) { self.push(LogLevel::Audit, msg); }

    /// 读取普通日志（用户态守护进程调用，落盘）
    pub fn drain(&self) -> alloc::vec::Vec<LogEntry> {
        let mut buf = self.buffer.lock();
        buf.drain(..).collect()
    }

    /// 读取审计日志
    pub fn drain_audit(&self) -> alloc::vec::Vec<LogEntry> {
        self.audit_buffer.lock().drain(..).collect()
    }
}

use crate::sync::Once;
static LOG: Once<KernelLog> = Once::new();

pub fn init() {
    LOG.call_once(|| KernelLog::new());
    info("log subsystem ready");
}

pub fn debug(msg: &str) { LOG.get().map(|l| l.debug(msg)); }
pub fn info(msg: &str) { LOG.get().map(|l| l.info(msg)); }
pub fn warn(msg: &str) { LOG.get().map(|l| l.warn(msg)); }
pub fn error(msg: &str) { LOG.get().map(|l| l.error(msg)); }
pub fn audit(msg: &str) { LOG.get().map(|l| l.audit(msg)); }

/// 便捷宏
#[macro_export]
macro_rules! klog {
    ($level:ident, $($arg:tt)*) => {
        $crate::log::$level(&$crate::log::format!($($arg)*))
    };
}
