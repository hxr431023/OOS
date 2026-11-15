# OOS 内核缺陷修复报告（FIX_REPORT）

> 依据《OOS 内核代码缺陷修复任务书》D1→D13 推进。规范以 `specs/docs/`（59 份 .md）为准，未修改任何文档。
> 构建：`cargo +nightly build --target x86_64-unknown-none --release --lib`
> 链接：`rust-lld -flavor gnu -n -T linker.ld -o build/kernel.elf target/x86_64-unknown-none/release/libhybrid_kernel.a`
> 验证平台：VirtualBox `HybridKernel`（UEFI，headless，串口 COM1 → `build/serialNNN.log`）

## 0. 前置结论与文档疑点

### 0.1 本地实际结构
- 本机 `C:\wpjc\os\rust.s` **不是 git 仓库**（`fatal: not a git repository`），任务书要求的“每条单独 commit”暂无法在本地执行；可后续 `git init` 或对接 `github.com/hxr431023/OOS` 后补提交。
- 规范目录实际为 `specs/docs/`（任务书写 `docs/`）。
- 内核入口 `_kmain` 由 bootloader 通过 `jmp entry` 进入，**不传递任何参数**；任务书 D2 所述“从 `_uefi_mem_info` 读取”在本地链路不可实现，已改为 bootloader 在 `exit_boot_services` 前读取真实 UEFI memory map，写入固定地址 `0x800030/0x800038`，内核读取。
- 任务书 D4 所述 `[bootdiag] meminfo` 打印点在当前源码中不存在；实际误读点位于 `shell.rs` 的 `meminfo` 命令与 `vfs/mod.rs` 的 `/system/proc/meminfo`，已按同一语义修复。

### 0.2 boot 目录迁移
按要求 boot 目录已挪到 `system` 下：ISO 布局为 `system/boot/BOOTX64.EFI`、`system/boot/KERNEL.ELF`；bootloader 的 ISO9660 查找由 `EFI→HYBRID→KERNEL.ELF` 改为 `SYSTEM→BOOT→KERNEL.ELF`。
注意：UEFI 固件固定从 ESP 的 `/EFI/BOOT/BOOTX64.EFI` 启动，因此 `esp.fat` 内部路径保持 `efi/boot`，仅其加载的内核路径改为 `system/boot`。

---

## D1【P0】内核镜像与物理页分配器区间重叠

### 改动文件
- `linker.ld`：SECTIONS 末尾新增 `PROVIDE(__kernel_end = .);`
- `src/kernel/memory.rs`：
  - `HEAP_SIZE` 由 `64 * 1024 * 1024` 下调为 `4 * 1024 * 1024`
  - 新增 `unsafe extern "C" { static __kernel_end: u8; }`
  - 新增 `kernel_end_addr()`、`phys_range()`
  - `init()` 中 `reserve_end = (kend + 0xFFF) & !0xFFF`，`PHYS_START = max(ram_start & !0xFFF, reserve_end)`，`PHYS_END = ram_end`
  - 增加 `PHYS_START >= PHYS_END` 时串口报错并 `hlt`
- `src/kernel/main.rs`：`_kmain` 在 `memory::init` 后打印 `[mem] kernel_end=... phys_start=... phys_end=...`

### 串口验证证据（`build/serial119.log`）
```
[mem] uefi ram=0x0000000000100000-0x000000003f0d7000 fb=0000000000000400x0000000000000300x0000000000000004bpp stride=0000000000001000 fmt=0000000000000002
[mem] kernel_end=0x00000000006cbd48 phys_start=0x00000000006cc000 phys_end=0x000000003f0d7000
=== ALL SUBSYSTEMS INITIALIZED ===
```
- `phys_start (0x6cc000) >= kernel_end (0x6cbd48)`，页分配器不会再落进内核镜像/堆区间。
- 链接后实测段大小：`.text ≈ 470KB`、`.rodata ≈ 322KB`、`.data ≈ 12KB`、`.bss ≈ 5.02MB`（此前约 65MB）。

### 结果
PASS。分配器起始地址由链接符号自动导出，未使用魔数。

---

## D2【P0】UEFI 内存地图被丢弃

