# rtc.md 时间与时钟规范

> 
> 前提说明
> 
> 1. 负责系统时间体系：硬件 RTC（实时时钟）、内核时钟、墙上时钟（wall clock）与单调时钟（monotonic）。
> 2. 开机时从 RTC 读取时间初始化内核时钟（见 `boot.md`）；运行时由时钟中断维护。
> 3. 提供时区管理与时间同步（NTP）；改时间 / 改 RTC 属敏感操作。
> 4. 定时器（一次性 / 周期）基于单调时钟实现，供 POSIX `timer` / `sleep` 等翻译（见 `Linux.md`）。
> 5. 时间源优先级：NTP 同步 > RTC > 出厂默认；不可信时间源不用于安全校验。

## 1 总体约束

1. 两类时钟：
   - **墙上时钟**：可被用户 / NTP 调整，用于显示与文件时间戳；
   - **单调时钟**：只增不减，用于计时、超时与性能测量，不受调时影响。
2. 时间戳统一使用自系统纪元起的纳秒计数（墙上）与单调纳秒计数（单调）。
3. RTC 保存墙上时钟（UTC），时区仅影响显示层，不写入 RTC。
4. 改系统时间 / RTC 仅 `aux_system` 可调用（认证与审计需要可信时间）。
5. NTP 客户端随网络可用性工作，同步后广播时间变更事件。

## 2 时间对象条目布局

### 时钟对象，路径 `/object/rtc/clock`

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `wall` | 只读 | 当前墙上时钟（纳秒） |
| `mono` | 只读 | 当前单调时钟（纳秒） |
| `tz` | 只读 | 当前时区 |
| `rtc` | 只读 | RTC 原始读数（UTC） |
| `set_wall` | 只写 | 设置墙上时钟（aux_system） |
| `set_rtc` | 只写 | 设置 RTC（aux_system） |
| `ntp_status` | 只读 | NTP 同步状态 / 最近同步时间 / 偏移 |

### 定时器

- 定时器对象路径 `/object/rtc/timer/<id>`：一次性 / 周期，到期投递事件；经 POSIX 翻译供 Linux 程序使用。

## 3 核心操作流程示例

### 示例 1：开机时间初始化

1. 内核读取 RTC（UTC）初始化墙上时钟，单调时钟从 0 起；
2. NTP 可用后同步校准（偏移超过阈值则逐步调整）；
3. 同步完成广播 `time_changed`。

### 示例 2：应用创建定时器

1. 应用申请定时器（一次性 / 周期），指定到期时间（单调）；
2. 到期内核投递到期事件；
3. 应用处理后释放定时器。

## 4 和其它文档对接关系

1. **对接 boot.md**：开机流程从 RTC 初始化系统时间。
2. **对接 kernel.md**：时钟中断服务内核时钟与调度。
3. **对接 net.md**：NTP 报文经网络栈收发。
4. **对接 config.md**：时区与时间格式在系统配置表中维护（见 `locale.md`）。
5. **对接 Linux.md**：POSIX `clock_gettime` / `timer_create` / `nanosleep` 经本规范翻译。

## 5 设计取舍

1. 墙上与单调分离：调时不影响计时与超时语义，避免应用时间跳变。
2. RTC 只存 UTC：时区纯显示层，切换时区不动硬件。
3. NTP 逐步调整而非跳变：避免日志时间戳混乱。

## 6 时间 `.api` 契约文件（rtc.api 全文）

时间子系统对外接口统一由以下契约文件声明，文件存放于 `/system/api/rtc.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# rtc.api 时间与时钟对外接口契约
version = 1.0
target = rtc
env = python
exec = /system/kernel/rtc
internal_loc = map + symbol + register
min_os_version = 1.0
global_acl = user_foreground
global_limit = 200次/秒

object /system/api/rtc rtc

# ---- 只读条目 ----
rtc: time_get { } /internal/rtc/wall
rtc: mono_get { } /internal/rtc/mono
rtc: rtc_read { } /internal/rtc/rtc
rtc: tz_get { } /internal/rtc/tz
rtc: ntp_status { } /internal/rtc/ntp

# ---- 可写控制条目 ----
rtc: time_set {
    return self.time_set(ns)
} /internal/rtc/wall_set
rtc: rtc_set {
    return self.rtc_set(ns)
} /internal/rtc/rtc_set
rtc: tz_set {
    return self.tz_set(tz)
} /internal/rtc/tz_set
rtc: ntp_sync {
    return self.ntp_sync()
} /internal/rtc/ntp_sync

# ---- 对外可监听标准事件 ----
EVENT:
    time_changed(kind: str) ｜ /internal/event/rtc_time
    ntp_synced(offset: int) ｜ /internal/event/rtc_ntp
    timer_expired(id: str) ｜ /internal/event/rtc_timer

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    time_set: aux_system
    rtc_set: aux_system
```

契约要点：

1. 读时间（`time_get` / `mono_get`）开放给普通应用；改时间 / 改 RTC 仅 `aux_system`。
2. `tz_set` 面向前台用户，仅改显示层，不动 RTC。
3. `timer_expired` 事件供定时器消费者（含 Linux 兼容层翻译）感知到期。
4. `ntp_sync` 触发即时同步，同步结果经 `ntp_synced` 广播。
