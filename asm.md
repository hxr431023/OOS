# asm.md 汇编工具链子系统规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；汇编工具链为**用户态工具集**，运行于 Ring3，供系统构建、引导、底层开发使用。
> 2. Ring1/Ring2 闲置；工具调用受 `acl.md` 权限管控。
> 3. **条目不在此文档硬编码**：汇编工具链对外条目的集合、命名、权限由 `asm.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义汇编工具链的行为语义，契约全文见第 7 节。
> 4. 对应目录树 `/system/asm/`：`asm_tools`（汇编器、链接器、反汇编器工具）、`src`（成品系统为空，不存放汇编源码）、`bin`（汇编工具链产出二进制）、`config`（汇编编译架构配置）。
> 5. 语言选型遵循 `srjg.md`：底层用 Rust、要用汇编的时候用汇编；本工具链为汇编语言开发提供编译支撑。
> 6. 依赖：`os.md`、`acl.md`、`fs.md`、`process.md`、`log.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 汇编器 | 将汇编源码汇编为目标文件（.o） |
| 链接器 | 将多个目标文件链接为可执行文件/内核镜像 |
| 反汇编器 | 将二进制反汇编为汇编文本，用于逆向与调试 |
| 目标架构 | 编译目标 CPU 架构，由 `config` 配置（如 x86_64） |

## 1 定位与职责

汇编工具链（`/system/asm/asm_tools`）承担：

1. 汇编：汇编源码 → 目标文件；
2. 链接：目标文件 → 可执行文件 / 内核镜像；
3. 反汇编：二进制 → 汇编文本（调试、逆向）；
4. 产出管理：`bin` 存放产出二进制，`src` 不存放源码（成品系统为空）。

## 2 工具链工作流程

1. **汇编**：调用汇编器，指定源码、目标架构（`config` 配置）、输出路径；产出目标文件。
2. **链接**：调用链接器，指定目标文件列表、链接脚本、输出路径；产出可执行/内核镜像。
3. **反汇编**：调用反汇编器，指定二进制与架构；输出汇编文本。
4. 工具以子进程方式运行（`process.md`），结果写入 `bin` 目录，操作写入审计日志（`log.md`）。

## 3 与构建流程协同

1. 系统/内核构建、引导程序开发、底层库汇编均依赖本工具链。
2. 汇编产出可交付内核模块（`kernel.md`）或启动引导（`boot.md`）。
3. 目标架构配置统一存放 `config`，多架构并行构建受 ACL 管控。

## 4 与各子系统对接

1. **os.md**：汇编工具链为系统开发基础设施，复用对象‑条目模型。
2. **acl.md**：工具调用、产出写入受 ACL 管控。
3. **fs.md**：源码/产出遵循 `/system/asm` 目录树。
4. **process.md**：工具以子进程运行，遵循进程生命周期。
5. **log.md**：构建操作写入审计日志。
6. **srjg.md**：汇编语言使用原则（要用汇编的时候用汇编）。
7. **.api.md**：汇编工具链对外接口经 `asm.api` 契约与桥接层。
8. **dllo.md**：导出符号表与重定位按 dllo.md 加载语义生成。

## 5 汇编工具链 `.api` 契约文件（asm.api 全文）

汇编工具链对外接口统一由以下契约文件声明，文件存放于 `/system/api/asm.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# asm.api 汇编工具链子系统对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = asm
env = python
exec = /system/asm/asm_tools
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = aux_system
# 全局默认调用限流规则
global_limit = 50次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/asm asm

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
asm: info { } /internal/asm/info
asm: arch_config { } /internal/asm/config

# ---- 可写控制条目：转换代码为Python，实现构建操作 ----
asm: assemble {
    self.assemble(src, arch, out)
} /internal/asm/assemble
asm: link { self.link(objects, out, script) } /internal/asm/link
asm: disassemble { return self.disassemble(bin, arch) } /internal/asm/disasm

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    asm_done(job_id: u64, out: str) ｜ /internal/event/asm_done
    asm_failed(job_id: u64, reason: str) ｜ /internal/event/asm_fail

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    assemble: 50次/秒
    link: 50次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    assemble: aux_system
    link: aux_system
```

契约要点：

1. 外部程序仅能访问 `/system/api/asm` 标准虚拟对象，禁止直连工具链内部实现。
2. 汇编/链接操作授权 `aux_system` 分类进程调用。
3. 事件仅封装契约声明的标准参数；构建完成/失败事件供构建系统监听。
4. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。

## 6 设计取舍

1. 汇编工具链为独立用户态工具集，崩溃不影响系统运行。
2. 工具调用统一走对象条目与 ACL，构建操作可审计。
3. 多架构配置集中管理，支持交叉构建。
4. 对外只暴露 `/system/api/asm` 标准接口，内部工具实现完全隐藏。
