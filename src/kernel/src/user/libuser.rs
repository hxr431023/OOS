//! 用户态原生对象API封装库
//! 所有用户态程序统一通过此库访问内核对象，禁止直接操作硬件
#![no_std]

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

#[repr(u64)]
enum SyscallNum {
    OpenObject = 0,
    CloseObject = 1,
    ReadEntry = 2,
    WriteEntry = 3,
    CreateEntry = 4,
    DeleteEntry = 5,
    CreateObject = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum UserErr {
    Ok = 0,
    EACCES = 1,
    ENOENT = 2,
    EINVAL = 22,
}

/// 对象句柄
#[derive(Debug, Clone, Copy)]
pub struct ObjectHandle {
    fd: u64,
}

impl ObjectHandle {
    pub fn raw(&self) -> u64 { self.fd }
}

/// 触发系统调用（x86_64 syscall指令）
#[inline(always)]
unsafe fn syscall6(num: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> u64 {
    let ret: u64;
    core::arch::asm!(
        "syscall",
        in("rax") num,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        in("r8") a5,
        lateout("rax") ret,
        out("rcx") _,
        out("r11") _,
    );
    ret
}

fn map_err(v: u64) -> Result<u64, UserErr> {
    match v {
        0 => Ok(0),
        1 => Err(UserErr::EACCES),
        2 => Err(UserErr::ENOENT),
        22 => Err(UserErr::EINVAL),
        _ => Ok(v),
    }
}

/// 打开内核对象，返回句柄
/// 路径示例: "/object/driver_mgr", "/object/net/sock/0"
pub fn open_object(path: &str) -> Result<ObjectHandle, UserErr> {
    let ret = unsafe {
        syscall6(
            SyscallNum::OpenObject as u64,
            path.as_ptr() as u64,
            path.len() as u64,
            0, 0, 0,
        )
    };
    let fd = map_err(ret)?;
    Ok(ObjectHandle { fd })
}

/// 关闭对象句柄
pub fn close_object(handle: ObjectHandle) -> Result<(), UserErr> {
    let ret = unsafe { syscall6(SyscallNum::CloseObject as u64, handle.fd, 0, 0, 0, 0) };
    map_err(ret)?;
    Ok(())
}

/// 读取条目数据
pub fn read_entry(handle: &ObjectHandle, entry_name: &str) -> Result<Vec<u8>, UserErr> {
    let ret = unsafe {
        syscall6(
            SyscallNum::ReadEntry as u64,
            handle.fd,
            entry_name.as_ptr() as u64,
            entry_name.len() as u64,
            0, 0,
        )
    };
    map_err(ret)?;
    // 实际数据通过内核共享缓冲区拷贝，此处简化返回空
    Ok(Vec::new())
}

/// 写入条目数据
pub fn write_entry(handle: &ObjectHandle, entry_name: &str, data: &[u8]) -> Result<(), UserErr> {
    let ret = unsafe {
        syscall6(
            SyscallNum::WriteEntry as u64,
            handle.fd,
            entry_name.as_ptr() as u64,
            entry_name.len() as u64,
            data.as_ptr() as u64,
            data.len() as u64,
        )
    };
    map_err(ret)?;
    Ok(())
}

/// 创建子条目
pub fn create_entry(handle: &ObjectHandle, entry_name: &str) -> Result<(), UserErr> {
    let ret = unsafe {
        syscall6(
            SyscallNum::CreateEntry as u64,
            handle.fd,
            entry_name.as_ptr() as u64,
            entry_name.len() as u64,
            0, 0,
        )
    };
    map_err(ret)?;
    Ok(())
}

/// 删除子条目
pub fn delete_entry(handle: &ObjectHandle, entry_name: &str) -> Result<(), UserErr> {
    let ret = unsafe {
        syscall6(
            SyscallNum::DeleteEntry as u64,
            handle.fd,
            entry_name.as_ptr() as u64,
            entry_name.len() as u64,
            0, 0,
        )
    };
    map_err(ret)?;
    Ok(())
}

/// 创建内核对象
pub fn create_object(path: &str) -> Result<(), UserErr> {
    let ret = unsafe {
        syscall6(
            SyscallNum::CreateObject as u64,
            path.as_ptr() as u64,
            path.len() as u64,
            0, 0, 0,
        )
    };
    map_err(ret)?;
    Ok(())
}

/// 便捷封装：带RAII的对象句柄
pub struct ScopedObject {
    handle: Option<ObjectHandle>,
}

impl ScopedObject {
    pub fn open(path: &str) -> Result<Self, UserErr> {
        Ok(Self { handle: Some(open_object(path)?) })
    }

    pub fn handle(&self) -> &ObjectHandle {
        self.handle.as_ref().unwrap()
    }
}

impl Drop for ScopedObject {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            let _ = close_object(h);
        }
    }
}
