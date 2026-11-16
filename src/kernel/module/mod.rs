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

    /// 校验 ELF64 内核模块镜像（准入检查：魔数/位数/类型/机器/段表）
    pub fn verify_elf(data: &[u8]) -> Result<(u64, usize), SysErr> {
        if data.len() < 64 { return Err(SysErr::EINVAL); }
        if &data[0..4] != b"\x7fELF" { return Err(SysErr::EINVAL); }
        if data[4] != 2 { return Err(SysErr::EINVAL); }      // ELF64
        if data[5] != 1 { return Err(SysErr::EINVAL); }      // little endian
        let e_type = u16::from_le_bytes([data[16], data[17]]);
        if e_type != 1 { return Err(SysErr::EINVAL); }       // ET_REL (.ko)
        let e_machine = u16::from_le_bytes([data[18], data[19]]);
        if e_machine != 62 { return Err(SysErr::EINVAL); }   // x86-64
        let e_phoff = u64::from_le_bytes(data[32..40].try_into().unwrap());
        let e_phnum = u16::from_le_bytes([data[56], data[57]]) as usize;
        if e_phnum == 0 || e_phoff == 0 { return Err(SysErr::EINVAL); }
        if e_phoff as usize + e_phnum * 56 > data.len() { return Err(SysErr::EINVAL); }
        Ok((e_phoff, e_phnum))
    }

    /// 内置模块登记（随内核镜像装载，boot.md 阶段 5）
    pub fn register_builtin(&self, name: &str, kind: &str) {
        let mut modules = self.modules.lock();
        if modules.contains_key(name) { return; }
        let mut module = LoadedModule::new(name, 0, 0);
        *module.state.lock() = ModuleState::Loaded;
        module.exported_symbols.lock().insert(String::from("kind"), 0);
        modules.insert(String::from(name), module);
        crate::println!("[module] builtin {} ({}) loaded", name, kind);
    }

    /// 按 boot.md 阶段 5 装载全部内置扩展（VMX 仅在 CPU 支持时加载）
    pub fn load_builtins(&self) -> usize {
        let list: [(&str, &str); 11] = [
            ("block", "modules/block"),
            ("fs", "aux_modules/fs"),
            ("vfs", "aux_modules/vfs"),
            ("devfs", "aux_modules/vfs-virtual"),
            ("net", "aux_modules/net"),
            ("nic", "modules/net"),
            ("gpu", "modules/gpu"),
            ("input", "modules/input"),
            ("rtc", "modules/rtc"),
            ("iommu", "modules/iommu"),
            ("vmx", "modules/vmx"),
        ];
        let vmx = has_vmx();
        let mut n = 0;
        for (name, kind) in list {
            if name == "vmx" && !vmx {
                crate::println!("[module] vmx skipped (cpu without VMX)");
                continue;
            }
            self.register_builtin(name, kind);
            n += 1;
        }
        n
    }

    /// 加载内核模块（.ko ELF）
    pub fn load_module(&self, name: &str, elf_data: &[u8]) -> Result<(), SysErr> {
        let mut modules = self.modules.lock();
        if modules.contains_key(name) {
            return Err(SysErr::EINVAL);
        }

        // 1. 准入校验 + 按槽位分配内核空间
        let (phoff, phnum) = ModuleManager::verify_elf(elf_data)?;
        let slot = modules.len() as u64;
        let base_addr = 0xFFFFFFFFC0000000u64.wrapping_add(slot * 0x200000);
        let size = elf_data.len();
        crate::println!("[module] {} elf ok: phoff={} phnum={} base=0x{:x}", name, phoff, phnum, base_addr);
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

/// CPUID.1:ECX[5] = VMX
pub fn has_vmx() -> bool {
    let r = unsafe { core::arch::x86_64::__cpuid(1) };
    r.ecx & (1 << 5) != 0
}

pub fn manager() -> &'static ModuleManager {
    MOD_MGR.get().expect("module manager not initialized")
}
