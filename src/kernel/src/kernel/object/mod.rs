#![no_std]
use crate::acl::AclMask;
use crate::sync::{Mutex, Once};
use crate::glue_layer::SysErr;
use hashbrown::HashMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
pub struct KEntry {
    pub name: String,
    pub acl: AclMask,
    pub lib_inst_id: u64,
    pub data: Mutex<Option<alloc::vec::Vec<u8>>>,
    pub waiters: Mutex<alloc::collections::VecDeque<u64>>,
}

impl KEntry {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            acl: AclMask::all(),
            lib_inst_id: 0,
            data: Mutex::new(None),
            waiters: Mutex::new(alloc::collections::VecDeque::new()),
        }
    }
}

#[derive(Debug)]
pub struct KObject {
    pub path: String,
    pub acl: AclMask,
    pub refcnt: AtomicUsize,
    pub entries: Mutex<HashMap<String, Arc<KEntry>>>,
    pub children: Mutex<HashMap<String, Arc<KObject>>>,
}

impl KObject {
    pub fn new(path: &str) -> Self {
        Self {
            path: String::from(path),
            acl: AclMask::all(),
            refcnt: AtomicUsize::new(1),
            entries: Mutex::new(HashMap::new()),
            children: Mutex::new(HashMap::new()),
        }
    }

    pub fn create_entry(&self, name: &str, caller_acl: AclMask) -> Result<Arc<KEntry>, SysErr> {
        if !caller_acl.contains(AclMask::CREATE_ENTRY) {
            return Err(SysErr::EACCES);
        }
        let mut entries = self.entries.lock();
        if entries.contains_key(name) {
            return Err(SysErr::EINVAL);
        }
        let entry = Arc::new(KEntry::new(name));
        entries.insert(String::from(name), entry.clone());
        Ok(entry)
    }

    pub fn delete_entry(&self, name: &str, caller_acl: AclMask) -> Result<(), SysErr> {
        if !caller_acl.contains(AclMask::DELETE_ENTRY) {
            return Err(SysErr::EACCES);
        }
        let mut entries = self.entries.lock();
        let entry = entries.remove(name).ok_or(SysErr::ENOENT)?;
        // 粘合层钩子：清理第三方库实例
        if entry.lib_inst_id != 0 {
            crate::glue_layer::glue_on_kobject_delete(entry.lib_inst_id);
        }
        Ok(())
    }

    pub fn get_entry(&self, name: &str) -> Option<Arc<KEntry>> {
        self.entries.lock().get(name).cloned()
    }

    pub fn read_entry(&self, name: &str, caller_acl: AclMask) -> Result<alloc::vec::Vec<u8>, SysErr> {
        let entry = self.get_entry(name).ok_or(SysErr::ENOENT)?;
        if !caller_acl.contains(AclMask::ENTRY_READ) && !entry.acl.contains(AclMask::ENTRY_READ) {
            return Err(SysErr::EACCES);
        }
        // 实时翻译粘合层：如果有lib_inst_id，实时调用第三方库
        if entry.lib_inst_id != 0 {
            return crate::glue_layer::glue_entry_read(entry.lib_inst_id);
        }
        Ok(entry.data.lock().clone().unwrap_or_default())
    }

    pub fn write_entry(&self, name: &str, data: &[u8], caller_acl: AclMask) -> Result<(), SysErr> {
        let entry = self.get_entry(name).ok_or(SysErr::ENOENT)?;
        if !caller_acl.contains(AclMask::ENTRY_WRITE) && !entry.acl.contains(AclMask::ENTRY_WRITE) {
            return Err(SysErr::EACCES);
        }
        // 实时翻译粘合层
        if entry.lib_inst_id != 0 {
            return crate::glue_layer::glue_entry_write(entry.lib_inst_id, data);
        }
        *entry.data.lock() = Some(data.to_vec());
        // 唤醒等待者
        let mut waiters = entry.waiters.lock();
        while let Some(_pid) = waiters.pop_front() {
            // wake_process(pid);
        }
        Ok(())
    }
}

static ROOT: Once<Arc<KObject>> = Once::new();

pub fn init() {
    let root = Arc::new(KObject::new("/"));
    ROOT.call_once(|| root);
    crate::println!("kobject subsystem ready");
}

pub fn root() -> &'static Arc<KObject> {
    ROOT.get().expect("kobject not initialized")
}

/// 按路径查找对象，支持 /object/driver_mgr 这种多级路径
pub fn lookup(path: &str) -> Result<Arc<KObject>, SysErr> {
    let parts: Vec<&str> = path.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    let mut current = root().clone();
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

pub fn create_object(path: &str) -> Result<Arc<KObject>, SysErr> {
    let parts: Vec<&str> = path.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return Ok(root().clone());
    }
    let mut current = root().clone();
    for (i, part) in parts.iter().enumerate() {
        let existing = {
            let children = current.children.lock();
            children.get(*part).cloned()
        };
        if let Some(child) = existing {
            current = child;
            continue;
        }
        // 创建新对象
        let full_path = if i == 0 {
            String::from("/") + part
        } else {
            current.path.clone() + "/" + part
        };
        let new_obj = Arc::new(KObject::new(&full_path));
        current.children.lock().insert(String::from(*part), new_obj.clone());
        current = new_obj;
    }
    Ok(current)
}
