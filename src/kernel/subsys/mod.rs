//! 系统控制对象批量注册：落地 specs 定义的各子系统纯控制对象与条目
//! 依据：storage.md / security.md / service.md / power.md / backup.md /
//!       pkg.md / update.md / filemgr.md / cmd.md / audio.md / external_dev.md
//! 全部基于 os.md 内核对象-条目模型；对象为纯控制对象，条目布局与各 .api 契约对齐。
use alloc::sync::Arc;
use crate::acl::AclMask;
use crate::object::KObject;

/// 按路径创建（或复用）对象
fn mkobj(path: &str) -> Arc<KObject> {
    crate::object::create_object(path)
        .unwrap_or_else(|_| crate::object::lookup(path).unwrap_or_else(|_| crate::object::root().clone()))
}

/// 在对象下创建条目并写入初始状态（内核初始化阶段持有全权限）
fn mkentry(obj: &Arc<KObject>, name: &str, initial: &str) {
    if obj.create_entry(name, AclMask::default_system()).is_ok() {
        let _ = obj.write_entry(name, initial.as_bytes(), AclMask::default_system());
    }
}

pub fn init() {
    storage_init();
    security_init();
    service_init();
    power_init();
    backup_init();
    pkg_init();
    update_init();
    filemgr_init();
    cmd_init();
    audio_init();
    dev_init();
    devmgr_init();
    crate::log::info("subsystem control objects registered");
    crate::println!("subsystem control objects ready (storage/security/service/power/backup/pkg/update/filemgr/cmd/audio/dev/devmgr)");
}

/// storage.md：/object/storage/mgr 存储管理对象
fn storage_init() {
    let o = mkobj("/object/storage/mgr");
    mkentry(&o, "volume_list", "[vol:0 root 0B]");
    mkentry(&o, "health", "[ok]");
    mkentry(&o, "format", "");
    mkentry(&o, "raid", "");
    mkentry(&o, "mount", "");
    mkentry(&o, "encrypt", "");
    mkentry(&o, "event", "");
    crate::log::info("storage manager object ready");
}

/// security.md：/object/security/mgr 安全体系对象
fn security_init() {
    let o = mkobj("/object/security/mgr");
    mkentry(&o, "session", "[none]");
    mkentry(&o, "fw_rules", "[]");
    mkentry(&o, "audit_list", "[]");
    mkentry(&o, "login", "");
    mkentry(&o, "logout", "");
    mkentry(&o, "fw_add_rule", "");
    mkentry(&o, "encrypt", "");
    mkentry(&o, "decrypt", "");
    crate::log::info("security manager object ready");
}

/// service.md：/object/service/mgr 服务管理对象
fn service_init() {
    let o = mkobj("/object/service/mgr");
    mkentry(&o, "list", "[init]");
    mkentry(&o, "state", "[init running]");
    mkentry(&o, "start", "");
    mkentry(&o, "stop", "");
    mkentry(&o, "restart", "");
    mkentry(&o, "auto_start", "[init:1]");
    mkentry(&o, "event", "");
    crate::log::info("service manager object ready");
}

/// power.md：/object/power/mgr 电源管理对象
fn power_init() {
    let o = mkobj("/object/power/mgr");
    mkentry(&o, "state", "running");
    mkentry(&o, "sleep", "");
    mkentry(&o, "wakeup", "");
    mkentry(&o, "policy", "balanced");
    mkentry(&o, "event", "");
    crate::log::info("power manager object ready");
}

/// backup.md：/object/backup/mgr 备份管理对象
fn backup_init() {
    let o = mkobj("/object/backup/mgr");
    mkentry(&o, "status", "[none]");
    mkentry(&o, "backup", "");
    mkentry(&o, "restore", "");
    mkentry(&o, "list_points", "[]");
    mkentry(&o, "config", "retain=7d");
    crate::log::info("backup manager object ready");
}

/// pkg.md：/object/pkg/mgr 包管理对象
fn pkg_init() {
    let o = mkobj("/object/pkg/mgr");
    mkentry(&o, "list", "[]");
    mkentry(&o, "install", "");
    mkentry(&o, "remove", "");
    mkentry(&o, "upgrade", "");
    mkentry(&o, "source", "[official]");
    mkentry(&o, "state", "idle");
    crate::log::info("pkg manager object ready");
}

/// update.md：/object/update/mgr 更新管理对象
fn update_init() {
    let o = mkobj("/object/update/mgr");
    mkentry(&o, "status", "version=0.1.0");
    mkentry(&o, "check", "");
    mkentry(&o, "apply", "");
    mkentry(&o, "rollback", "");
    mkentry(&o, "channel", "stable");
    crate::log::info("update manager object ready");
}

/// filemgr.md：/object/filemgr/mgr 文件管理对象
fn filemgr_init() {
    let o = mkobj("/object/filemgr/mgr");
    mkentry(&o, "list", "[]");
    mkentry(&o, "copy", "");
    mkentry(&o, "move", "");
    mkentry(&o, "rename", "");
    mkentry(&o, "delete", "");
    mkentry(&o, "mkdir", "");
    mkentry(&o, "stat", "");
    mkentry(&o, "event", "");
    crate::log::info("filemgr manager object ready");
}

/// cmd.md：/object/cmd/exec 命令执行对象（基座阶段创建）
fn cmd_init() {
    let o = mkobj("/object/cmd/exec");
    mkentry(&o, "exec", "");
    mkentry(&o, "output", "");
    mkentry(&o, "exit_code", "0");
    mkentry(&o, "env", "");
    crate::log::info("cmd exec object ready");
}

/// audio.md：/object/audio 音频设备命名空间（IO 流对象，待硬件接入）
fn audio_init() {
    let o = mkobj("/object/audio");
    mkentry(&o, "mgr", "[no audio device]");
    crate::log::info("audio namespace ready (no device)");
}

/// external_dev.md：/object/dev 外部设备命名空间（IO 流对象，待热插拔）
fn dev_init() {
    let o = mkobj("/object/dev");
    mkentry(&o, "mgr", "[no device]");
    crate::log::info("dev namespace ready (no device)");
}

/// devmgr.md：/object/devmgr 设备管理器对象（设备发现/驱动匹配/绑定编排）
fn devmgr_init() {
    let o = mkobj("/object/devmgr");
    mkentry(&o, "devices", "[]");
    mkentry(&o, "drivers", "[official]");
    mkentry(&o, "match_rules", "[vid:pid:class]");
    mkentry(&o, "bind_driver", "");
    crate::log::info("devmgr object ready");
}
