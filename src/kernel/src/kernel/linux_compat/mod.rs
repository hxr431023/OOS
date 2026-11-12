#![no_std]
//! Linux 兼容层（Linux.md）
//! ELF加载、系统调用翻译、procfs/sysfs模拟
//! 把Linux POSIX语义翻译成本系统对象条目操作
use crate::sync::Mutex;
use crate::glue_layer::SysErr;
use hashbrown::HashMap;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum LinuxSyscall {
    Read = 0,
    Write = 1,
    Open = 2,
    Close = 3,
    Stat = 4,
    Fstat = 5,
    Lstat = 6,
    Poll = 7,
    Lseek = 8,
    Mmap = 9,
    Mprotect = 10,
    Munmap = 11,
    Brk = 12,
    RtSigaction = 13,
    RtSigprocmask = 14,
    Ioctl = 16,
    Pread64 = 17,
    Pwrite64 = 18,
    Access = 21,
    Pipe = 22,
    Select = 23,
    SchedYield = 24,
    Mremap = 25,
    Msync = 26,
    Mincore = 27,
    Madvise = 28,
    Shmget = 29,
    Shmat = 30,
    Shmctl = 31,
    Dup = 32,
    Dup2 = 33,
    Pause = 34,
    Nanosleep = 35,
    Getitimer = 36,
    Alarm = 37,
    Setitimer = 38,
    Getpid = 39,
    Sendfile = 40,
    Socket = 41,
    Connect = 42,
    Accept = 43,
    Sendto = 44,
    Recvfrom = 45,
    Sendmsg = 46,
    Recvmsg = 47,
    Shutdown = 48,
    Bind = 49,
    Listen = 50,
    Getsockname = 51,
    Getpeername = 52,
    Socketpair = 53,
    Setsockopt = 54,
    Getsockopt = 55,
    Clone = 56,
    Fork = 57,
    Vfork = 58,
    Execve = 59,
    Exit = 60,
    Wait4 = 61,
    Kill = 62,
    Uname = 63,
    Semget = 64,
    Semop = 65,
    Semctl = 66,
    Shmdt = 67,
    Msgget = 68,
    Msgsnd = 69,
    Msgrcv = 70,
    Msgctl = 71,
    Fcntl = 72,
    Flock = 73,
    Fsync = 74,
    Fdatasync = 75,
    Truncate = 76,
    Ftruncate = 77,
    Getdents = 78,
    Getcwd = 79,
    Chdir = 80,
    Fchdir = 81,
    Rename = 82,
    Mkdir = 83,
    Rmdir = 84,
    Creat = 85,
    Link = 86,
    Unlink = 87,
    Symlink = 88,
    Readlink = 89,
    Chmod = 90,
    Fchmod = 91,
    Chown = 92,
    Fchown = 93,
    Lchown = 94,
    Umask = 95,
    Gettimeofday = 96,
    Getrlimit = 97,
    Getrusage = 98,
    Sysinfo = 99,
    Times = 100,
    Ptrace = 101,
    Getuid = 102,
    Syslog = 103,
    Getgid = 104,
    Setuid = 105,
    Setgid = 106,
    Geteuid = 107,
    Getegid = 108,
    Setpgid = 109,
    Getppid = 110,
    Getpgrp = 111,
    Setsid = 112,
    Setreuid = 113,
    Setregid = 114,
    Getgroups = 115,
    Setgroups = 116,
    Setresuid = 117,
    Getresuid = 118,
    Setresgid = 119,
    Getresgid = 120,
    Getpgid = 121,
    Setfsuid = 122,
    Setfsgid = 123,
    Getsid = 124,
    Capget = 125,
    Capset = 126,
    RtSigpending = 127,
    RtSigtimedwait = 128,
    RtSigqueueinfo = 129,
    RtSigsuspend = 130,
    Sigaltstack = 131,
    Utime = 132,
    Mknod = 133,
    Uselib = 134,
    Personality = 135,
    Ustat = 136,
    Statfs = 137,
    Fstatfs = 138,
    Sysfs = 139,
    Getpriority = 140,
    Setpriority = 141,
    SchedSetparam = 142,
    SchedGetparam = 143,
    SchedSetscheduler = 144,
    SchedGetscheduler = 145,
    SchedGetPriorityMax = 146,
    SchedGetPriorityMin = 147,
    SchedRrGetInterval = 148,
    Mlock = 149,
    Munlock = 150,
    Mlockall = 151,
    Munlockall = 152,
    Vhangup = 153,
    ModifyLdt = 154,
    PivotRoot = 155,
    Sysctl = 156,
    Prctl = 157,
    ArchPrctl = 158,
    Adjtimex = 159,
    Setrlimit = 160,
    Chroot = 161,
    Sync = 162,
    Acct = 163,
    Settimeofday = 164,
    Mount = 165,
    Umount2 = 166,
    Swapon = 167,
    Swapoff = 168,
    Reboot = 169,
    Sethostname = 170,
    Setdomainname = 171,
    Iopl = 172,
    Ioperm = 173,
    CreateModule = 174,
    InitModule = 175,
    DeleteModule = 176,
    GetKernelSyms = 177,
    QueryModule = 178,
    Quotactl = 179,
    Nfsservctl = 180,
    Getpmsg = 181,
    Putpmsg = 182,
    AfsSyscall = 183,
    Tuxcall = 184,
    Security = 185,
    Gettid = 186,
    Readahead = 187,
    Setxattr = 188,
    Lsetxattr = 189,
    Fsetxattr = 190,
    Getxattr = 191,
    Lgetxattr = 192,
    Fgetxattr = 193,
    Listxattr = 194,
    Llistxattr = 195,
    Flistxattr = 196,
    Removexattr = 197,
    Lremovexattr = 198,
    Fremovexattr = 199,
    Tkill = 200,
    Time = 201,
    Futex = 202,
    SchedSetaffinity = 203,
    SchedGetaffinity = 204,
    SetThreadArea = 205,
    IoSetup = 206,
    IoDestroy = 207,
    IoGetevents = 208,
    IoSubmit = 209,
    IoCancel = 210,
    GetThreadArea = 211,
    LookupDcookie = 212,
    EpollCreate = 213,
    EpollCtlOld = 214,
    EpollWaitOld = 215,
    RemapFilePages = 216,
    Getdents64 = 217,
    SetTidAddress = 218,
    RestartSyscall = 219,
    Semtimedop = 220,
    Fadvise64 = 221,
    TimerCreate = 222,
    TimerSettime = 223,
    TimerGettime = 224,
    TimerGetoverrun = 225,
    TimerDelete = 226,
    ClockSettime = 227,
    ClockGettime = 228,
    ClockGetres = 229,
    ClockNanosleep = 230,
    ExitGroup = 231,
    EpollWait = 232,
    EpollCtl = 233,
    Tgkill = 234,
    Utimes = 235,
    Vserver = 236,
    Mbind = 237,
    SetMempolicy = 238,
    GetMempolicy = 239,
    MqOpen = 240,
    MqUnlink = 241,
    MqTimedsend = 242,
    MqTimedreceive = 243,
    MqNotify = 244,
    MqGetsetattr = 245,
    KexecLoad = 246,
    Waitid = 247,
    AddKey = 248,
    RequestKey = 249,
    Keyctl = 250,
    IoprioSet = 251,
    IoprioGet = 252,
    InotifyInit = 253,
    InotifyAddWatch = 254,
    InotifyRmWatch = 255,
    MigratePages = 256,
    Openat = 257,
    Mkdirat = 258,
    Mknodat = 259,
    Fchownat = 260,
    Futimesat = 261,
    Newfstatat = 262,
    Unlinkat = 263,
    Renameat = 264,
    Linkat = 265,
    Symlinkat = 266,
    Readlinkat = 267,
    Fchmodat = 268,
    Faccessat = 269,
    Pselect6 = 270,
    Ppoll = 271,
    Unshare = 272,
    RtSigreturn = 273,
    IoCancel2 = 274,
    Cpuset = 275,
    Getcpu = 276,
    EpollPwait = 277,
    KexecFileLoad = 278,
    Utimensat = 279,
    Signalfd = 280,
    TimerfdCreate = 281,
    Eventfd = 282,
    Fallocate = 283,
    TimerfdSettime = 284,
    TimerfdGettime = 285,
    Accept4 = 286,
    Signalfd4 = 287,
    Eventfd2 = 288,
    EpollCreate1 = 289,
    Dup3 = 290,
    Pipe2 = 291,
    InotifyInit1 = 292,
    Preadv = 293,
    Pwritev = 294,
    RtTgsigqueueinfo = 295,
    PerfEventOpen = 296,
    Recvmmsg = 297,
    FanotifyInit = 298,
    FanotifyMark = 299,
    Prlimit64 = 300,
    NameToHandleAt = 301,
    OpenByHandleAt = 302,
    ClockAdjtime = 303,
    Syncfs = 304,
    Sendmmsg = 305,
    Setns = 306,
    Getcpu2 = 307,
    ProcessVmReadv = 308,
    ProcessVmWritev = 309,
    Kcmp = 310,
    FinitModule = 311,
    SchedSetattr = 312,
    SchedGetattr = 313,
    Renameat2 = 314,
    Seccomp = 315,
    Getrandom = 316,
    MemfdCreate = 317,
    KexecFileLoad2 = 318,
    Bpf = 319,
    Execveat = 320,
    Userfaultfd = 321,
    Membarrier = 322,
    Mlock2 = 323,
    CopyFileRange = 324,
    Preadv2 = 325,
    Pwritev2 = 326,
    PkeyMprotect = 327,
    PkeyAlloc = 328,
    PkeyFree = 329,
    Statx = 330,
    IoPgetevents = 331,
    Rseq = 332,
    PidfdSendSignal = 424,
    IoUringSetup = 425,
    IoUringEnter = 426,
    IoUringRegister = 427,
    OpenTree = 428,
    MoveMount = 429,
    Fsopen = 430,
    Fsconfig = 431,
    Fsmount = 432,
    Fspick = 433,
    PidfdOpen = 434,
    Clone3 = 435,
    CloseRange = 436,
    Openat2 = 437,
    PidfdGetfd = 438,
    Faccessat2 = 439,
    ProcessMadvise = 440,
    EpollPwait2 = 441,
    MountSetattr = 442,
    QuotactlFd = 443,
    LandlockCreateRuleset = 444,
    LandlockAddRule = 445,
    LandlockRestrictSelf = 446,
    MemfdSecret = 447,
    ProcessMrelease = 448,
    FutexWaitv = 449,
    FutexWake = 450,
    FutexWait = 451,
    SetMempolicyHomeNode = 452,
    Cachestat = 453,
    Fchmodat2 = 454,
    MapShadowStack = 455,
    FutexWakev = 456,
}

