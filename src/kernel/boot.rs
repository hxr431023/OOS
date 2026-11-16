//! 启动阶段机（boot.md 阶段 3-6）
//! 阶段推进：微内核 -> 对象基座 -> 内核扩展 -> 用户态主程序
use core::sync::atomic::{AtomicU8, Ordering};
use crate::acl::AclMask;

static PHASE: AtomicU8 = AtomicU8::new(0);

pub const P_MICRO: u8 = 3;
pub const P_BASE: u8 = 4;
pub const P_MODULE: u8 = 5;
pub const P_USER: u8 = 6;
pub const P_CMD: u8 = 7;
pub const P_READY: u8 = 8;

pub fn current() -> u8 { PHASE.load(Ordering::SeqCst) }

pub fn phase_name(p: u8) -> &'static str {
    match p {
        1 => "firmware",
        2 => "bootloader",
        3 => "microkernel",
        4 => "object-base",
        5 => "kernel-ext",
        6 => "user-init",
        7 => "command-line",
        8 => "ready",
        _ => "unknown",
    }
}

/// 推进启动阶段：串口/文字控制台提示 + 写启动对象条目
pub fn advance(p: u8) {
    PHASE.store(p, Ordering::SeqCst);
    crate::println!("[boot] phase {} {}", p, phase_name(p));
    if let Ok(obj) = crate::object::lookup("/object/boot") {
        let _ = obj.write_entry("phase", &[p], AclMask::default_system());
    }
}

/// 注册启动流程对象（boot.md 阶段 4；/system/api/boot 契约载体）
pub fn init() {
    if let Ok(o) = crate::object::create_object("/object/boot") {
        let _ = o.create_entry("phase", AclMask::ENTRY_READ);
        let _ = o.create_entry("state", AclMask::ENTRY_READ);
        let _ = o.create_entry("hw_enum", AclMask::ENTRY_READ);
        let _ = o.create_entry("get_phase", AclMask::ENTRY_READ);
    }
    advance(P_MICRO);
}

/// 启动收尾：用户态服务就绪，应用与 GUI 暂不做
pub fn finish() {
    advance(P_READY);
    crate::println!("[boot] system ready: user services up, apps pending, GUI not built");
}
