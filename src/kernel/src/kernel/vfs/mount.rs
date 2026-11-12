#![no_std]
use alloc::string::String;
use alloc::sync::Arc;
use super::inode::Inode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsType {
    /// 磁盘真实文件系统
    Disk,
    /// 内核IO流对象内存虚拟节点
    ObjectVirtual,
    /// procfs 模拟（Linux兼容层）
    ProcFs,
    /// sysfs 模拟
    SysFs,
    /// tmpfs 内存文件系统
    TmpFs,
}

#[derive(Debug)]
pub struct MountPoint {
    pub fs_type: String,
    pub root_inode: Arc<Inode>,
    pub mounted: bool,
}

impl MountPoint {
    pub fn new(fs_type: &str, root_inode: Arc<Inode>) -> Self {
        Self {
            fs_type: String::from(fs_type),
            root_inode,
            mounted: true,
        }
    }

    pub fn fs_type_enum(&self) -> FsType {
        match self.fs_type.as_str() {
            "disk" => FsType::Disk,
            "object" => FsType::ObjectVirtual,
            "proc" => FsType::ProcFs,
            "sys" => FsType::SysFs,
            "tmpfs" => FsType::TmpFs,
            _ => FsType::Disk,
        }
    }
}