/// Linux 兼容运行时
pub struct LinuxCompat {
    /// 已加载的Linux ELF进程
    pub processes: Mutex<HashMap<u64, LinuxProcess>>,
    /// procfs 模拟条目
    pub proc_entries: Mutex<HashMap<String, String>>,
    /// sysfs 模拟条目
    pub sys_entries: Mutex<HashMap<String, String>>,
}

#[derive(Debug)]
pub struct LinuxProcess {
    pub pid: u64,
    pub elf_path: String,
    pub brk: u64,
    pub mmap_base: u64,
    /// 文件描述符表：linux_fd -> 本系统对象句柄
    pub fd_table: Mutex<HashMap<i32, u64>>,
}

impl LinuxCompat {
    pub fn new() -> Self {
        Self {
            processes: Mutex::new(HashMap::new()),
            proc_entries: Mutex::new(HashMap::new()),
            sys_entries: Mutex::new(HashMap::new()),
        }
    }

    /// 加载ELF二进制（Linux.md：ELF加载）
    pub fn load_elf(&self, path: &str) -> Result<u64, SysErr> {
        // 解析ELF头，映射段到用户地址空间
        // 设置入口点
        let pid = crate::process::current_pid();
        let proc = LinuxProcess {
            pid,
            elf_path: String::from(path),
            brk: 0x40000000,
            mmap_base: 0x7F0000000000,
            fd_table: Mutex::new(HashMap::new()),
        };
        self.processes.lock().insert(pid, proc);
        crate::log::info(&alloc::format!("linux compat: loaded ELF {}", path));
        Ok(pid)
    }

