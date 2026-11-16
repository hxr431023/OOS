//! 用户态服务程序登记（service.md：服务进程运行在 Ring3）
//! 本模块只负责“服务程序清单 + 启动计划”，真正的 ring3 装载由 user::spawn_user_program。
use crate::sync::{Lazy, Mutex};
use alloc::vec::Vec;
use alloc::string::String;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode { KernelEmbedded, UserProcess }

#[derive(Debug, Clone)]
pub struct UserProgram {
    pub name: &'static str,
    pub entry: &'static str,
    pub stack_kb: u32,
    pub mode: RunMode,
}

static PROGRAMS: Lazy<Mutex<Vec<UserProgram>>> = Lazy::new(|| Mutex::new(Vec::new()));

fn table() -> &'static Mutex<Vec<UserProgram>> { Lazy::force(&PROGRAMS) }

pub fn register(name: &'static str, entry: &'static str, stack_kb: u32) {
    let mut t = table().lock();
    if t.iter().any(|p| p.name == name) { return; }
    t.push(UserProgram { name, entry, stack_kb, mode: RunMode::UserProcess });
}

pub fn list() -> Vec<(String, &'static str, u32)> {
    table().lock().iter().map(|p| (String::from(p.name), p.entry, p.stack_kb)).collect()
}

/// 启动一个用户态服务：当前阶段只返回计划（ENOSYS），待 ring3 装载器落地后执行
pub fn spawn(name: &str) -> Result<(), crate::glue_layer::SysErr> {
    let t = table().lock();
    let p = t.iter().find(|p| p.name == name).ok_or(crate::glue_layer::SysErr::ENOENT)?;
    crate::println!("[svc-user] spawn plan: {} entry={} stack={}KiB (ring3 loader pending)",
        p.name, p.entry, p.stack_kb);
    Err(crate::glue_layer::SysErr::EINVAL)
}

pub fn init() {
    // 服务管理子系统之外的服务全部按用户态程序登记
    register("service_loader", "/system/userland/services/loader", 64);
    register("service_host", "/system/userland/services/host", 256);
    register("trigger_task", "/system/userland/services/trigger", 64);
    register("devmgr", "/system/userland/services/devmgr", 64);
    register("netmgr", "/system/userland/services/netmgr", 64);
    register("logsvc", "/system/userland/services/logsvc", 64);
    register("timesync", "/system/userland/services/timesync", 32);
    register("power", "/system/userland/services/power", 32);
    register("audio", "/system/userland/services/audio", 64);
    register("auth_svc", "/system/userland/services/auth", 64);
    register("session", "/system/userland/services/session", 64);
    register("update", "/system/userland/services/update", 64);
    register("backup", "/system/userland/services/backup", 64);
    register("installer", "/system/userland/services/installer", 64);
    crate::println!("[svc-user] {} user service program(s) registered", table().lock().len());
}
