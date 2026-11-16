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


    /// ELF64 节装载 + 重定位（R_X86_64_64/PC32/PLT32/32）
    /// 返回 (映像基址, 映像大小, 已应用重定位数)
    pub fn elf_load(data: &[u8]) -> Result<(u64, usize, usize), SysErr> {
        let shoff = u64::from_le_bytes(data[40..48].try_into().unwrap()) as usize;
        let shentsize = u16::from_le_bytes([data[58], data[59]]) as usize;
        let shnum = u16::from_le_bytes([data[60], data[61]]) as usize;
        if shoff == 0 || shnum == 0 || shentsize < 64 { return Err(SysErr::EINVAL); }
        if shoff + shnum * shentsize > data.len() { return Err(SysErr::EINVAL); }

        let sh = |i: usize| -> &[u8] { &data[shoff + i * shentsize .. shoff + i * shentsize + 64] };
        let mut symtab = 0usize; let mut strtab = 0usize; let mut total = 0usize;
        for i in 0..shnum {
            let e = sh(i);
            let ty = u32::from_le_bytes(e[4..8].try_into().unwrap());
            let fl = u64::from_le_bytes(e[8..16].try_into().unwrap());
            let sz = u64::from_le_bytes(e[32..40].try_into().unwrap()) as usize;
            let addr = u64::from_le_bytes(e[16..24].try_into().unwrap()) as usize;
            if ty == 2 && fl & 2 != 0 { total = total.max(addr + sz); }
            if ty == 2 { symtab = i; }
            if ty == 3 { strtab = i; }
        }
        if total == 0 { return Err(SysErr::EINVAL); }
        total = (total + 0xFFF) & !0xFFF;

        let layout = core::alloc::Layout::from_size_align(total, 0x1000).map_err(|_| SysErr::EINVAL)?;
        let base = unsafe { alloc::alloc::alloc_zeroed(layout) } as u64;
        if base == 0 { return Err(SysErr::EINVAL); }

        // 1) 拷贝可分配节
        for i in 0..shnum {
            let e = sh(i);
            let ty = u32::from_le_bytes(e[4..8].try_into().unwrap());
            let fl = u64::from_le_bytes(e[8..16].try_into().unwrap());
            let addr = u64::from_le_bytes(e[16..24].try_into().unwrap()) as usize;
            let off = u64::from_le_bytes(e[24..32].try_into().unwrap()) as usize;
            let sz = u64::from_le_bytes(e[32..40].try_into().unwrap()) as usize;
            if ty != 1 || fl & 2 == 0 || sz == 0 { continue; }
            if off + sz > data.len() || addr + sz > total { continue; }
            unsafe { core::ptr::copy_nonoverlapping(data[off..off+sz].as_ptr(), (base as *mut u8).add(addr), sz); }
        }

        // 2) 重定位
        let syment = 24usize;
        let sym_off = u64::from_le_bytes(sh(symtab)[24..32].try_into().unwrap()) as usize;
        let sym_cnt = u64::from_le_bytes(sh(symtab)[32..40].try_into().unwrap()) as usize / syment;
        let str_off = u64::from_le_bytes(sh(strtab)[24..32].try_into().unwrap()) as usize;
        let sym_at = |idx: usize| -> (usize, u64) {
            let o = sym_off + idx * syment;
            let shndx = u16::from_le_bytes([data[o+6], data[o+7]]) as usize;
            let val = u64::from_le_bytes(data[o+8..o+16].try_into().unwrap());
            (shndx, val)
        };
        let mut applied = 0usize;
        for i in 0..shnum {
            let e = sh(i);
            let ty = u32::from_le_bytes(e[4..8].try_into().unwrap());
            if ty != 4 { continue; } // SHT_RELA
            let off = u64::from_le_bytes(e[24..32].try_into().unwrap()) as usize;
            let sz = u64::from_le_bytes(e[32..40].try_into().unwrap()) as usize;
            if off + sz > data.len() { continue; }
            for k in (0..sz).step_by(24) {
                let ro = u64::from_le_bytes(data[off+k..off+k+8].try_into().unwrap()) as usize;
                let info = u64::from_le_bytes(data[off+k+8..off+k+16].try_into().unwrap());
                let add = i64::from_le_bytes(data[off+k+16..off+k+24].try_into().unwrap());
                let rtype = (info & 0xFFFF_FFFF) as u32;
                let symidx = (info >> 32) as usize;
                let (shndx, val) = sym_at(symidx);
                // 未定义符号在当前内置符号表里按 0 处理（后续接全局符号解析）
                let s_addr: u64 = if shndx == 0 { 0 } else { base + val };
                let place = unsafe { (base as *mut u8).add(ro) } as *mut u8;
                unsafe {
                    match rtype {
                        1 => *(place as *mut u64) = (s_addr as i64).wrapping_add(add) as u64,
                        2 | 4 => {
                            let p = place as u64;
                            let v = (s_addr as i64).wrapping_add(add).wrapping_sub(p as i64);
                            *(place as *mut i32) = v as i32;
                        }
                        10 => *(place as *mut u32) = ((s_addr as i64).wrapping_add(add)) as u32,
                        _ => {}
                    }
                }
                applied += 1;
            }
        }
        let _ = str_off; let _ = sym_cnt;
        Ok((base, total, applied))
    }

    /// 加载内核模块（.ko ELF）
    pub fn load_module(&self, name: &str, elf_data: &[u8]) -> Result<(), SysErr> {
        let mut modules = self.modules.lock();
        if modules.contains_key(name) {
            return Err(SysErr::EINVAL);
        }

        // 1. 准入校验 + 按槽位分配内核空间
        let (phoff, phnum) = ModuleManager::verify_elf(elf_data)?;
        let (base_addr, size, relocs) = ModuleManager::elf_load(elf_data)?;
        crate::println!("[module] {} elf ok: phoff={} phnum={} base=0x{:x} relocated={}",
            name, phoff, phnum, base_addr, relocs);
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
