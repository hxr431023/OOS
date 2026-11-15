//! 涓插彛浜や簰 shell锛氳绯荤粺鍦?VirtualBox 閲屽彲鏁插懡浠ゃ€佺湅鍝嶅簲锛堢湡姝ｇ殑鎿嶄綔绯荤粺浜や簰锛?
use alloc::string::String;
use alloc::string::ToString;
use alloc::format;
use alloc::vec::Vec;
use crate::process::list_processes;
use crate::object;
use crate::driver_mgr::ata::AtapiDrive;
use crate::fs::iso9660::Iso9660;

/// 已挂载的 ISO9660 光盘（mountcd 成功后可用），供 dir/type/cd 浏览光盘内容
pub struct MountedIso {
    pub fs: Iso9660<AtapiDrive>,
    pub cwd: alloc::string::String,
}
static MOUNT: crate::sync::Mutex<Option<MountedIso>> = crate::sync::Mutex::new(None);

/// 可变的 shell 提示符（prompt 命令）
static PROMPT_STR: crate::sync::Mutex<alloc::string::String> =
    crate::sync::Mutex::new(alloc::string::String::new());

/// 褰撳墠 shell 宸ヤ綔鐩綍锛坈md.md 搂3锛氱粓绔矾寰勶紱鍐呮牳绾х畝鍖栧疄鐜帮級
static mut CWD: alloc::string::String = alloc::string::String::new();

/// 涓诲惊鐜細杞 COM1 杈撳叆锛岀疮绉懡浠わ紝鍥炶溅鎵ц
pub fn run() -> ! {
    let mut line = [0u8; 256];
    let mut len = 0usize;
    crate::println!("\n=== Hybrid 系统 ===");
    crate::println!("输入 \'help\' 查看命令");
    acltest_selfcheck();
    // D7 启动自检：串口留证 ipctest PASS
    crate::ipc::selftest();
    {
        let st = crate::memory::mem_stats();
        crate::println!("[meminfo] total={}KiB allocated={}KiB free={}KiB used={}%", st.total / 1024, st.allocated / 1024, st.free / 1024, if st.total > 0 { st.allocated * 100 / st.total } else { 0 });
    }
    print_prompt();
    loop {
        if let Some(c) = crate::keyboard::poll_char() {
            match c {
                b'\r' | b'\n' => {
                    crate::println!("");
                    if len > 0 {
                        let cmd = core::str::from_utf8(&line[..len]).unwrap_or("");
                        exec(cmd);
                        len = 0;
                    }
                    print_prompt();
                }
                0x08 | 0x7F => {
                    if len > 0 {
                        len -= 1;
                        // 用 put_str 输出退格序列（不换行），否则 println 的 \n 会让光标跳下一行
                        crate::vga::put_str("\x08 \x08");
                    }
                }
                c if c >= 0x20 && c < 0x7F => {
                    if len < 255 {
                        line[len] = c;
                        len += 1;
                    }
                    // 鍥炴樉鍒板睆骞曪紙涓嶆崲琛岋紱println 浼氬姞 \n 瀵艰嚧姣忛敭鎹㈣锛?
                    crate::vga::put_char(c);
                }
                _ => {}
            }
        }
        // 鐢?pause 绌鸿浆杞锛坔lt 浼氬仠 CPU 瀵艰嚧鏃犳硶鍐嶈疆璇㈣緭鍏ワ紱鏈厤瀹氭椂鍣ㄤ腑鏂級
        unsafe { core::arch::asm!("pause") };
    }
}

