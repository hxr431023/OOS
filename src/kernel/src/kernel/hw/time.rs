//! 内核时间源（TSC + APIC timer）
use core::sync::atomic::{AtomicU64, Ordering};

/// 全局 tick 计数（毫秒）
static TICK_MS: AtomicU64 = AtomicU64::new(0);
/// TSC 频率（MHz），启动时校准
static TSC_MHZ: AtomicU64 = AtomicU64::new(1000);

/// 读取 CPU TSC
pub fn read_tsc() -> u64 {
    unsafe {
        let lo: u32;
        let hi: u32;
        core::arch::asm!(
            "rdtsc",
            out("eax") lo,
            out("edx") hi,
            options(nomem, nostack, preserves_flags)
        );
        ((hi as u64) << 32) | (lo as u64)
    }
}

/// 校准 TSC 频率（实际用 PIT/HPET 校准，此处简化）
pub fn calibrate_tsc() {
    // 简化：假设 1000MHz
    TSC_MHZ.store(1000, Ordering::SeqCst);
}

/// 获取当前毫秒数
pub fn now_ms() -> u64 {
    TICK_MS.load(Ordering::SeqCst)
}

/// APIC timer 中断回调，每次 tick 调用
pub fn on_tick() {
    TICK_MS.fetch_add(1, Ordering::SeqCst);
}

/// 忙等待指定毫秒（早期内核用）
pub fn busy_wait_ms(ms: u64) {
    let start = read_tsc();
    let mhz = TSC_MHZ.load(Ordering::SeqCst);
    let target = start + ms * mhz * 1000;
    while read_tsc() < target {
        core::hint::spin_loop();
    }
}

pub fn init() {
    calibrate_tsc();
    crate::println!("time source ready, tsc={}MHz", TSC_MHZ.load(Ordering::SeqCst));
}
