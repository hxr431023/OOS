#![no_std]
use crate::sync::Mutex;
use hashbrown::HashMap;
use alloc::string::String;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InodeType {
    File,
    Directory,
    /// 内核IO流对象映射的内存虚拟节点（fs.md）
    VirtualNode,
    /// 字符设备
    CharDevice,
    /// 块设备
    BlockDevice,
    /// 符号链接
    Symlink,
}

#[derive(Debug)]
pub struct Inode {
    pub name: String,
    pub itype: InodeType,
    pub ino: u64,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: AtomicU64,
    pub data: Mutex<alloc::vec::Vec<u8>>,
    pub children: Mutex<HashMap<String, Arc<Inode>>>,
    /// 如果是虚拟节点，关联的内核对象路径
    pub object_path: Mutex<Option<String>>,
}

static INO_COUNTER: AtomicU64 = AtomicU64::new(1);

impl Inode {
    pub fn new(name: &str, itype: InodeType, mode: u32) -> Self {
        let ino = INO_COUNTER.fetch_add(1, Ordering::SeqCst);
        Self {
            name: String::from(name),
            itype,
            ino,
            mode,
            uid: 0,
            gid: 0,
            size: AtomicU64::new(0),
            data: Mutex::new(alloc::vec::Vec::new()),
            children: Mutex::new(HashMap::new()),
            object_path: Mutex::new(None),
        }
    }

    /// 标记为内核对象虚拟节点（fs.md：IO流对象映射的内存虚拟节点）
    pub fn set_virtual(&self, object_path: &str) {
        *self.object_path.lock() = Some(String::from(object_path));
    }

    pub fn is_virtual(&self) -> bool {
        self.object_path.lock().is_some()
    }
}