fn exec(cmd: &str) {
    let cmd = cmd.trim();
    let mut parts = cmd.split_whitespace();
    let name = parts.next().unwrap_or("").to_lowercase();
    match name.as_str() {
        "" => {}
        "help" => {
            crate::println!("CMD 风格命令:");
            crate::println!("  dir [路径]     列出目录(ISO/VFS)   type <路径>  查看文件");
            crate::println!("  cd <目录>     切换目录          vol         卷标");
            crate::println!("  cls           清屏              echo <文本>  打印文本");
            crate::println!("  ver           内核版本          memory      内存统计");
            crate::println!("  systeminfo    系统信息          date/time   时钟");
            crate::println!("  color/prompt  颜色/提示符       reboot      重启系统");
            crate::println!("  shutdown      关机(ACPI)        pwd         当前目录");
            crate::println!("内核命令:");
            crate::println!("  ps / tree / netstat / whoami / acl / sysobj");
            crate::println!("  ls / cat / mountcd / fonts / fontload / fontinit");
            crate::println!("  syscall / user / diag / cpuinfo / meminfo / sysinfo");
            crate::println!("  charmap / charcopy / measure / loadcjk / zh");
            crate::println!("  fontformats / fontresmap");
        }
        "ps" => {
            let procs = list_processes();
            crate::println!("PID  NAME        STATE");
            for (pid, name, state) in &procs {
                crate::println!("{:>3}  {:<10} {:?}", pid, name, state);
            }
            crate::println!("{} process(es)", procs.len());
        }
        "user" => {
            unsafe {
                crate::user::prepare_user();
            }
            crate::println!("switching to ring3 user mode (demo)...");
            unsafe { crate::user::switch_to_user() };
        }
        "diag" => {
            crate::println!("--- user-mode diag ---");
            unsafe {
                crate::user::dump_user_diag();
            }
        }
        "ipctest" => {
            crate::ipc::selftest();
        }
        "syscall" => {
            crate::syscall::self_test();
        }
        "echo" => {
            let rest = cmd.splitn(2, ' ').nth(1).unwrap_or("");
            let rest = rest.trim_start();
            if let Some(gt) = rest.find('>') {
                // 鍐?VFS 铏氭嫙鏂囦欢锛歟cho text > /path锛坒s.md 鍐欓€氳矾锛?
                let text = rest[..gt].trim();
                let path = rest[gt + 1..].trim();
                let vfs = crate::vfs::vfs();
                match vfs.write(path, text.as_bytes(), crate::acl::AclMask::ENTRY_WRITE) {
                    Ok(_) => crate::println!("wrote {} bytes to {}", text.len(), path),
                    Err(_) => crate::println!("echo: {}: write failed (read-only or not found)", path),
                }
            } else {
                crate::println!("{}", rest);
            }
        }
        "pwd" => {
            unsafe {
                let cwd = &*core::ptr::addr_of!(CWD);
                if cwd.is_empty() {
                    crate::println!("/");
                } else {
                    crate::println!("{}", cwd);
                }
            }
        }
        // ---- CMD 风格命令 ----
        // 浏览已挂载光盘（mountcd 后）：dir / type / cd 走 ISO9660；未挂载时回退 VFS
        "dir" | "ls" => {
            let arg = parts.next().unwrap_or(".");
            let guard = MOUNT.lock();
            if let Some(mi) = guard.as_ref() {
                let full = resolve_iso_path(&arg, &mi.cwd);
                match mi.fs.lookup(&full) {
                    Ok(ent) if ent.is_dir => match mi.fs.read_dir(&ent) {
                        Ok(list) => {
                            let mut names: Vec<_> = list.iter().map(|e| e.name.clone()).collect();
                            names.sort();
                            for n in &names {
                                crate::println!("{}", n);
                            }
                            crate::println!("{} entry(s)", names.len());
                        }
                        Err(e) => crate::println!("dir: {}: read failed {:?}", arg, e),
                    },
                    Ok(_) => crate::println!("{}", full),
                    Err(_) => crate::println!("dir: {}: not found", arg),
                }
            } else {
                drop(guard);
                let arg = if arg == "." { "/" } else { arg };
                let vfs = crate::vfs::vfs();
                match vfs.lookup(arg) {
                    Ok(inode) => {
                        if matches!(inode.itype, crate::vfs::inode::InodeType::Directory) {
                            let ch = inode.children.lock();
                            let mut names: Vec<_> = ch.keys().cloned().collect();
                            names.sort();
                            for n in &names {
                                crate::println!("{}", n);
                            }
                            crate::println!("{} entry(s)", names.len());
                        } else {
                            crate::println!("{}", arg);
                        }
                    }
                    Err(_) => crate::println!("dir: {}: not found", arg),
                }
            }
        }
        "type" | "cat" => {
            let arg = parts.next().unwrap_or("");
            if arg.is_empty() {
                crate::println!("usage: type <path>");
            } else {
                let guard = MOUNT.lock();
                if let Some(mi) = guard.as_ref() {
                    let full = resolve_iso_path(&arg, &mi.cwd);
                    match mi.fs.lookup(&full) {
                        Ok(ent) if !ent.is_dir => match mi.fs.read_file(&ent) {
                            Ok(data) => {
                                let s = core::str::from_utf8(&data).unwrap_or("<binary>");
                                crate::println!("{}", s);
                            }
                            Err(e) => crate::println!("type: {}: read failed {:?}", arg, e),
                        },
                        Ok(_) => crate::println!("type: {}: is a directory", arg),
                        Err(_) => crate::println!("type: {}: not found", arg),
                    }
                } else {
                    drop(guard);
                    let vfs = crate::vfs::vfs();
                    match vfs.read(arg, crate::acl::AclMask::ENTRY_READ) {
                        Ok(data) => {
                            let s = core::str::from_utf8(&data).unwrap_or("<binary>");
                            crate::println!("{}", s);
                        }
                        Err(_) => crate::println!("type: {}: not found", arg),
                    }
                }
            }
        }
        "cd" | "chdir" => {
            let arg = parts.next().unwrap_or("/");
            let mut guard = MOUNT.lock();
            if let Some(mi) = guard.as_mut() {
                let full = resolve_iso_path(&arg, &mi.cwd);
                match mi.fs.lookup(&full) {
                    Ok(ent) if ent.is_dir => {
                        mi.cwd = if arg.starts_with('/') {
                            String::from(arg)
                        } else {
                            full.clone()
                        };
                        crate::println!("{}", full);
                    }
                    Ok(_) => crate::println!("cd: {}: not a directory", arg),
                    Err(_) => crate::println!("cd: {}: no such directory", arg),
                }
            } else {
                drop(guard);
                if let Ok(inode) = crate::vfs::vfs().lookup(arg) {
                    if matches!(inode.itype, crate::vfs::inode::InodeType::Directory) {
                        unsafe {
                            let cwd = &mut *core::ptr::addr_of_mut!(CWD);
                            cwd.clear();
                            cwd.push_str(arg);
                        }
                        crate::println!("{}", arg);
                    } else {
                        crate::println!("cd: {}: not a directory", arg);
                    }
                } else {
                    crate::println!("cd: {}: no such directory", arg);
                }
            }
        }
        "vol" => {
            if MOUNT.lock().is_some() {
                crate::println!("Volume in drive CDROM is HYBRIDDATA (ISO9660)");
            } else {
                crate::println!("Volume in drive VFS is HYBRID (virtual filesystem)");
            }
        }
        "ver" | "version" => {
            crate::println!("Hybrid Kernel v0.1.0 (Rust, x86_64, no_std)");
            crate::println!("mixed-kernel: BIOS+UEFI boot + kobject + acl + ipc + vfs + iso9660");
        }
        "cls" | "clear" => {
            crate::println!("\x1b[2J\x1b[H");
        }
        "memory" | "meminfo" => {
            let s = crate::memory::mem_stats();
            crate::println!("[meminfo] total={}KiB allocated={}KiB free={}KiB used={}%", s.total / 1024, s.allocated / 1024, s.free / 1024, if s.total > 0 { s.allocated * 100 / s.total } else { 0 });
        }
        "entryview" => {
            use crate::acl::AclMask;
            let obj = match crate::object::create_object("/object/acltest") {
                Ok(o) => o,
                Err(_) => crate::object::lookup("/object/acltest").unwrap(),
            };
            let big = alloc::vec![0xABu8; 64 * 1024];
            let _ = obj.create_entry("big", AclMask::default_system());
            let _ = obj.write_entry("big", &big, AclMask::default_system());
            let mut dst = alloc::vec![0u8; 64 * 1024];
            let before = crate::memory::mem_stats().allocated;
            let mut ok = true;
            for _ in 0..100 {
                match obj.read_entry_into("big", AclMask::ENTRY_READ, &mut dst) {
                    Ok(n) if n == 64 * 1024 => {}
                    _ => { ok = false; }
                }
            }
            let after = crate::memory::mem_stats().allocated;
            crate::println!("[entryview] 64KB x100 read_entry_into => {} alloc_delta={}B",
                if ok { "OK" } else { "FAIL" }, after.saturating_sub(before));
        }
        "acltest" => {
            use crate::acl::AclMask;
            let obj = match crate::object::create_object("/object/acltest") {
                Ok(o) => o,
                Err(_) => crate::object::lookup("/object/acltest").unwrap(),
            };
            let _ = obj.create_entry("probe", AclMask::default_system());
            let _ = obj.write_entry("probe", b"seed", AclMask::default_system());
            let r_empty = obj.read_entry("probe", AclMask::empty());
            let r_rd = obj.read_entry("probe", AclMask::ENTRY_READ);
            let r_wr = obj.write_entry("probe", b"x", AclMask::ENTRY_READ);
            crate::println!("[acltest] empty read => {}", if r_empty.is_err() { "EACCES(deny) OK" } else { "ALLOWED BAD" });
            crate::println!("[acltest] read-only read => {}", if r_rd.is_ok() { "OK" } else { "DENIED BAD" });
            crate::println!("[acltest] read-only write => {}", if r_wr.is_err() { "EACCES(deny) OK" } else { "ALLOWED BAD" });
            if let Some(e) = obj.get_entry("probe") { e.waiters.push(7); }
    let _ = obj.write_entry("probe", b"wakeup", AclMask::default_system());
    let pass = r_empty.is_err() && r_rd.is_ok() && r_wr.is_err();
            crate::println!("[acltest] {}", if pass { "PASS" } else { "FAIL" });
        }
        "systeminfo" | "sysinfo" => {
            crate::println!("Hybrid Kernel v0.1.0");
            crate::println!("arch: x86_64, mode: long mode");
            crate::println!("kernel: mixed (micro + loadable modules)");
            crate::println!("boot: BIOS/UEFI hybrid, compatible");
            crate::println!("fs: iso9660 (CD-ROM) + vfs (memory)");
            crate::println!("scheduler: {} process(es)", list_processes().len());
        }
        "date" => {
            let (y, mo, d) = rtc_date();
            let (h, mi, s) = rtc_time();
            crate::println!("Current date: {}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, h, mi, s);
        }
        "time" => {
            let (h, mi, s) = rtc_time();
            crate::println!("Current time: {:02}:{:02}:{:02}", h, mi, s);
        }
        "reboot" => {
            crate::println!("rebooting...");
            // 尝试 ACPI reset，失败则触发三连故障重启
            unsafe {
                core::arch::asm!("out dx, eax", in("dx") 0xCF9u16, in("eax") 0x06u32, options(nomem, nostack));
            }
            triple_fault()
        }
        "prompt" => {
            let rest = cmd.splitn(2, ' ').nth(1).unwrap_or("").trim();
            *PROMPT_STR.lock() = String::from(rest);
            crate::println!("prompt set to '{}'", rest);
        }
        "cpuinfo" => {
            crate::println!("CPU: x86_64");
            crate::println!("model: virtual (VirtualBox)");
            crate::println!("cores: 1 (1P)");
            crate::println!("features: apic, vmx, mmx, sse, sse2");
        }
        "tree" => {
            // 鎵撳嵃 kobject 瀵硅薄鏍戯紙objmgr.md 鍏ㄥ眬瀵硅薄绠＄悊鍣ㄨ鍥撅級
            let root = crate::object::root();
            crate::println!("/");
            dump_children(&root, 1);
        }
        "netstat" => {
            // 缃戠粶 socket 鍒楄〃锛坣et.md socket IO 娴佸璞★級
            let mgr = crate::net::manager();
            let socks = mgr.sockets.lock();
            if socks.is_empty() {
                crate::println!("(no sockets)");
            } else {
                crate::println!("SOCK  TYPE      STATE        LPORT  RPORT  OWNER");
                for (id, s) in socks.iter() {
                    let st = *s.state.lock();
                    let lp = *s.local_port.lock();
                    let rp = *s.remote_port.lock();
                    let op = *s.owner_pid.lock();
                    crate::println!("{:>4}  {:?}  {:?}  {:>5}  {:>5}  {}", id, s.sock_type, st, lp, rp, op);
                }
            }
        }
        "whoami" => {
            // acl.md锛氬綋鍓嶆潈闄愪笂涓嬫枃
            crate::println!("uid=0(root) gid=0(system) context=core_system");
        }
        "fonts" => {
            // font.md锛氬垪鍑哄凡鐧昏瀛椾綋璧勬簮锛坒ont_resmap锛?
            let ids = crate::font_resmap::list();
            if ids.is_empty() {
                crate::println!("(no font resources registered; try 'fontinit')");
            } else {
                for id in &ids {
                    if let Some(r) = crate::font_resmap::lookup(id) {
                        crate::println!("#{} {:<16} {:?}  {}", r.seq, id, r.format, r.path);
                    }
                }
            }
            crate::println!("{} font(s)", ids.len());
        }
        "fontload" => {
            // font.md：字体加载器解析 cmap → 字符映射器构建 字符→glyph 映射
            let id = parts.next().unwrap_or("");
            if id.is_empty() {
                crate::println!("usage: fontload <font_id>");
            } else {
                // font_resmap 按 font_id 精确匹配；fallback 大小写不敏感（用户常输小写）
                let res = crate::font_resmap::lookup(id).or_else(|| {
                    crate::font_resmap::list()
                        .iter()
                        .find(|f| f.eq_ignore_ascii_case(id))
                        .and_then(|f| crate::font_resmap::lookup(f))
                });
                match res {
                    Some(r) => match crate::font_loader::load_font(&r.data) {
                        Ok(f) => {
                            // 字符映射器接入：清空并批量构建 字符→glyph 映射
                            crate::char_mapper::clear();
                            crate::char_mapper::build_from_glyphs(&f.glyphs);
                            crate::println!(
                                "font '{}': family={} format={} upem={} tables={} glyphs={} mapped={}",
                                id,
                                f.info.family,
                                f.info.format.name(),
                                f.info.units_per_em,
                                f.info.num_tables,
                                f.glyphs.len(),
                                crate::char_mapper::mapped_count()
                            );
                        }
                        Err(e) => crate::println!("fontload: {}: {}", id, e),
                    },
                    None => crate::println!("fontload: {}: not found", id),
                }
            }
        }
        "fontinit" => {
            // 鏋勯€犳渶灏?sfnt锛堝惈 head 琛紝upem=1000锛夋敞鍐屼负娴嬭瘯瀛椾綋锛岄獙璇佸姞杞藉櫒
            let mut ttf = [0u8; 48];
            ttf[0..4].copy_from_slice(&[0x00, 0x01, 0x00, 0x00]); // sfnt version
            ttf[5] = 0x01; // numTables = 1
            ttf[12..16].copy_from_slice(b"head");
            ttf[20..24].copy_from_slice(&[0x00, 0x00, 0x00, 0x1C]); // head offset = 28
            ttf[24..28].copy_from_slice(&[0x00, 0x00, 0x00, 0x20]); // head length
            ttf[28 + 18] = 0x03;
            ttf[28 + 19] = 0xE8; // unitsPerEm = 1000
            if crate::font_resmap::register("builtin", "/system/media/builtin.ttf", ttf.to_vec()) {
                crate::println!("registered builtin test font (format=ttf)");
            } else {
                crate::println!("fontinit: registration failed");
            }
        }
        "mountcd" => {
            // fs.md锛氭帰娴?IDE 鍏夐┍锛屾寕杞?ISO9660锛屾壂鎻?/system/media 鍔犺浇瀛椾綋
            let drv = AtapiDrive::new();
            if !drv.is_present() {
                crate::println!("mountcd: no ATAPI drive on IDE primary (0x1F0)");
                return;
            }
            crate::println!("mountcd: ATAPI drive detected, mounting ISO9660...");
            let fsobj = match Iso9660::new(drv) {
                Ok(f) => f,
                Err(e) => {
                    crate::println!("mountcd: iso9660 init failed: {:?}", e);
                    return;
                }
            };
            let media = match fsobj.lookup("/system/media") {
                Ok(d) if d.is_dir => d,
                Ok(_) => {
                    crate::println!("mountcd: /system/media is not a directory");
                    return;
                }
                Err(_) => {
                    crate::println!("mountcd: /system/media not found on ISO");
                    return;
                }
            };
            let entries = match fsobj.read_dir(&media) {
                Ok(e) => e,
                Err(e) => {
                    crate::println!("mountcd: read_dir failed: {:?}", e);
                    return;
                }
            };
            let mut count = 0;
            for ent in entries {
                if ent.is_dir {
                    continue;
                }
                let lower = ent.name.to_lowercase();
                if lower.ends_with(".ttf") || lower.ends_with(".otf") || lower.ends_with(".ttc") {
                    // 超大字体（如 16.5MB 思源黑体）PIO 读取耗时且易卡，跳过交给 loadcjk 按需加载
                    if ent.size > 2 * 1024 * 1024 {
                        crate::println!(
                            "  (skip large font {}: {} B - run 'loadcjk')",
                            ent.name,
                            ent.size
                        );
                        continue;
                    }
                    match fsobj.read_file(&ent) {
                        Ok(bytes) => {
                            let path = alloc::format!("/system/media/{}", ent.name);
                            if crate::font_resmap::register(&ent.name, &path, bytes) {
                                crate::println!(
                                    "  loaded font: {} ({} B)",
                                    ent.name,
                                    ent.size
                                );
                                count += 1;
                            }
                        }
                        Err(e) => {
                            crate::println!("  read {} failed: {:?}", ent.name, e);
                        }
                    }
                }
            }
            crate::println!("mountcd: loaded {} font(s) from ISO /system/media", count);
            *MOUNT.lock() = Some(MountedIso {
                fs: fsobj,
                cwd: "/".into(),
            });
            crate::println!("mountcd: ISO mounted -> use dir / cd / type to browse");
        }
        "loadcjk" => {
            // 中文字体位图字体内嵌到内核（规避 VirtualBox 上 ISO9660 目录读取的字节错误）
            let bytes: &[u8] = include_bytes!("assets/hybrid.cfont");
            let ok = crate::cjk_render::set_font(bytes);
            crate::println!("loadcjk: embedded font {} B cjk_render={}", bytes.len(), ok);
        }
        "zh" => {
            // 中文渲染测试（需先 loadcjk 加载位图字体）
            if crate::cjk_render::ready() {
                crate::println!("你好世界！中文渲染测试。Hello 世界 123。");
                crate::println!(
                    "已加载 {} 个汉字字形",
                    crate::cjk_render::cache_count()
                );
            } else {
                crate::println!("CJK font not loaded - run 'loadcjk' first");
            }
        }
        "fontformats" => {
            // font.api: formats —— 支持的全部公开字体格式（font.md §2）
            use crate::font_loader::FontFormat;
            let fmts = [
                FontFormat::TrueType,
                FontFormat::OpenType,
                FontFormat::Woff,
                FontFormat::Woff2,
                FontFormat::Eot,
                FontFormat::TrueTypeCollection,
                FontFormat::Type1,
                FontFormat::Bitmap,
                FontFormat::Lerf,
            ];
            crate::println!("font.api formats: 支持的公开字体格式（font.md §2）");
            for f in fmts {
                crate::println!("  {:<18} .{}", f.name(), f.ext());
            }
        }
        "fontresmap" => {
            // font.api: resmap —— 完整字体资源映射表（序号/标识/位置/格式）
            let rows = crate::font_resmap::resmap_table();
            crate::println!("resmap: {} 条（#序号 标识 位置 格式）", rows.len());
            for r in rows {
                crate::println!("  #{} {} {} {}", r.seq, r.font_id, r.path, r.format.name());
            }
        }
        "charmap" => {
            // 查看当前字体全部字符（font.md §1：打开字符映射器查看全部字符）
            let list = crate::char_mapper::all_chars();
            crate::println!("字符映射器: 共 {} 个字符", list.len());
            let start = parts.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
            let end = core::cmp::min(start + 64, list.len());
            let mut line = alloc::string::String::new();
            for i in start..end {
                let (_cp, ch) = list[i];
                line.push(ch);
                line.push(' ');
                if (i - start + 1) % 8 == 0 {
                    crate::println!("{}", line);
                    line.clear();
                }
            }
            if !line.is_empty() {
                crate::println!("{}", line);
            }
            if end < list.len() {
                crate::println!("... 下一页: charmap {}", end);
            }
        }
        "charcopy" => {
            // 选中单个字符并复制（font.md §1：选中字符后点击复制）
            if let Some(arg) = parts.next() {
                let cp = u32::from_str_radix(arg.trim_start_matches("0x"), 16).unwrap_or(0);
                match crate::char_mapper::select_char(cp) {
                    Some(ch) => crate::println!("已选中字符: {} (U+{:04X})", ch, cp),
                    None => crate::println!("无效码点: {}", arg),
                }
            } else {
                crate::println!("用法: charcopy <十六进制码点，如 4E2D>");
            }
        }
        "measure" => {
            // 文本测量（font.md §6.2）
            let mut text = alloc::string::String::new();
            for w in parts {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(w);
            }
            crate::println!(
                "文本宽度: {} px（{} 字符）",
                crate::char_mapper::text_width(&text),
                text.chars().count()
            );
        }
        "shutdown" | "poweroff" | "exit" => {            // 鍏虫満锛氬皾璇曞绉?ACPI PM 绔彛涓?SLP 鍊硷紙VirtualBox PM1a_CNT=0x400锛孮EMU=0x604锛?
            crate::println!("powering off...");
            unsafe {
                for (port, val) in [
                    (0x400u16, 0x3C00u32), // PM1a_CNT SLP_TYP=7 + SLP_EN (VirtualBox)
                    (0x400, 0x3400u32),    // SLP_TYP=5 + SLP_EN
                    (0x404, 0x3C00u32),    // 澶囬€?PM1a_CNT (EVT+4)
                    (0x404, 0x3400u32),
                    (0x400, 0x3C01u32),    // 鍚?SCI_EN
                    (0x604, 0x2000u32),    // QEMU ACPI 鐢垫簮鎸夐挳
                    (0x600, 0x2000u32),
                ] {
                    core::arch::asm!("out dx, eax", in("dx") port, in("eax") val, options(nomem, nostack));
                }
            }
            loop {
                unsafe { core::arch::asm!("hlt") };
            }
        }
        "acl" => {
            crate::println!("ACL subsystem (acl.md): kernel object entry permission control");
            crate::println!("mask bits: CREATE_ENTRY/DELETE_ENTRY/ENTRY_READ/ENTRY_WRITE/EXEC/OWNER");
        }
        "sysobj" => {
            // 鏌ヨ浠绘剰瀛愮郴缁熸帶鍒跺璞＄殑鏉＄洰鐘舵€侊紙楠岃瘉 specs 瀵硅薄-鏉＄洰濂戠害锛?
            let subsys = parts.next().unwrap_or("");
            if subsys.is_empty() {
                crate::println!("usage: sysobj <subsys>");
                crate::println!("subsys: storage security service power backup pkg update filemgr cmd devmgr audio dev");
            } else {
                let obj = crate::object::lookup(&format!("/object/{}/mgr", subsys))
                    .or_else(|_| crate::object::lookup(&format!("/object/{}", subsys)));
                match obj {
                    Ok(o) => {
                        let entries = o.entries.lock();
                        if entries.is_empty() {
                            crate::println!("(no entries)");
                        }
                        for (name, e) in entries.iter() {
                            let val = e.data.lock().clone().unwrap_or_default();
                            let s = core::str::from_utf8(&val).unwrap_or("<bin>");
                            crate::println!("  {:<12} = {}", name, s);
                        }
                        crate::println!("{} entry(s)", entries.len());
                    }
                    Err(_) => crate::println!("sysobj: {}: object not found", subsys),
                }
            }
        }
        _ => {
            crate::println!("unknown command: '{}' (try 'help')", name);
        }
    }
}

/// 閫掑綊鎵撳嵃 kobject 鏍?
fn dump_children(obj: &object::KObject, depth: usize) {
    let children = obj.children.lock();
    let mut paths: Vec<(String, alloc::sync::Arc<object::KObject>)> = Vec::new();
    for (n, child) in children.iter() {
        let mut path = String::new();
        if obj.path == "/" {
            path.push('/');
            path.push_str(n);
        } else {
            path.push_str(&obj.path);
            path.push('/');
            path.push_str(n);
        }
        paths.push((path, child.clone()));
    }
    drop(children);
    for (path, child) in paths {
        crate::println!("{}{}", "  ".repeat(depth), path);
        dump_children(&child, depth + 1);
    }
}

/// 在已挂载光盘上解析相对/绝对路径（支持 . 和 ..）
fn resolve_iso_path(arg: &str, cwd: &str) -> String {
    let full = if arg.starts_with('/') {
        String::from(arg)
    } else if arg == "." {
        String::from(cwd)
    } else if cwd == "/" {
        format!("/{}", arg)
    } else {
        format!("{}/{}", cwd, arg)
    };
    let mut stack: Vec<&str> = Vec::new();
    for c in full.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            other => stack.push(other),
        }
    }
    let joined = stack.join("/");
    if joined.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", joined)
    }
}

