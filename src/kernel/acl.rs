use bitflags::bitflags;

bitflags! {
    pub struct AclMask: u64 {
        const ENTRY_READ   = 1 << 0;
        const ENTRY_WRITE  = 1 << 1;
        const CREATE_ENTRY = 1 << 2;
        const DELETE_ENTRY = 1 << 3;
    }
}

pub fn init() {
    crate::println!("acl subsystem ready");
}

pub fn check_perm(have: AclMask, need: AclMask) -> bool {
    have.contains(need)
}

impl AclMask {
    /// 普通用户：可读可写，不可创建/删除条目
    pub fn default_user() -> Self {
        AclMask::ENTRY_READ | AclMask::ENTRY_WRITE
    }
    /// 系统级：读/写 + 创建/删除
    pub fn default_system() -> Self {
        AclMask::ENTRY_READ | AclMask::ENTRY_WRITE | AclMask::CREATE_ENTRY | AclMask::DELETE_ENTRY
    }
}
