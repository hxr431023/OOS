# input.md 输入子系统规范

> 
> 前提说明
> 
> 1. 负责键盘 / 鼠标 / 触摸屏 / 触摸板 / 手柄等输入设备的**事件采集、归一化、焦点分发与注入**。
> 2. 设备枚举与服务加载由 `devmgr.md` / `hardware_svc.md` / `external_dev.md` 负责；本规范只处理事件链与分发。
> 3. 输入事件按 **前台会话焦点窗口** 分发；焦点由窗口合成器（`ui.md`）维护，输入子系统只消费焦点表。
> 4. 支持**输入注入**（自动化 / 远程控制 / 无障碍），注入为敏感操作，仅系统级可调用。
> 5. 输入法（IME）作为输入源接入事件链，不改变设备事件模型。

## 1 总体约束

1. 事件链：`输入设备 → 服务（service）→ 输入核心 → 归一化事件 → 焦点窗口`，全链路在内核 + 输入服务协作完成。
2. 归一化事件统一为结构化事件（类型 / 坐标 / 键码 / 时间戳 / 设备来源）。
3. 焦点窗口不存在的输入事件（如全局热键）由输入核心按全局绑定表处理。
4. 注入事件与真实事件在事件流中不可区分（对接收方透明），但事件元数据标记来源（injected / hardware）。
5. 输入核心维护各设备状态（按键状态、指针位置），供查询与复用。

## 2 输入对象条目布局

### 输入核心对象，路径 `/object/input/core`

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `devices` | 只读 | 已注册输入设备列表 |
| `pointer` | 只读 | 当前指针位置与按钮状态 |
| `keymap` | 可读可写 | 键码 → 键值映射（布局相关，见 `locale.md`） |
| `grab` | 可读可写 | 独占抓取：某窗口独占一段时间的输入流 |
| `inject` | 只写 | 注入输入事件（仅 aux_system） |

### 输入法（IME）接入

- IME 以独立输入源注册进事件链：原始按键先经 IME 处理，产出候选串 / 提交文本作为"合成事件"继续分发。
- IME 状态（激活 / 候选窗口）由输入法服务维护，候选窗口由 `ui.md` 图层承载。

## 3 核心操作流程示例

### 示例 1：按键分发

1. 键盘服务上报按键 → 输入核心；
2. 输入核心按 keymap 归一化为键值事件；
3. 若焦点窗口存在且未抓取 → 投递到焦点窗口；
4. 若是全局热键 → 由全局绑定表处理，不投递窗口。

### 示例 2：输入注入（自动化）

1. 调用方请求注入（需 aux_system）；
2. 输入核心将注入事件插入事件流，标记来源 injected；
3. 焦点窗口按普通事件处理。

## 4 和其它文档对接关系

1. **对接 devmgr.md / hardware_svc.md**：输入设备服务注册上报通道。
2. **对接 external_dev.md**：键盘 / 鼠标 / 触摸等标准外设的枚举与能力。
3. **对接 ui.md**：焦点表由窗口合成器维护，事件按焦点分发；锁屏时输入仅放行解锁通道。
4. **对接 session.md**：输入按前台会话分发，锁屏期间挂起普通输入。
5. **对接 locale.md**：键码映射、键盘布局由本地化配置决定。

## 5 设计取舍

1. 事件链统一为一种结构化事件，不区分设备类型，降低分发复杂度。
2. 注入与真实事件同流但带来源标记：兼顾自动化能力与审计。
3. 焦点之外用全局热键兜底，保证系统级快捷键始终可用。

## 6 输入 `.api` 契约文件（input.api 全文）

输入子系统对外接口统一由以下契约文件声明，文件存放于 `/system/api/input.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# input.api 输入子系统对外接口契约
version = 1.0
target = input
env = python
exec = /system/input/core
internal_loc = map + symbol + register
min_os_version = 1.0
global_acl = user_foreground
global_limit = 200次/秒

object /system/api/input input

# ---- 只读条目 ----
input: input_devices { } /internal/input/devices
input: pointer_pos { } /internal/input/pointer
input: key_state { } /internal/input/keys

# ---- 可写控制条目：抓取 / 注入 / IME 接入 ----
input: input_grab {
    return self.input_grab(win, dev_mask)
} /internal/input/grab
input: input_release {
    return self.input_release(win)
} /internal/input/release
input: input_inject {
    return self.input_inject(ev)
} /internal/input/inject
input: ime_attach {
    return self.ime_attach(ime_id)
} /internal/input/ime
input: ime_detach {
    return self.ime_detach()
} /internal/input/ime_detach

# ---- 对外可监听标准事件 ----
EVENT:
    input_event(ev: str) ｜ /internal/event/input
    input_device_added(dev: str) ｜ /internal/event/input_add
    input_device_removed(dev: str) ｜ /internal/event/input_remove
    focus_changed(win: str) ｜ /internal/event/input_focus

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    input_inject: aux_system
    input_grab: user_foreground
```

契约要点：

1. `input_inject` 输入注入属敏感操作，仅 `aux_system` 可调用（自动化 / 无障碍 / 远程控制）。
2. `input_grab` 独占抓取面向当前前台窗口，抓取期间事件不再分发给其他窗口。
3. IME 经 `ime_attach` 接入事件链，事件类型不变，仅改变分发路径。
4. `input_event` 为高频事件流，供程序自动化与无障碍监听。