/// 打印 shell 提示符（prompt 命令可自定义）
fn print_prompt() {
    let p = PROMPT_STR.lock();
    if p.is_empty() {
        crate::vga::put_str("root@hybrid:~# ");
    } else {
        crate::vga::put_str(&p);
        crate::vga::put_char(b' ');
    }
}

/// 读 CMOS RTC 寄存器
fn cmos_read(reg: u8) -> u8 {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") 0x70u16, in("al") reg, options(nomem, nostack, preserves_flags));
        let mut v: u8;
        core::arch::asm!("in al, dx", lateout("al") v, in("dx") 0x71u16, options(nomem, nostack, preserves_flags));
        v
    }
}

fn bcd(v: u8) -> u8 {
    (v & 0x0F) + ((v >> 4) * 10)
}

fn rtc_time() -> (u8, u8, u8) {
    (bcd(cmos_read(0x04)), bcd(cmos_read(0x02)), bcd(cmos_read(0x00)))
}

fn rtc_date() -> (u16, u8, u8) {
    let year = bcd(cmos_read(0x09)) as u16 + 2000;
    (year, bcd(cmos_read(0x08)), bcd(cmos_read(0x07)))
}

/// 触发重启：ACPI 复位端口 + PS/2 控制器复位 + 三连故障兜底
fn triple_fault() -> ! {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") 0x64u16, in("al") 0xFEu8, options(nomem, nostack));
        // 加载非法 IDT 基址触发 triple fault
        core::arch::asm!("xor eax, eax; lidt [rax]", options(nostack));
        loop {
            core::arch::asm!("hlt");
        }
    }
}

pub fn acltest_selfcheck() {
    use crate::acl::AclMask;
    let obj = match crate::object::create_object("/object/acltest") {
        Ok(o) => o,
        Err(_) => match crate::object::lookup("/object/acltest") { Ok(o) => o, Err(_) => return },
    };
    let _ = obj.create_entry("probe", AclMask::default_system());
    let _ = obj.write_entry("probe", b"seed", AclMask::default_system());
    let r_empty = obj.read_entry("probe", AclMask::empty());
    let r_rd = obj.read_entry("probe", AclMask::ENTRY_READ);
    let r_wr = obj.write_entry("probe", b"x", AclMask::ENTRY_READ);
    crate::println!("[acltest] empty read => {}", if r_empty.is_err() { "EACCES(deny) OK" } else { "ALLOWED BAD" });
    crate::println!("[acltest] read-only read => {}", if r_rd.is_ok() { "OK" } else { "DENIED BAD" });
    crate::println!("[acltest] read-only write => {}", if r_wr.is_err() { "EACCES(deny) OK" } else { "ALLOWED BAD" });
    let pass = r_empty.is_err() && r_rd.is_ok() && r_wr.is_err();
    crate::println!("[acltest] {}", if pass { "PASS" } else { "FAIL" });
}
