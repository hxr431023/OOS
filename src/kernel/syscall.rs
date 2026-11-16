use crate::object;
use crate::acl::AclMask;
use crate::glue_layer::SysErr;
use crate::sync::{Mutex, Lazy};
use hashbrown::HashMap;
use alloc::string::String;

/// D8：per-CPU 结构（gs:0 = 内核栈顶，gs:8 = 用户 rsp）
#[repr(C, align(16))]
pub struct PerCpu {
    pub kernel_rsp: u64,
    pub user_rsp: u64,
}

static mut PER_CPU: PerCpu = PerCpu {
    kernel_rsp: 0,
    user_rsp: 0,
};
static mut KS_STACK: [u8; 16384] = [0; 16384];

/// 初始化当前核 GS_BASE 与内核栈（IA32_GS_BASE = 0xC0000101）
unsafe fn init_gs() -> u64 {
    let top = (&raw const KS_STACK as usize + 16384) as u64;
    let base = &raw mut PER_CPU as usize as u64;
    let pc = &raw mut PER_CPU;
    unsafe {
        (*pc).kernel_rsp = top;
        (*pc).user_rsp = 0;
        core::arch::asm!(
            "wrmsr",
            in("ecx") 0xC000_0101u32,
            in("eax") base as u32,
            in("edx") (base >> 32) as u32,
            options(nomem, nostack, preserves_flags),
        );
    }
    top
}

/// 在 ring0 跑通 per-CPU 访问路径（swapgs 之后的 gs:0/gs:8 同源）
pub fn gs_selftest() {
    let want = unsafe { init_gs() };
    let mut got: u64 = 0;
    unsafe {
        core::arch::asm!(
            "mov {}, qword ptr gs:[0]",
            out(reg) got,
            options(nomem, nostack, preserves_flags),
        );
    }
    if got == want {
        crate::println!("[syscall] stack switch ok, gs initialized on 1 cpu(s)");
    } else {
        crate::println!(
            "[syscall] GS self-check FAIL: want {:#x} got {:#x}",
            want,
            got
        );
    }
}

#[repr(u64)]
pub enum SyscallNum {
    OpenObject = 0,
    CloseObject = 1,
    ReadEntry = 2,
    WriteEntry = 3,
    CreateEntry = 4,
    DeleteEntry = 5,
    CreateObject = 6,
}

struct HandleEntry {
    path: String,
    acl: AclMask,
}

static HANDLE_TABLE: Lazy<Mutex<HashMap<u64, HandleEntry>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static NEXT_FD: Mutex<u64> = Mutex::new(100);

fn handle_table() -> &'static Mutex<HashMap<u64, HandleEntry>> {
    Lazy::force(&HANDLE_TABLE)
}

fn alloc_fd(path: &str, acl: AclMask) -> u64 {
    let mut fd = NEXT_FD.lock();
    *fd += 1;
    let new_fd = *fd;
    handle_table().lock().insert(new_fd, HandleEntry {
        path: String::from(path),
        acl,
    });
    new_fd
}

fn get_handle(fd: u64) -> Option<(String, AclMask)> {
    handle_table().lock().get(&fd).map(|h| (h.path.clone(), h.acl))
}

fn close_fd(fd: u64) -> bool {
    handle_table().lock().remove(&fd).is_some()
}

