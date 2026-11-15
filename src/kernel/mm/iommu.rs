//! IOMMU (VT-d) 管理（memory.md）
//! 防止DMA越权访问内存，虚拟机直通设备依赖此隔离
use crate::sync::Mutex;
use crate::glue_layer::SysErr;
use hashbrown::HashMap;
use alloc::collections::BTreeSet;

bitflags::bitflags! {
    pub struct IommuDomainFlags: u32 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const ALL = Self::READ.bits() | Self::WRITE.bits();
    }
}

/// IOMMU 域（隔离域）
#[derive(Debug)]
pub struct IommuDomain {
    pub id: u32,
    /// 允许访问的物理页范围
    pub allowed_pages: Mutex<BTreeSet<u64>>,
    pub flags: IommuDomainFlags,
}

impl IommuDomain {
    pub fn new(id: u32, flags: IommuDomainFlags) -> Self {
        Self {
            id,
            allowed_pages: Mutex::new(alloc::collections::BTreeSet::new()),
            flags,
        }
    }

    /// 授权设备访问指定物理页
    pub fn grant_page(&self, phys: u64) {
        self.allowed_pages.lock().insert(phys & !0xFFF);
    }

    /// 撤销授权
    pub fn revoke_page(&self, phys: u64) {
        self.allowed_pages.lock().remove(&(phys & !0xFFF));
    }

    /// 检查DMA访问是否合法
    pub fn check_dma(&self, phys: u64, write: bool) -> bool {
        if write && !self.flags.contains(IommuDomainFlags::WRITE) {
            return false;
        }
        self.allowed_pages.lock().contains(&(phys & !0xFFF))
    }
}

/// IOMMU 管理器
pub struct IommuManager {
    pub domains: Mutex<HashMap<u32, IommuDomain>>,
    pub next_domain_id: Mutex<u32>,
    pub enabled: bool,
}

impl IommuManager {
    pub fn new() -> Self {
        Self {
            domains: Mutex::new(HashMap::new()),
            next_domain_id: Mutex::new(1),
            enabled: false,
        }
    }

    /// 创建隔离域（虚拟机直通设备时调用）
    pub fn create_domain(&self, flags: IommuDomainFlags) -> u32 {
        let mut id = self.next_domain_id.lock();
        *id += 1;
        let domain_id = *id;
        self.domains.lock().insert(domain_id, IommuDomain::new(domain_id, flags));
        domain_id
    }

    /// 将PCI设备绑定到隔离域
    pub fn attach_device(&self, domain_id: u32, _pci_addr: u32) -> Result<(), SysErr> {
        if !self.domains.lock().contains_key(&domain_id) {
            return Err(SysErr::ENOENT);
        }
        // 实际：配置VT-d上下文表，将设备BDF映射到域
        Ok(())
    }

    /// 销毁隔离域
    pub fn destroy_domain(&self, domain_id: u32) -> Result<(), SysErr> {
        self.domains.lock().remove(&domain_id).ok_or(SysErr::ENOENT)?;
        Ok(())
    }
}

use crate::sync::Once;
static IOMMU: Once<IommuManager> = Once::new();

pub fn init() {
    let mgr = IommuManager::new();
    // 检测VT-d硬件，启用IOMMU
    // 实际：解析ACPI DMAR表，配置IOMMU
    IOMMU.call_once(|| mgr);
    crate::println!("iommu manager ready (VT-d)");
}

pub fn iommu() -> &'static IommuManager {
    IOMMU.get().expect("iommu not initialized")
}
