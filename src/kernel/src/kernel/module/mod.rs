//! 可加载内核模块机制
//! 模块作为.ko加载，支持重定位、导出符号表
use crate::sync::Mutex;
use crate::glue_layer::SysErr;
use hashbrown::HashMap;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ModuleState {
    Unloaded,
    Loading,
    Loaded,
    Unloading,
    Failed,
}

/// 已加载内核模块
#[derive(Debug)]
pub struct LoadedModule {
    pub name: String,
    pub base_addr: u64,
    pub size: usize,
    pub state: Mutex<ModuleState>,
    /// 模块导出的符号
    pub exported_symbols: Mutex<HashMap<String, u64>>,
    /// 模块依赖
    pub dependencies: Mutex<Vec<String>>,
    /// 模块引用计数
    pub refcount: Mutex<u32>,
}

impl LoadedModule {
    pub fn new(name: &str, base_addr: u64, size: usize) -> Self {
        Self {
            name: String::from(name),
            base_addr,
            size,
            state: Mutex::new(ModuleState::Loading),
            exported_symbols: Mutex::new(HashMap::new()),
            dependencies: Mutex::new(Vec::new()),
            refcount: Mutex::new(0),
        }
    }
}

/// 模块管理器
pub struct ModuleManager {
    pub modules: Mutex<HashMap<String, LoadedModule>>,
    /// 全局符号表：符号名 -> 地址
    pub global_symbols: Mutex<HashMap<String, u64>>,
}

impl ModuleManager {
    pub fn new() -> Self {
        Self {
            modules: Mutex::new(HashMap::new()),
            global_symbols: Mutex::new(HashMap::new()),
        }
    }

    /// 注册内核导出符号（模块加载时解析依赖用）
    pub fn register_symbol(&self, name: &str, addr: u64) {
        self.global_symbols.lock().insert(String::from(name), addr);
    }

    /// 查找符号地址
    pub fn lookup_symbol(&self, name: &str) -> Option<u64> {
        self.global_symbols.lock().get(name).copied()
    }

    /// 加载内核模块（.ko ELF）
    pub fn load_module(&self, name: &str, elf_data: &[u8]) -> Result<(), SysErr> {
        let mut modules = self.modules.lock();
        if modules.contains_key(name) {
            return Err(SysErr::EINVAL);
        }

        // 1. 解析ELF，分配内核内存，拷贝段
        let base_addr = 0xFFFFFFFFC0000000; // 简化：固定地址
        let size = elf_data.len();
        // 实际：解析ELF程序头，映射可加载段，处理重定位

        // 2. 解析依赖，查找全局符号表
        // for dep in module_deps {
        //     if !modules.contains_key(dep) { return Err(SysErr::ENOENT); }
        // }

        // 3. 重定位：解析未定义符号，修正地址
        // for reloc in relocations {
        //     let sym_addr = self.lookup_symbol(reloc.symbol_name)?;
        //     apply_relocation(base_addr + reloc.offset, sym_addr, reloc.type);
        // }

        let module = LoadedModule::new(name, base_addr, size);
        *module.state.lock() = ModuleState::Loaded;

        // 4. 注册模块导出符号到全局表
        // for (sym_name, sym_addr) in module.exported_symbols.lock().iter() {
        //     self.global_symbols.lock().insert(sym_name.clone(), base_addr + sym_addr);
        // }

        modules.insert(String::from(name), module);
        crate::log::info(&alloc::format!("module loaded: {} (size={})", name, size));
        Ok(())
    }

    /// 卸载模块
    pub fn unload_module(&self, name: &str) -> Result<(), SysErr> {
        let mut modules = self.modules.lock();
        let module = modules.get(name).ok_or(SysErr::ENOENT)?;

        // 检查引用计数
        if *module.refcount.lock() > 0 {
            return Err(SysErr::EACCES);
        }

        // 检查是否有其他模块依赖此模块
        for (_other_name, other) in modules.iter() {
            if other.name != name {
                for dep in other.dependencies.lock().iter() {
                    if dep == name {
                        return Err(SysErr::EACCES);
                    }
                }
            }
        }

        *module.state.lock() = ModuleState::Unloading;
        // 从全局符号表移除该模块导出的符号
        // for sym_name in module.exported_symbols.lock().keys() {
        //     self.global_symbols.lock().remove(sym_name);
        // }
        // 释放模块内存
        modules.remove(name);
        crate::log::info(&alloc::format!("module unloaded: {}", name));
        Ok(())
    }

    /// 获取模块信息
    pub fn get_module(&self, name: &str) -> Option<&LoadedModule> {
        // 注意：不能直接返回引用因为Mutex guard生命周期
        // 实际实现需要不同的设计
        let _ = name;
        None
    }
}

use crate::sync::Once;
static MOD_MGR: Once<ModuleManager> = Once::new();

pub fn init() {
    let mgr = ModuleManager::new();
    // 注册内核核心导出符号
    mgr.register_symbol("kobject_create", 0xFFFFFFFF80100000);
    mgr.register_symbol("kobject_lookup", 0xFFFFFFFF80100100);
    mgr.register_symbol("ipc_send", 0xFFFFFFFF80200000);
    mgr.register_symbol("vfs_mount", 0xFFFFFFFF80300000);
    mgr.register_symbol("log_write", 0xFFFFFFFF80400000);
    MOD_MGR.call_once(|| mgr);
    crate::println!("module manager ready, core symbols exported");
}

pub fn manager() -> &'static ModuleManager {
    MOD_MGR.get().expect("module manager not initialized")
}
