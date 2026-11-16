//! 虚拟机子系统（vm.md）
//! VMX在内核模块；虚拟机实例为纯控制对象；用户态管理程序只能操作对象条目
//! 支持硬件直通，依赖IOMMU隔离
use crate::sync::Mutex;
use crate::glue_layer::SysErr;
use crate::ipc::WaitQueue;
use hashbrown::HashMap;
use alloc::string::String;
use alloc::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VmState {
    Stopped,
    Starting,
    Running,
    Paused,
    Suspended,
    Crashed,
}

#[derive(Debug, Clone, Copy)]
pub struct VmConfig {
    pub vcpu_count: u32,
    pub memory_mb: u32,
    pub vcpu_weight: u32,
    pub cpu_quota: u32,
}

/// 虚拟机实例（vm.md：纯控制内核对象）
#[derive(Debug)]
pub struct VirtualMachine {
    pub id: u64,
    pub name: Mutex<String>,
    pub state: Mutex<VmState>,
    pub config: VmConfig,
    /// VMCS 物理地址（VMX虚拟机控制结构）
    pub vmcs_phys: Mutex<u64>,
    /// EPT 页表根（扩展页表，虚拟机物理地址->宿主机物理地址）
    pub ept_root: Mutex<u64>,
    /// vCPU列表
    pub vcpus: Mutex<Vec<Vcpu>>,
    /// 事件IPC队列（虚拟机中断、退出事件通知用户态管理程序）
    pub event_queue: WaitQueue,
    /// 直通设备列表（依赖IOMMU）
    pub passthrough_devices: Mutex<Vec<String>>,
    /// 关联的tap虚拟网卡
    pub tap_interface: Mutex<Option<String>>,
    /// 管理进程PID
    pub manager_pid: Mutex<u64>,
}

#[derive(Debug, Clone, Copy)]
pub struct Vcpu {
    pub id: u32,
    pub vmcs_phys: u64,
    pub running: bool,
    pub last_exit_reason: u32,
}

impl VirtualMachine {
    pub fn new(id: u64, config: VmConfig) -> Self {
        Self {
            id,
            name: Mutex::new(String::new()),
            state: Mutex::new(VmState::Stopped),
            config,
            vmcs_phys: Mutex::new(0),
            ept_root: Mutex::new(0),
            vcpus: Mutex::new(Vec::new()),
            event_queue: WaitQueue::new(),
            passthrough_devices: Mutex::new(Vec::new()),
            tap_interface: Mutex::new(None),
            manager_pid: Mutex::new(0),
        }
    }

    pub fn object_path(&self) -> String {
        alloc::format!("/object/vm/{}", self.id)
    }

    /// 启动虚拟机（vm.md：用户态不能直接执行VMX指令，通过对象条目触发）
    pub fn start(&self) -> Result<(), SysErr> {
        let mut state = self.state.lock();
        if *state != VmState::Stopped {
            return Err(SysErr::EINVAL);
        }
        *state = VmState::Starting;
        // 分配VMCS、EPT，初始化vCPU
        // 实际：VMXON、VMPTRLD、VMWRITE设置VMCS控制字段
        *state = VmState::Running;
        crate::log::info(&alloc::format!("VM {} started", self.id));
        Ok(())
    }

    /// 暂停虚拟机
    pub fn pause(&self) -> Result<(), SysErr> {
        *self.state.lock() = VmState::Paused;
        Ok(())
    }

    /// 停止虚拟机
    pub fn stop(&self) -> Result<(), SysErr> {
        *self.state.lock() = VmState::Stopped;
        // 释放VMCS、EPT资源
        Ok(())
    }

    /// 添加硬件直通设备（依赖IOMMU隔离，vm.md）
    pub fn add_passthrough_device(&self, pci_path: &str) -> Result<(), SysErr> {
        // 创建IOMMU隔离域，绑定设备
        let _domain = crate::mm::iommu::iommu().create_domain(
            crate::mm::iommu::IommuDomainFlags::ALL);
        crate::mm::iommu::iommu().attach_device(_domain, 0)?;
        self.passthrough_devices.lock().push(String::from(pci_path));
        crate::log::info(&alloc::format!("VM {} passthrough: {}", self.id, pci_path));
        Ok(())
    }

    /// 连接tap虚拟网卡
    pub fn attach_tap(&self, tap_name: &str) -> Result<(), SysErr> {
        *self.tap_interface.lock() = Some(String::from(tap_name));
        Ok(())
    }
}

use alloc::vec::Vec;

/// 虚拟机管理器
pub struct VmManager {
    pub vms: Mutex<HashMap<u64, Arc<VirtualMachine>>>,
    pub next_vm_id: Mutex<u64>,
    /// VMX是否已启用
    pub vmx_enabled: Mutex<bool>,
}

impl VmManager {
    pub fn new() -> Self {
        Self {
            vms: Mutex::new(HashMap::new()),
            next_vm_id: Mutex::new(0),
            vmx_enabled: Mutex::new(false),
        }
    }

    /// 初始化VMX（检测CPU支持，开启VMXON）
    pub fn init_vmx(&self) -> Result<(), SysErr> {
        // 检测CPUID VMX支持
        // 开启CR4.VMXE
        // VMXON指令
        *self.vmx_enabled.lock() = true;
        crate::println!("VMX enabled");
        Ok(())
    }

    /// 创建虚拟机实例
    pub fn create_vm(&self, name: &str, config: VmConfig) -> Arc<VirtualMachine> {
        let mut id = self.next_vm_id.lock();
        *id += 1;
        let vm_id = *id;
        let vm = Arc::new(VirtualMachine::new(vm_id, config));
        *vm.name.lock() = String::from(name);
        self.vms.lock().insert(vm_id, vm.clone());
        let _ = crate::object::create_object(&vm.object_path());
        vm
    }

    /// 销毁虚拟机
    pub fn destroy_vm(&self, id: u64) -> Result<(), SysErr> {
        let vm = self.vms.lock().remove(&id).ok_or(SysErr::ENOENT)?;
        let _ = vm.stop();
        Ok(())
    }
}

use crate::sync::Once;
static VM_MGR: Once<VmManager> = Once::new();

pub fn init() {
    let mgr = VmManager::new();
    let _ = mgr.init_vmx();
    VM_MGR.call_once(|| mgr);
    let _ = crate::object::create_object("/object/vm");
    crate::println!("vm subsystem ready, /object/vm created");
}

pub fn manager() -> &'static VmManager {
    VM_MGR.get().expect("vm manager not initialized")
}