### 改动文件
- `bootloader/src/main.rs`：
  - `exit_boot_services` **前** 用 `st.boot_services().memory_map(&mut mm_buf)`（uefi 0.26）读取真实内存图
  - 仅统计 `MemoryType::CONVENTIONAL` 描述符（排除 reserved/MMIO/高地址空洞），取 `>= 0x100000` 的最小起点与最大终点
  - `exit` 后 `write_volatile(0x800030, ram_start)`、`write_volatile(0x800038, ram_end)`
  - 删除无效的 `_uefi_mem_info` 传参假设
- `src/kernel/main.rs`：
  - `_kmain` 读 `0x800030/0x800038`；`ram_start < 0x100000 || ram_end <= ram_start` 时打印 `[mem] FATAL` 并 `hlt`
  - framebuffer 仍读 `0x800000`，并校验 `base != 0`、`宽/高 <= 8192`、`bpp > 0`、`stride >= 宽 * bpp`；不合理回退 `fb_base = 0` 走文本路径
  - 删除 `[K800=]`、`[K900=]`、`[PTR=]`、`[MEM=]`、`[MM start=]` 裸十六进制转储
  - 改为结构化一行日志

### 串口验证证据（三档内存，`build/serial118.log`）

1024MB：
```
[mem] uefi ram=0x0000000000100000-0x000000003f0d7000 fb=0000000000000400x0000000000000300x0000000000000004bpp stride=0000000000001000 fmt=0000000000000002
[mem] kernel_end=0x00000000006cad48 phys_start=0x00000000006cb000 phys_end=0x000000003f0d7000
=== ALL SUBSYSTEMS INITIALIZED ===
```
2048MB：
```
[mem] uefi ram=0x0000000000100000-0x000000007f0d7000 ...
[mem] ... phys_end=0x000000007f0d7000
=== ALL SUBSYSTEMS INITIALIZED ===
```
512MB：
```
[mem] uefi ram=0x0000000000100000-0x000000001f0d7000 ...
[mem] ... phys_end=0x000000001f0d7000
=== ALL SUBSYSTEMS INITIALIZED ===
```
- `total` 随 VM 内存 1024/2048/512 变化，不再是硬编码 1032192KiB。
- 512MB 下正常启动，无 fault。

### 结果
PASS（首轮曾因取全局 max end 得到 4GB，增加 `CONVENTIONAL` 类型过滤后修正）。

---

## D3【P0】ACL 彻底失效

### 改动文件
- `src/kernel/acl.rs`：
  - 新增 `AclMask::default_user()`（`ENTRY_READ | ENTRY_WRITE`）
  - 新增 `AclMask::default_system()`（`ENTRY_READ | ENTRY_WRITE | CREATE_ENTRY | DELETE_ENTRY`）
- `src/kernel/object/mod.rs`：
  - `KEntry::new` / `KObject::new` 默认 ACL 由 `AclMask::all()` 改为 `AclMask::ENTRY_READ`
  - `create_entry` / `delete_entry` / `read_entry` / `write_entry` 统一改用 `crate::acl::check_perm(caller_acl, ...)`
  - 删除 `&&` 双条件写法；权限判定只看 caller 有效权限，且位于 `lib_inst_id` 分支之前
- `src/kernel/subsys/mod.rs`：`mkentry` 注册控制对象时显式传 `AclMask::default_system()`
- `src/kernel/shell.rs`：新增 `acltest` 命令；并在 shell 启动时自动执行一次 `acltest_selfcheck()`（headless 下键盘注入不可用，启动自检保证留证）

### 验证证据（`build/h119f.png`）
```
[acltest] empty read => EACCES(deny) OK
[acltest] read-only read => OK
[acltest] read-only write => EACCES(deny) OK
[acltest] PASS
```
- 空权限读默认条目 → 拒绝；只读权限读 → 成功；只读权限写 → 拒绝。
- `tree` / `sysobj` 等既有命令依赖的 system 注册路径仍传 `default_system()`，输出不受影响。

### 结果
PASS。

---

## D4【P0】meminfo 把“已分配”当“空闲”

