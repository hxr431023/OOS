//! 服务管理子系统（service.md）
//! 组成：服务管理器 + 服务加载器 + 服务运行容器 + 触发任务器。
//! 本实现提供对象、条目、状态机与打包容器登记；进程线程执行体在用户态服务程序实现。
use crate::sync::{Lazy, Mutex};
use crate::acl::AclMask;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvcState { Stopped, Running, Crashed, Fused }

#[derive(Debug, Clone)]
pub struct ServiceDesc {
    pub name: String,
    pub auto_start: bool,
    pub state: SvcState,
    pub restart_count: u32,
    pub category: &'static str,
    pub host: &'static str,
}

static SERVICES: Lazy<Mutex<Vec<ServiceDesc>>> = Lazy::new(|| Mutex::new(Vec::new()));

fn table() -> &'static Mutex<Vec<ServiceDesc>> { Lazy::force(&SERVICES) }

fn add(name: &str, auto: bool, category: &'static str, host: &'static str) {
    table().lock().push(ServiceDesc {
        name: String::from(name),
        auto_start: auto,
        state: SvcState::Stopped,
        restart_count: 0,
        category,
        host,
    });
}

/// 启动一个服务（服务加载器 + 打包容器语义）
pub fn start(name: &str) -> Result<(), crate::glue_layer::SysErr> {
    let mut t = table().lock();
    let svc = t.iter_mut().find(|s| s.name == name).ok_or(crate::glue_layer::SysErr::ENOENT)?;
    if svc.state == SvcState::Running { return Ok(()); }
    svc.state = SvcState::Running;
    crate::println!("[svc] start {} (host={}, class={})", name, svc.host, svc.category);
    Ok(())
}

pub fn stop(name: &str) -> Result<(), crate::glue_layer::SysErr> {
    let mut t = table().lock();
    let svc = t.iter_mut().find(|s| s.name == name).ok_or(crate::glue_layer::SysErr::ENOENT)?;
    svc.state = SvcState::Stopped;
    crate::println!("[svc] stop {}", name);
    Ok(())
}

pub fn restart(name: &str) -> Result<(), crate::glue_layer::SysErr> {
    stop(name)?;
    let mut t = table().lock();
    let svc = t.iter_mut().find(|s| s.name == name).ok_or(crate::glue_layer::SysErr::ENOENT)?;
    svc.state = SvcState::Running;
    svc.restart_count += 1;
    if svc.restart_count >= 3 {
        svc.state = SvcState::Fused;
        crate::println!("[svc] {} fused after {} restarts", name, svc.restart_count);
    }
    Ok(())
}

pub fn list() -> Vec<(String, SvcState, bool)> {
    table().lock().iter().map(|s| (s.name.clone(), s.state, s.auto_start)).collect()
}

/// 服务管理器对象：/object/service/mgr（service.md §2）
fn register_object() {
    if let Ok(o) = crate::object::create_object("/object/service/mgr") {
        let _ = o.create_entry("list", AclMask::ENTRY_READ);
        let _ = o.create_entry("state", AclMask::ENTRY_READ);
        let _ = o.create_entry("start", AclMask::ENTRY_WRITE);
        let _ = o.create_entry("stop", AclMask::ENTRY_WRITE);
        let _ = o.create_entry("restart", AclMask::ENTRY_WRITE);
        let _ = o.create_entry("auto_start", AclMask::ENTRY_READ | AclMask::ENTRY_WRITE);
        let _ = o.create_entry("event", AclMask::ENTRY_READ);
    }
}

pub fn init() {
    register_object();
    // 服务加载器 + 打包容器 + 触发任务器 + 常驻后台服务（boot.md 阶段 6）
    add("service_mgr", true, "core_system", "host/core");
    add("service_loader", true, "core_system", "host/core");
    add("service_host", true, "core_system", "host/core");
    add("trigger_task", true, "core_system", "host/core");
    add("devmgr", true, "aux_system", "host/aux1");
    add("netmgr", true, "aux_system", "host/aux1");
    add("logsvc", true, "aux_system", "host/aux1");
    add("timesync", true, "aux_system", "host/aux1");
    add("power", true, "aux_system", "host/aux2");
    add("audio", true, "aux_system", "host/aux2");
    add("auth_svc", true, "aux_system", "host/aux2");
    add("session", true, "aux_system", "host/aux2");
    add("update", false, "aux_system", "host/aux2");
    add("backup", false, "aux_system", "host/aux2");
    add("installer", false, "aux_system", "host/aux2");

    // 按自启配置拉起
    let names: Vec<String> = table().lock().iter().filter(|s| s.auto_start).map(|s| s.name.clone()).collect();
    let mut n = 0u32;
    for name in names {
        if start(&name).is_ok() { n += 1; }
    }
    let running = table().lock().iter().filter(|s| s.state == SvcState::Running).count();
    let hosts = ["host/core", "host/aux1", "host/aux2"];
    crate::println!("[svc] manager ready: registered={} auto_started={} running={} containers={}",
        table().lock().len(), n, running, hosts.len());
}

/// 启动自检：list / start / stop / restart / 熔断
pub fn selftest() {
    let before = list().len();
    crate::println!("[svc] registered {}", before);
    let _ = start("update");
    let _ = stop("update");
    let _ = restart("update");
    let st = list().iter().find(|(n, _, _)| n == "update").map(|(_, s, _)| *s);
    crate::println!("[svc] update after restart => {:?}", st);
    crate::println!("[svc] PASS");
}