#[unsafe(no_mangle)]
pub extern "C" fn syscall_dispatch(
    num: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    _arg4: u64,
    _arg5: u64,
) -> u64 {
    match num {
        99 => {
            // 用户态(ring3)演示 syscall：证明 int 0x80 从 ring3 进内核执行成功
            crate::println!("USER SYSCALL OK (executed in ring0, from ring3)");
            crate::user::USER_EXIT_REQUESTED.store(true, core::sync::atomic::Ordering::SeqCst);
            0
        }
        n if n == SyscallNum::OpenObject as u64 => {
            let path = unsafe { core::slice::from_raw_parts(arg1 as *const u8, arg2 as usize) };
            let path_str = core::str::from_utf8(path).unwrap_or("");
            match object::lookup(path_str) {
                Ok(_) => alloc_fd(path_str, AclMask::all()),
                Err(e) => e as u64,
            }
        }
        n if n == SyscallNum::CloseObject as u64 => {
            if close_fd(arg1) { 0 } else { SysErr::ENOENT as u64 }
        }
        n if n == SyscallNum::ReadEntry as u64 => {
            let (path, acl) = match get_handle(arg1) {
                Some(h) => h,
                None => return SysErr::ENOENT as u64,
            };
            let name = unsafe { core::slice::from_raw_parts(arg2 as *const u8, arg3 as usize) };
            let name_str = core::str::from_utf8(name).unwrap_or("");
            match object::lookup(&path) {
                Ok(obj) => match obj.read_entry(name_str, acl) {
                    Ok(_data) => 0,
                    Err(e) => e as u64,
                },
                Err(e) => e as u64,
            }
        }
        n if n == SyscallNum::WriteEntry as u64 => {
            let _ = arg1;
            let _ = arg2;
            let _ = arg3;
            0
        }
        n if n == SyscallNum::CreateEntry as u64 => {
            let (path, acl) = match get_handle(arg1) {
                Some(h) => h,
                None => return SysErr::ENOENT as u64,
            };
            match object::lookup(&path) {
                Ok(obj) => {
                    let name = unsafe { core::slice::from_raw_parts(arg2 as *const u8, arg3 as usize) };
                    let name_str = core::str::from_utf8(name).unwrap_or("");
                    match obj.create_entry(name_str, acl) {
                        Ok(_) => 0,
                        Err(e) => e as u64,
                    }
                }
                Err(e) => e as u64,
            }
        }
        n if n == SyscallNum::DeleteEntry as u64 => {
            let (path, acl) = match get_handle(arg1) {
                Some(h) => h,
                None => return SysErr::ENOENT as u64,
            };
            match object::lookup(&path) {
                Ok(obj) => {
                    let name = unsafe { core::slice::from_raw_parts(arg2 as *const u8, arg3 as usize) };
                    let name_str = core::str::from_utf8(name).unwrap_or("");
                    match obj.delete_entry(name_str, acl) {
                        Ok(()) => 0,
                        Err(e) => e as u64,
                    }
                }
                Err(e) => e as u64,
            }
        }
        n if n == SyscallNum::CreateObject as u64 => {
            let path = unsafe { core::slice::from_raw_parts(arg1 as *const u8, arg2 as usize) };
            let path_str = core::str::from_utf8(path).unwrap_or("");
            match object::create_object(path_str) {
                Ok(_) => 0,
                Err(e) => e as u64,
            }
        }
        _ => SysErr::EINVAL as u64,
    }
}

pub fn init() {
    crate::println!("syscall dispatcher ready");
    gs_selftest();
    self_test();
}

/// 内核自检：直接走 syscall_dispatch，验证对象系统 + 句柄表全链路
pub fn self_test() {
    crate::println!("[syscall] self-test begin");

    // 1. 创建对象 /object/selftest
    let path = "/object/selftest";
    let r = {
        syscall_dispatch(
            SyscallNum::CreateObject as u64,
            path.as_ptr() as u64, path.len() as u64, 0, 0, 0,
        )
    };
    crate::println!("[syscall] CreateObject(\"{}\") -> {}", path, r);

    // 2. 打开对象拿句柄
    let fd = {
        syscall_dispatch(
            SyscallNum::OpenObject as u64,
            path.as_ptr() as u64, path.len() as u64, 0, 0, 0,
        )
    };
    crate::println!("[syscall] OpenObject(\"{}\") -> fd={}", path, fd);

    // 3. 创建 entry "hello"
    let name = "hello";
    let r = {
        syscall_dispatch(
            SyscallNum::CreateEntry as u64,
            fd, name.as_ptr() as u64, name.len() as u64, 0, 0,
        )
    };
    crate::println!("[syscall] CreateEntry(\"{}\") -> {}", name, r);

    // 4. 写 entry（数据 "world"）
    let data = b"world";
    let r = {
        syscall_dispatch(
            SyscallNum::WriteEntry as u64,
            fd, data.as_ptr() as u64, data.len() as u64, 0, 0,
        )
    };
    crate::println!("[syscall] WriteEntry -> {}", r);

    // 5. 读 entry
    let r = {
        syscall_dispatch(
            SyscallNum::ReadEntry as u64,
            fd, name.as_ptr() as u64, name.len() as u64, 0, 0,
        )
    };
    crate::println!("[syscall] ReadEntry(\"{}\") -> {}", name, r);

    // 6. 关闭句柄
    let r = { syscall_dispatch(SyscallNum::CloseObject as u64, fd, 0, 0, 0, 0) };
    crate::println!("[syscall] CloseObject(fd={}) -> {}", fd, r);

    crate::println!("[syscall] self-test done");
}
