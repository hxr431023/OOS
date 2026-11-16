//! VFS 虚拟文件系统
//! 两套后端：磁盘真实文件系统 + 内核IO流对象内存虚拟节点
//! 遵循 fs.md 设计

pub mod inode;
pub mod mount;

use crate::sync::Mutex;
use crate::glue_layer::SysErr;
use crate::acl::AclMask;
use hashbrown::HashMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use inode::{Inode, InodeType};

/// VFS 超级块，管理整个文件系统命名空间
pub struct VfsSuper {
    pub root_inode: Arc<Inode>,
    pub mounts: Mutex<HashMap<String, mount::MountPoint>>,
}

impl VfsSuper {
    pub fn new() -> Self {
        let root = Arc::new(Inode::new("/", InodeType::Directory, 0));
        Self {
            root_inode: root,
            mounts: Mutex::new(HashMap::new()),
        }
    }

    /// 按路径查找 inode
    pub fn lookup(&self, path: &str) -> Result<Arc<Inode>, SysErr> {
        let parts: Vec<&str> = path.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();
        let mut current = self.root_inode.clone();
        for part in parts {
            let next = {
                let children = current.children.lock();
                children.get(part).cloned()
            };
            match next {
                Some(child) => current = child,
                None => return Err(SysErr::ENOENT),
            }
        }
        Ok(current)
    }

    /// 创建 inode（文件或目录）
    pub fn create(&self, path: &str, itype: InodeType) -> Result<Arc<Inode>, SysErr> {
        let parts: Vec<&str> = path.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return Ok(self.root_inode.clone());
        }
        let mut current = self.root_inode.clone();
        for (i, part) in parts.iter().enumerate() {
            let existing = {
                let children = current.children.lock();
                children.get(*part).cloned()
            };
            if let Some(child) = existing {
                current = child;
                continue;
            }
            // 最后一级按指定类型，中间级强制为目录
            let node_type = if i == parts.len() - 1 { itype } else { InodeType::Directory };
            let new_inode = Arc::new(Inode::new(part, node_type, 0));
            current.children.lock().insert(String::from(*part), new_inode.clone());
            current = new_inode;
        }
        Ok(current)
    }

    /// 读取文件数据
    pub fn read(&self, path: &str, _acl: AclMask) -> Result<alloc::vec::Vec<u8>, SysErr> {
        let inode = self.lookup(path)?;
        if matches!(inode.itype, InodeType::Directory) {
            return Err(SysErr::EINVAL);
        }
        Ok(inode.data.lock().clone())
    }

    /// 写入文件数据
    pub fn write(&self, path: &str, data: &[u8], _acl: AclMask) -> Result<(), SysErr> {
        let inode = self.lookup(path)?;
        if matches!(inode.itype, InodeType::Directory) {
            return Err(SysErr::EINVAL);
        }
        *inode.data.lock() = data.to_vec();
        Ok(())
    }

    /// 挂载文件系统到指定路径
    pub fn mount(&self, path: &str, fs_type: &str) -> Result<(), SysErr> {
        let inode = self.lookup(path)?;
        let mp = mount::MountPoint::new(fs_type, inode.clone());
        self.mounts.lock().insert(String::from(path), mp);
        Ok(())
    }

    /// 卸载
    pub fn umount(&self, path: &str) -> Result<(), SysErr> {
        self.mounts.lock().remove(path).ok_or(SysErr::ENOENT)?;
        Ok(())
    }
}

use crate::sync::Once;
static VFS: Once<Arc<VfsSuper>> = Once::new();

pub fn init() {
    let vfs = Arc::new(VfsSuper::new());
    // 创建标准目录结构（对应 fs.md 定义的 VFS 根布局）
    let _ = vfs.create("/system", InodeType::Directory);
    let _ = vfs.create("/system/dev", InodeType::Directory);
    let _ = vfs.create("/system/proc", InodeType::Directory);
    let _ = vfs.create("/system/sys", InodeType::Directory);
// font.md：字体资源目录（外部字体文件 /system/media/*.ttf|otf）
let _ = vfs.create("/system/media", InodeType::Directory);
    let _ = vfs.create("/object", InodeType::Directory);

    // 标准内存虚拟文件（fs.md 后端 B：IO 流对象 / 内核信息映射虚拟节点）
    let rw = AclMask::ENTRY_READ;
    let _ = vfs.create("/system/proc/version", InodeType::VirtualNode);
    let _ = vfs.write(
        "/system/proc/version",
        b"Hybrid Kernel v0.1.0 (Rust, x86_64, no_std)\nmixed-kernel: kobject + acl + ipc + vfs + syscall + procfs\n",
        rw,
    );
    let _ = vfs.create("/system/proc/cpuinfo", InodeType::VirtualNode);
    let _ = vfs.write(
        "/system/proc/cpuinfo",
        b"processor\t: 0\nvendor_id\t: VirtualBox\nmodel\t\t: x86_64\ncores\t\t: 4 (2P + 1M + 1E)\n",
        rw,
    );
    let _ = vfs.create("/system/proc/meminfo", InodeType::VirtualNode);
    {
        let st = crate::memory::mem_stats();
        let s = alloc::format!(
            "MemTotal:\t{} kB\nMemUsed:\t{} kB\nMemFree:\t{} kB\n",
            st.total / 1024,
            st.allocated / 1024,
            st.free / 1024
        );
        let _ = vfs.write("/system/proc/meminfo", s.as_bytes(), rw);
    }
    let _ = vfs.create("/system/dev/serial", InodeType::VirtualNode);
    let _ = vfs.write(
        "/system/dev/serial",
        b"device\t: COM1\nio\t: 0x3F8\nirq\t: 4\n",
        rw,
    );
    let _ = vfs.create("/system/sys/cpu", InodeType::VirtualNode);
    let _ = vfs.write(
        "/system/sys/cpu",
        b"vendor\t: VirtualBox\narch\t: x86_64\ncores\t: 4\n",
        rw,
    );
    let _ = vfs.create("/system/sys/memory", InodeType::VirtualNode);
    let _ = vfs.write("/system/sys/memory", b"type\t: RAM\n", rw);

    VFS.call_once(|| vfs);
    crate::println!("vfs subsystem ready");
}

pub fn vfs() -> &'static Arc<VfsSuper> {
    VFS.get().expect("vfs not initialized")
}
