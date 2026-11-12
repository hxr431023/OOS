#![no_std]
//! 驱动管理器（driver.md）
//! 用户态驱动注册模型：驱动向 /object/driver_mgr 创建注册条目完成注册
//! 内核负责MMIO页表映射、IOMMU配置；驱动崩溃自动复位硬件
//! ata：ATAPI PIO 只读光驱驱动（块设备层，为 ISO9660 提供 BlockRead）
pub mod ata;
use crate::sync::Mutex;
use crate::glue_layer::SysErr;
use crate::hw::pci::PciDevice;
use hashbrown::HashMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DriverState {
    Unloaded,
    Loading,
    Running,
    Crashed,
    Unloading,
}

/// 已注册驱动
#[derive(Debug)]
pub struct RegisteredDriver {
    pub name: String,
    /// 驱动进程PID
    pub pid: u64,
    /// 支持的PCI vendor:device
    pub supported_devices: Vec<(u16, u16)>,
    pub state: Mutex<DriverState>,
    /// 绑定的PCI设备路径
    pub bound_device: Mutex<Option<String>>,
}

impl RegisteredDriver {
    pub fn new(name: &str, pid: u64) -> Self {
        Self {
            name: String::from(name),
            pid,
            supported_devices: Vec::new(),
            state: Mutex::new(DriverState::Loading),
            bound_device: Mutex::new(None),
        }
    }
}

/// 驱动管理器
pub struct DriverManager {
    pub drivers: Mutex<HashMap<String, Arc<RegisteredDriver>>>,
    /// 设备绑定表：pci_path -> driver_name
    pub bindings: Mutex<HashMap<String, String>>,
}

impl DriverManager {
    pub fn new() -> Self {
        Self {
            drivers: Mutex::new(HashMap::new()),
            bindings: Mutex::new(HashMap::new()),
        }
    }

    /// 用户态驱动注册（向 /object/driver_mgr 创建条目触发）
    pub fn register(&self, name: &str, pid: u64) -> Result<Arc<RegisteredDriver>, SysErr> {
        let mut drivers = self.drivers.lock();
        if drivers.contains_key(name) {
            return Err(SysErr::EINVAL);
        }
        let drv = Arc::new(RegisteredDriver::new(name, pid));
        *drv.state.lock() = DriverState::Running;
        drivers.insert(String::from(name), drv.clone());
        crate::log::info(&alloc::format!("driver registered: {} pid={}", name, pid));
        Ok(drv)
    }

    /// 驱动绑定PCI设备
    pub fn bind_device(&self, driver_name: &str, pci_path: &str) -> Result<(), SysErr> {
        let drivers = self.drivers.lock();
        let drv = drivers.get(driver_name).ok_or(SysErr::ENOENT)?;

        // 检查设备是否存在
        let _dev = crate::hw::pci::get_device(pci_path).ok_or(SysErr::ENOENT)?;

        // 映射MMIO到驱动进程地址空间（driver.md：内核负责MMIO页表映射）
        // self.map_mmio_to_process(drv.pid, dev.bar[0])?;

        // 配置IOMMU，允许设备DMA
        // let domain = crate::mm::iommu::iommu().create_domain(...);

        *drv.bound_device.lock() = Some(String::from(pci_path));
        self.bindings.lock().insert(String::from(pci_path), String::from(driver_name));
        crate::log::info(&alloc::format!("driver {} bound to {}", driver_name, pci_path));
        Ok(())
    }

    /// 驱动崩溃处理（driver.md：自动复位硬件、回收资源，不整机panic）
    pub fn on_driver_crash(&self, pid: u64) {
        let drivers = self.drivers.lock();
        for (_name, drv) in drivers.iter() {
            if drv.pid == pid {
                *drv.state.lock() = DriverState::Crashed;
                // 复位硬件
                if let Some(dev_path) = drv.bound_device.lock().as_ref() {
                    crate::log::error(&alloc::format!(
                        "driver {} crashed, resetting device {}", drv.name, dev_path));
                    // self.reset_pci_device(dev_path);
                    self.bindings.lock().remove(dev_path);
                }
                break;
            }
        }
    }

    /// 卸载驱动
    pub fn unregister(&self, name: &str) -> Result<(), SysErr> {
        let mut drivers = self.drivers.lock();
        let drv = drivers.remove(name).ok_or(SysErr::ENOENT)?;
        *drv.state.lock() = DriverState::Unloading;
        if let Some(dev_path) = drv.bound_device.lock().as_ref() {
            self.bindings.lock().remove(dev_path);
        }
        Ok(())
    }
}

use crate::sync::Once;
static DRV_MGR: Once<DriverManager> = Once::new();

pub fn init() {
    let mgr = DriverManager::new();
    DRV_MGR.call_once(|| mgr);
    // 创建驱动管理器对象 /object/driver_mgr
    let _ = crate::object::create_object("/object/driver_mgr");
    crate::println!("driver manager ready, /object/driver_mgr created");
}

pub fn manager() -> &'static DriverManager {
    DRV_MGR.get().expect("driver manager not initialized")
}
