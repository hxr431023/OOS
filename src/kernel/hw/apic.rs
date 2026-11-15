//! APIC 中断控制器（x86-64）
use x86_64::registers::model_specific::Msr;

const APIC_BASE_MSR: u32 = 0x1B;
const APIC_SPURIOUS: u64 = 0xF0;
const APIC_TPR: u64 = 0x80;
const APIC_EOI: u64 = 0xB0;

pub struct Apic {
    base_addr: u64,
}

impl Apic {
    pub fn new() -> Self {
        let mut msr = Msr::new(APIC_BASE_MSR);
        let base = unsafe { msr.read() } & 0xFFFFF000;
        Self { base_addr: base }
    }

    fn read_reg(&self, offset: u64) -> u32 {
        unsafe { core::ptr::read_volatile((self.base_addr + offset) as *const u32) }
    }

    fn write_reg(&self, offset: u64, val: u32) {
        unsafe { core::ptr::write_volatile((self.base_addr + offset) as *mut u32, val) };
    }

    /// 初始化 APIC
    pub fn init(&self) {
        // 设置任务优先级寄存器为0（接受所有中断）
        self.write_reg(APIC_TPR, 0);
        // 启用 APIC，设置假中断向量
        self.write_reg(APIC_SPURIOUS, 0x100 | 0xFF);
    }

    /// 发送中断结束信号（EOI）
    pub fn eoi(&self) {
        self.write_reg(APIC_EOI, 0);
    }

    /// 获取当前 CPU ID
    pub fn cpu_id(&self) -> u32 {
        self.read_reg(0x20) >> 24
    }
}

use crate::sync::Once;
static APIC: Once<Apic> = Once::new();

pub fn init() {
    let apic = Apic::new();
    apic.init();
    let cpu_id = apic.cpu_id();
    APIC.call_once(|| apic);
    crate::println!("apic ready, cpu_id={}", cpu_id);
}

pub fn apic() -> &'static Apic {
    APIC.get().expect("apic not initialized")
}
