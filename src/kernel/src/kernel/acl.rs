#![no_std]
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
