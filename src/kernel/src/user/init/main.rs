//! init 进程 (PID=1) 用户态示例
//! 由内核拉起，负责启动全部用户态服务、驱动、GUI环境
#![no_std]
#![no_main]

extern crate alloc;

// 链接用户态 libuser 库
// use libuser::{open_object, create_object, ScopedObject};

#[no_mangle]
fn _start() -> ! {
    // init 进程入口
    // 1. 挂载根文件系统
    // 2. 启动日志守护进程
    // 3. 启动设备管理器
    // 4. 启动 API 桥接服务
    // 5. 启动网络守护
    // 6. 启动桌面环境
    loop {
        // init 主循环：等待子进程退出，重启崩溃的服务
        unsafe { core::arch::asm!("hlt") };
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("hlt") }; }
}