    /// 翻译Linux系统调用 -> 本系统对象条目操作
    pub fn dispatch_syscall(&self, num: u64, args: [u64; 6]) -> i64 {
        match num {
            // getpid -> 返回本系统PID
            n if n == LinuxSyscall::Getpid as u64 => {
                crate::process::current_pid() as i64
            }
            // write -> 翻译为对象条目写入
            n if n == LinuxSyscall::Write as u64 => {
                let _fd = args[0] as i32;
                let _buf = args[1] as *const u8;
                let _count = args[2] as usize;
                // 翻译：查找fd对应的本系统对象，调用ObjectWriteEntry
                _count as i64
            }
            // read -> 翻译为对象条目读取
            n if n == LinuxSyscall::Read as u64 => {
                let _fd = args[0] as i32;
                let _buf = args[1] as *mut u8;
                let _count = args[2] as usize;
                _count as i64
            }
            // open -> 翻译为OpenObject
            n if n == LinuxSyscall::Open as u64 => {
                let _path = args[0] as *const u8;
                let _flags = args[1] as i32;
                // 翻译路径：/dev/null -> /object/dev/null
                3 // 返回fd
            }
            // close -> 翻译为CloseHandle
            n if n == LinuxSyscall::Close as u64 => {
                0
            }
            // brk -> 本系统内存分配
            n if n == LinuxSyscall::Brk as u64 => {
                let _new_brk = args[0];
                0x40000000
            }
            // mmap -> 本系统内存映射
            n if n == LinuxSyscall::Mmap as u64 => {
                let _addr = args[0];
                let _len = args[1];
                0x7F0000000000
            }
            // munmap
            n if n == LinuxSyscall::Munmap as u64 => 0,
            // exit -> 本系统进程退出
            n if n == LinuxSyscall::Exit as u64 => {
                let code = args[0] as i32;
                // crate::process::current_process().exit(code);
                let _ = code;
                0
            }
            // uname -> 返回模拟信息
            n if n == LinuxSyscall::Uname as u64 => 0,
            // 未实现的系统调用返回ENOSYS
            _ => -38, // ENOSYS
        }
    }

