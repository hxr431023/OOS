# srjg.md 语言与工具链选型规范

## 1 定位

本文档定义自研操作系统的**实现语言分工原则**与语言运行时 / 工具链的登记契约。全系统代码按场景选择最合适的语言，底层关键路径保证性能与内存安全。

## 2 语言分工原则

1. **能用 Go 的地方用 Go**：系统服务、守护进程、管理工具、网络服务等并发 / 网络密集逻辑，优先使用 Go。
2. **能用 Java 的地方用 Java**：上层应用、桌面 / GUI 业务、跨平台服务等偏业务逻辑场景，优先使用 Java。
3. **底层用 Rust**：内核、服务、硬件访问、VFS、内存管理等底层系统代码，使用 Rust 保证内存安全与确定性性能。
4. **要用汇编的时候用汇编**：启动引导、上下文切换、中断入口、CPU 特性探测等必须直接操作寄存器的场景，使用汇编（`asm.md`）。

## 3 语言运行时 / 工具链

1. 各语言运行时 / 工具链作为系统组件登记，统一经 `srjg.api` 契约注册与查询。
2. 运行时遵循 `pkg.md` 安装与卸载规则；系统自带语言运行时为系统核心组件，不可随意卸载（见 `pkg.md`）。

## 4 语言与工具链 `.api` 契约文件（srjg.api 全文）

语言与工具链对外接口统一由以下契约文件声明，文件存放于 `/system/api/srjg.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# srjg.api 语言与工具链选型对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = srjg
env = python
exec = /system/SystemApps
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 100次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/srjg srjg

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
srjg: langs { } /internal/srjg/langs
srjg: toolchain { } /internal/srjg/toolchain

# ---- 可写控制条目：转换代码为Python，实现语言查询与登记 ----
srjg: select {
    return self.select(task)
} /internal/srjg/select
srjg: register_lang {
    return self.register_lang(lang, role)
} /internal/srjg/register

# ---- 对外可监听标准事件（保持原规范） ----
EVENT:
    lang_registered(lang: str) ｜ /internal/event/srjg_reg
```

契约要点：

1. `select` 按任务场景返回推荐语言（Go / Java / Rust / 汇编）；`register_lang` 登记语言运行时到系统。
2. 已登记语言与工具链信息只读透传。
3. 语言运行时登记事件供系统组件清单统计。