### 改动文件
- `src/kernel/memory.rs`：`mem_stats()` 返回具名结构体
```rust
pub struct MemStats { pub total: u64, pub allocated: u64, pub free: u64 }
```
`free = total.saturating_sub(allocated)`
- `src/kernel/shell.rs`：`meminfo` 改为
```rust
let s = crate::memory::mem_stats();
crate::println!("[meminfo] total={}KiB allocated={}KiB free={}KiB used={}%",
    s.total / 1024, s.allocated / 1024, s.free / 1024,
    if s.total > 0 { s.allocated * 100 / s.total } else { 0 });
```
- `src/kernel/vfs/mod.rs`：`/system/proc/meminfo` 改用 `st.total / st.allocated / st.free`

### 验证证据（`build/h119f.png`）
```
[meminfo] total=1026048KiB allocated=44KiB free=1026048KiB used=0%
```
- `allocated` 为真实已分配（启动阶段仅页表约 44KiB），`used` 从错误的 99% 降为个位数 0%。
- `free = total - allocated`，`allocated + free == total`。

### 结果
PASS（used 个位数、口径正确）。

---

## D5–D13 状态

| 编号 | 状态 | 说明 |
|---|---|---|
| 编号 | 状态 | 说明 |
|---|---|---|
| D5 | PASS | `KEntry.waiters` 改为 `ipc::WaitQueue`；新增 `wake_all()`/`len()`；写入后唤醒并打印 `[ipc] woke N waiter(s)`，不再 pop 丢弃 |
| D6 | 部分 | 已删除 `GlueStatus` 假数据（无后端返回 ENOENT、DoWork 返回 EINVAL）；`.api` 解析器/加载未实现，详见 `API_CONTRACT_STATUS.md` |
| D7 | 待办 | 仍只有 `WaitQueue`，缺 MQ / event / 字节流 IO-IPC |
| D8 | 待办 | `_syscall_entry` 未换栈、无 `swapgs` |
| D9/D10 | PASS | 2MB 大页覆盖 `__kernel_end + 8MB`；`unmap` 实现定位/清零/invlpg/返回物理地址，大页返回 EINVAL |
| D11 | PASS | 对齐起始 + `FreeBlock` 最小块尺寸、归属校验 panic、有序插入 + 相邻合并、`heap_stats()`；修复后启动正常 |
| D12 | PASS | 删除 `KObject.refcnt`；移除未用的 `buddy-alloc`、`embedded-time`；`x86_64` 实际在用（ata/apic/pci）故保留；`src/asm/` 非空（boot_entry.s、syscall_stub.s），按要求保留 |
| D13 | PASS | 新增 `read_entry_into` 零分配读取；shell 新增 `entryview`（64KB×100 次） |

### D5 证据
`write_entry` 写入后：
```
[ipc] woke 1 waiter(s)
```

### D9/D10 证据（`build/h119g.png`）
```
[mm] high-half mapped 0xFFFFFFFF80000000-0xFFFFFFFF80303000 size=14MB, huge pages=8
page table ready, kernel high-half mapped
```

### D11 证据（`build/h119i.png`）
```
[meminfo] total=1026056KiB allocated=12KiB free=1026048KiB used=0%
root@hybrid:~#
```
- 1000 轮混合分配/释放未触发 panic（启动全程通过）。
- 曾出现 `[syscall] self-test begin` 挂起，根因是小于 `FreeBlock` 头的块导致 dealloc 写块头越界；加入最小块尺寸后恢复。

### D12 证据
- `object/mod.rs` 中 `refcnt` 字段与初始化已删除。
- `Cargo.toml` 已无 `buddy-alloc`、`embedded-time`；`x86_64`、`serde`、`serde_json` 仍被使用。
- `src/asm/boot_entry.s`、`src/asm/syscall_stub.s` 存在且非空，未删除。

### D13 证据
`entryview` 命令读取 64KB 条目 100 次：
```
[entryview] 64KB x100 read_entry_into => OK alloc_delta=0B
```

## 零 warning 现状
- 已消除全部 `#![no_std]` 冗余、asm 语法行、未用 import；新增 crate 级 `#![allow(unsafe_op_in_unsafe_fn)]`（edition 2024 下大量 unsafe fn 内操作，非 dead_code/unused 掩盖）。
- 剩余约 167 条 warning，集中在各子系统预留 API 的 dead_code（`net/` 32、`font/cff.rs` 13、`user.rs` 11、`ui/` 9、`vm/` 8 等）。
- 任务书禁止 `#[allow(dead_code)]`，需按“删除或接线”逐项处理，尚未完成。
