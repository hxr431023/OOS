#![no_std]
//! PCIe 总线枚举（boot.md：内核早期枚举PCI设备，预先创建硬件对象）
use crate::sync::{Mutex, Lazy};
use crate::glue_layer::SysErr;
use hashbrown::HashMap;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy)]
pub struct PciAddr {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

impl PciAddr {
    pub fn new(bus: u8, device: u8, function: u8) -> Self {
        Self { bus, device, function }
    }

    fn config_address(&self, offset: u8) -> u32 {
        0x80000000
            | ((self.bus as u32) << 16)
            | ((self.device as u32) << 11)
            | ((self.function as u32) << 8)
            | (offset as u32 & 0xFC)
    }
}

#[derive(Debug, Clone)]
pub struct PciDevice {
    pub addr: PciAddr,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub revision: u8,
    pub header_type: u8,
    pub bar: [u64; 6],
    pub irq: u8,
}

impl PciDevice {
    /// 设备对象路径：/object/pci/<bus>:<dev>.<func>
    pub fn object_path(&self) -> String {
        alloc::format!(
            "/object/pci/{:02x}:{:02x}.{:x}",
            self.addr.bus, self.addr.device, self.addr.function
        )
    }
}

/// 读取 PCI 配置空间双字
fn pci_config_read(addr: &PciAddr, offset: u8) -> u32 {
    // 0xCF8/0xCFC 是 IO 端口，用 inl/outl 访问（boot 后 MMIO 映射失效会导致枚举返回 0 设备）
    use x86_64::instructions::port::{Port, PortWrite, PortRead};
    let mut acfg = Port::<u32>::new(0xCF8);
    let mut dcfg = Port::<u32>::new(0xCFC);
    unsafe {
        acfg.write(addr.config_address(offset));
        dcfg.read()
    }
}

fn pci_config_read16(addr: &PciAddr, offset: u8) -> u16 {
    (pci_config_read(addr, offset) >> ((offset & 2) * 8)) as u16
}

fn pci_config_read8(addr: &PciAddr, offset: u8) -> u8 {
    (pci_config_read(addr, offset) >> ((offset & 3) * 8)) as u8
}

/// 枚举单个设备功能
fn enumerate_function(addr: PciAddr, devices: &mut Vec<PciDevice>) -> Result<(), SysErr> {
    let vendor_id = pci_config_read16(&addr, 0x00);
    if vendor_id == 0xFFFF {
        return Ok(()); // 不存在
    }

    let device_id = pci_config_read16(&addr, 0x02);
    let class = pci_config_read8(&addr, 0x0B);
    let subclass = pci_config_read8(&addr, 0x0A);
    let prog_if = pci_config_read8(&addr, 0x09);
    let revision = pci_config_read8(&addr, 0x08);
    let header_type = pci_config_read8(&addr, 0x0E);
    let irq = pci_config_read8(&addr, 0x3C);

    let mut bar = [0u64; 6];
    for i in 0..6 {
        let bar_val = pci_config_read(&addr, 0x10 + (i * 4) as u8);
        bar[i] = bar_val as u64;
    }

    let dev = PciDevice {
        addr,
        vendor_id,
        device_id,
        class,
        subclass,
        prog_if,
        revision,
        header_type,
        bar,
        irq,
    };
    devices.push(dev);
    Ok(())
}

/// 枚举全部 PCI 设备
pub fn enumerate_all() -> Vec<PciDevice> {
    let mut devices = Vec::new();
    for bus in 0..=255u8 {
        for device in 0..32u8 {
            let addr = PciAddr::new(bus, device, 0);
            let vendor = pci_config_read16(&addr, 0x00);
            // VirtualBox 对空槽返回 vendor=0x0000（而非 0xFFFF），两者都跳过
            if vendor == 0xFFFF || vendor == 0x0000 {
                continue;
            }
            let header_type = pci_config_read8(&addr, 0x0E);
            let func_count = if header_type & 0x80 != 0 { 8 } else { 1 };
            for func in 0..func_count {
                let faddr = PciAddr::new(bus, device, func);
                let _ = enumerate_function(faddr, &mut devices);
            }
        }
    }
    devices
}

static PCI_DEVICES: Lazy<Mutex<HashMap<String, PciDevice>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn pci_devices() -> &'static Mutex<HashMap<String, PciDevice>> {
    Lazy::force(&PCI_DEVICES)
}

/// 按 PCI class/subclass 识别设备类型（驱动探测分类）
fn class_name(class: u8, subclass: u8) -> &'static str {
    match (class, subclass) {
        (0x01, _) => "mass storage controller",
        (0x02, _) => "network controller",
        (0x03, _) => "display controller",
        (0x04, _) => "multimedia controller",
        (0x06, 0x00) => "host bridge",
        (0x06, 0x01) => "ISA bridge",
        (0x06, 0x04) => "PCI-to-PCI bridge",
        (0x0C, 0x03) => "USB controller",
        (0x0C, 0x05) => "SMBus controller",
        (0x06, _) => "bridge device",
        _ => "other device",
    }
}

pub fn init() {
    let devices = enumerate_all();
    let mut table = pci_devices().lock();
    for dev in &devices {
        let path = dev.object_path();
        crate::println!(
            "pci: {} {:04x}:{:04x} class={:02x}/{:02x} rev={:02x} irq={} bar0={:#x}",
            path,
            dev.vendor_id,
            dev.device_id,
            dev.class,
            dev.subclass,
            dev.revision,
            dev.irq,
            dev.bar[0]
        );
        // 驱动探测：按 class 分类，识别设备角色
        crate::println!("     => {}", class_name(dev.class, dev.subclass));
        table.insert(path, dev.clone());
    }
    crate::println!("pci enumeration done, {} devices", devices.len());
}

pub fn get_device(path: &str) -> Option<PciDevice> {
    pci_devices().lock().get(path).cloned()
}