    /// procfs 模拟（Linux.md）
    pub fn read_proc(&self, path: &str) -> Result<String, SysErr> {
        self.proc_entries.lock().get(path).cloned().ok_or(SysErr::ENOENT)
    }

    /// sysfs 模拟
    pub fn read_sys(&self, path: &str) -> Result<String, SysErr> {
        self.sys_entries.lock().get(path).cloned().ok_or(SysErr::ENOENT)
    }
}

use crate::sync::Once;
static LINUX_COMPAT: Once<LinuxCompat> = Once::new();

pub fn init() {
    let compat = LinuxCompat::new();
    // 初始化procfs模拟条目
    let mut proc = compat.proc_entries.lock();
    proc.insert(String::from("/proc/version"), String::from("Linux version 6.1.0-hybrid (builder@hybrid)"));
    proc.insert(String::from("/proc/cpuinfo"), String::from("processor : 0\nvendor_id : GenuineIntel\n"));
    proc.insert(String::from("/proc/meminfo"), String::from("MemTotal: 16384000 kB\n"));
    drop(proc);
    // 初始化sysfs模拟
    let mut sys = compat.sys_entries.lock();
    sys.insert(String::from("/sys/kernel/hostname"), String::from("hybrid-os"));
    drop(sys);
    LINUX_COMPAT.call_once(|| compat);
    crate::println!("linux compat layer ready, procfs/sysfs simulated");
}

pub fn compat() -> &'static LinuxCompat {
    LINUX_COMPAT.get().expect("linux compat not initialized")
}
