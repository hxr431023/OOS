# time.md 时间应用规范（x86‑64）

> 
> 前提说明
> 
> 
> 1. 完全基于 `os.md` 内核对象‑条目模型；时间应用为**系统内置 GUI 应用**，运行于 Ring3，对应 Windows 10 任务栏左下角时钟：点击时间即打开本应用。
> 2. 时间读取只走 `rtc.md` 与内核单调时钟；**应用不直接写硬件 RTC**，系统时间修改只经 `time_svc.service` 并需管理员授权。
> 3. **条目不在此文档硬编码**：时间应用对外条目的集合、命名、权限由 `time.api`、`time.sync.api`、`time.timer.api` 契约文件声明，经 `.api` 桥接层加载；本文档只定义行为语义，契约全文见第 7 节。
> 4. 对应目录树 `/system/SystemApps/time/`：`time_app`（应用主程序）、`time_svc.service`（后台校时与时间服务器服务）、`config`（时区库、地区列表、服务器、日历 / 日程 / 闹钟与计时配置）。
> 5. 依赖：`os.md`、`rtc.md`、`ui.md`、`uio.md`、`net.md`、`security.md`、`log.md`、`acl.md`、`session.md`、`update.md`、`pkg.md`、`locale.md`、`audio.md`、`trigger_task.md`、`.api.md`。

## 术语

| 术语 | 说明 |
| --- | --- |
| 任务栏时钟 | 任务栏左下角的时间与日期显示区，点击打开时间应用 |
| 地区时间 | 除本地时区外，另行添加并显示的地区当前时间、UTC 偏移与夏令时状态 |
| 时间服务器 | 提供 NTP 校时服务的主机；本应用可添加多个，也可把本机作为服务器对外提供 |
| 墙上时间 | 由 RTC 维护的日历时间，可被校时调整 |
| 单调时钟 | 内核高精度递增计时器，不受系统时间调整影响，供全部计时工具使用 |
| 秒表 | 正计时并支持计次（lap）的计时工具 |
| 倒计时 | 从设定时长递减至零并提醒的计时工具 |
| 正计时 | 从零向上累计的计时工具 |
| 番茄计时 | 工作 / 休息阶段循环、按轮次统计的计时方法 |
| 日历 | 月 / 周 / 日视图的日期浏览与日程承载界面 |
| 日程 | 带开始 / 结束时间、提醒与重复规则的事件 |
| 闹钟 | 指定时刻触发的提醒，支持重复与贪睡 |

## 1 定位与入口

1. **任务栏时钟**：位于任务栏左下角（与 Windows 10 一致），显示当前时间与日期；点击该区域直接打开时间应用主窗口。
2. **应用面板**：分为七个页签——**地区时间**、**日历**、**日程**、**闹钟**、**校时**、**计时工具**、**设置**。
3. 时间应用是**系统唯一提供时间显示、日历、日程、闹钟、校时与计时工具的应用**；其它应用不得自建时间服务、日历或另设时间服务器。
4. 应用为系统自带核心应用，**不可卸载**（`pkg.md`）。

## 2 本机时钟读取

1. **墙上时间**：经 `rtc.md` 读取本机 RTC，得到年月日时分秒与时区信息；任务栏时钟每秒刷新一次。
2. **单调时钟**：秒表、倒计时、正计时、番茄计时全部基于内核单调时钟，**不使用墙上时间**，避免校时导致计时跳变。
3. 时间应用对 RTC **只读**；修改系统时间必须经 `time_svc.service`，由 `user_admin` 提权确认后执行，并写 `log.md` 审计。
4. 读取结果附带时钟源标记：`rtc` / `ntp` / `manual`，供界面显示当前时间来源。

## 3 地区时间

1. 默认显示本地时区；本地时区由 `locale.md` / 系统配置决定。
2. **添加地区**：按 IANA 时区名（如 `Asia/Shanghai`、`Europe/London`）搜索并加入列表，显示该地区当前时间、UTC 偏移、夏令时状态。
3. **删除地区**：从列表移除；本地时区不可删除，只能切换。
4. 地区列表持久化于 `config/regions`，登录后按账户配置视图加载（`session.md`）。
5. 时区数据库随系统更新维护（`update.md`），存放于 `config/tzdata`；更新后无需重启即可生效。

## 4 网络校时与时间服务器

### 4.1 校时（客户端）

1. 默认使用 NTP（UDP 123）向时间服务器校时；支持添加多个服务器，按优先级依次尝试，失败自动回退下一个。
2. 支持**自动校时**（按周期）与**立即同步**（手动触发）。
3. 偏差处理：小于阈值采用**缓慢调整（slewing）**，不跳变；大于阈值（默认 5 分钟）必须经 `user_admin` 确认后才**跳变**，并在界面标注影响。
4. 校时失败不清除上次成功结果，界面保留「上次成功校时」时间与偏差值。
5. 服务器地址支持主机名与 IP，可配置端口；地址进入白名单后才允许连接。

### 4.2 作为时间服务器

1. 本机可开启 NTP 服务端（默认**关闭**），向局域网提供校时；开启需 `user_admin` 授权。
2. 开启后可设置监听端口（默认 123）、允许来源网段、每秒请求上限。
3. 服务端只提供只读时间应答，**不接受任何配置写入**；全部请求写审计（来源、时间、结果）。
4. 检测到异常请求（放大攻击、伪造源）时自动限流并记录，不响应异常报文。

## 5 计时工具

1. **秒表**：开始 / 停止 / 继续 / 复位 / 计次（lap）；显示总时长与各计次。
2. **倒计时**：设定时、分、秒后开始；支持暂停 / 继续 / 取消；到零触发提醒（通知 + 可选提示音）。
3. **正计时**：从零向上累计，支持暂停 / 复位。
4. **番茄计时**：设定工作时长（默认 25 分钟）、休息时长（默认 5 分钟）与轮次；阶段切换提醒，统计当日完成番茄数。
5. 全部计时基于单调时钟；应用退出后由 `time_svc.service` 托管在跑计时，到点仍能通知，重新打开应用可恢复显示。

## 6 日历

1. **视图**：月、周、日三种视图；默认月视图并定位到今天，可前后翻月 / 翻年、一键回到今天。
2. **日期标记**：有日程的日期显示圆点与数量；今天、选中日期有独立高亮。
3. **时区显示**：日历按本地时区渲染；切换时区后，日程时间按新时区换算显示，底层仍存 UTC。
4. **节假日与周末**：周起始日、节假日与调休标记由 `locale.md` 与 `config/holidays` 提供，可按地区切换。
5. 日历只读系统时间，不修改 RTC；日期计算全部基于本地时区与 UTC 互转。

## 7 日程

1. **创建**：标题、开始 / 结束时间、是否全天、地点、备注、提醒提前量、重复规则（不重复 / 每天 / 每周 / 每月 / 每年 / 自定义）。
2. **存储**：底层统一存 **UTC + 时区 ID**，显示按当前时区换算，跨时区不漂移。
3. **提醒**：到点由 `time_svc.service` 触发，弹通知（`ui.md`）并可播提示音；关机 / 休眠期间错过的提醒，开机后补提示并标注「已错过」。
4. **重复实例**：按重复规则展开实例；修改单次只影响该实例，删除可选「仅本次」或「全部」。
5. **账户隔离**：日程按账户存储，登录后加载各自配置视图（`session.md`），互不可见。
6. 日程数据属个人数据，不入系统重要文件，不随系统更新改写。

## 8 闹钟

1. **设定**：时 : 分，支持重复（每天 / 工作日 / 自定义星期）。
2. **触发**：全屏闹钟界面 + 铃声（`audio.md`），显示时间、标签与贪睡 / 关闭按钮。
3. **贪睡**：默认 5 分钟，可配 5 / 10 / 15 分钟与最大次数；达到次数上限自动关闭。
4. **错过补提醒**：关机或休眠期间错过的闹钟，开机后补提醒并标注原定时刻。
5. **与日程提醒区分**：闹钟是纯时刻提醒，无时间段与地点；日程是带起止时间的日历事件。
6. 铃声文件位于 `config/sounds`，可自定义，格式由 `audio.md` 解码。

## 9 与各子系统对接

1. **os.md**：时间应用复用对象‑条目模型；任务栏时钟为 UI 图层中的一个控件。
2. **rtc.md**：墙上时间与单调时钟的唯一来源；本应用只读。
3. **ui.md / uio.md**：任务栏时钟控件、应用窗口、页签与提醒弹窗。
4. **net.md**：NTP 客户端与服务端的网络收发、端口监听与限流。
5. **security.md**：校时与开启时间服务器属系统级操作，纳入安全策略。
6. **acl.md**：校时、改系统时间、开启服务端需 `user_admin`；地区时间与计时工具为 `user_foreground`。
7. **log.md**：改时间、校时、服务端请求全部审计。
8. **session.md**：地区列表、日历日程、闹钟与计时配置按账户隔离。
9. **update.md**：时区数据库随系统更新。
10. **pkg.md**：系统自带应用不可卸载；卸载连带其 API、组件与配置。
11. **trigger_task.md**：日程、闹钟与计时器的到点触发复用触发任务器的定时能力。
12. **locale.md**：日历的周起始日、节假日与调休标记。
13. **audio.md**：闹钟与日程提醒的铃声播放与格式解码。

## 10 时间应用 `.api` 契约文件（四份全文）

时间应用对外接口由四份契约声明，分别存放于 `/system/api/time/time.api`、`/system/api/time/time.sync.api`、`/system/api/time/time.timer.api`、`/system/api/time/time.schedule.api`，由桥接层加载解析。**本文档内置契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# time.api 时间应用基础对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = time
env = python
exec = /system/SystemApps/time/time_app
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 20次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/time/app time_app

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
time_app: now { } /internal/time/now
time_app: date { } /internal/time/date
time_app: timezone { } /internal/time/timezone
time_app: regions { } /internal/time/regions
time_app: clock_source { } /internal/time/source

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
time_app: set_timezone { return self.set_timezone(tz) } /internal/time/set_tz
time_app: add_region { return self.add_region(tz) } /internal/time/add_region
time_app: remove_region { return self.remove_region(tz) } /internal/time/rm_region
time_app: open { return self.open(page) } /internal/time/open

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    time_changed() ｜ /internal/event/time_changed
    timezone_changed(tz: str) ｜ /internal/event/tz_changed
    region_added(tz: str) ｜ /internal/event/region_added

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    set_timezone: 5次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    set_timezone: user_foreground
    add_region: user_foreground
    remove_region: user_foreground
```

```text
# time.sync.api 校时与时间服务器对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = time.sync
env = python
exec = /system/SystemApps/time/time_svc.service
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 10次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/time/sync time_sync

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
time_sync: state { } /internal/sync/state
time_sync: last_sync { } /internal/sync/last
time_sync: offset_ms { } /internal/sync/offset
time_sync: servers { } /internal/sync/servers
time_sync: server_enabled { } /internal/sync/server_on
time_sync: server_stats { } /internal/sync/server_stats

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
time_sync: sync_now { return self.sync_now() } /internal/sync/now
time_sync: add_server { return self.add_server(addr, port) } /internal/sync/add_server
time_sync: remove_server { return self.remove_server(addr) } /internal/sync/rm_server
time_sync: set_auto { return self.set_auto(enabled, interval) } /internal/sync/auto
time_sync: enable_server { return self.enable_server(port, cidr, limit) } /internal/sync/server_start
time_sync: disable_server { return self.disable_server() } /internal/sync/server_stop

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    sync_ok(offset_ms: int) ｜ /internal/event/sync_ok
    sync_fail(reason: str) ｜ /internal/event/sync_fail
    server_started(port: int) ｜ /internal/event/server_start
    server_stopped() ｜ /internal/event/server_stop

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    sync_now: 1次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    sync_now: user_admin
    add_server: user_admin
    remove_server: user_admin
    set_auto: user_admin
    enable_server: user_admin
    disable_server: user_admin
```

```text
# time.timer.api 计时工具对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = time.timer
env = python
exec = /system/SystemApps/time/time_app
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 10次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/time/timer time_timer

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
time_timer: stopwatch_ms { } /internal/timer/stopwatch
time_timer: countdown_remaining { } /internal/timer/countdown_left
time_timer: countup_ms { } /internal/timer/countup
time_timer: pomodoro_phase { } /internal/timer/pomodoro_phase
time_timer: pomodoro_count { } /internal/timer/pomodoro_count

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
time_timer: stopwatch_start { return self.stopwatch_start() } /internal/timer/stopwatch_start
time_timer: stopwatch_stop { return self.stopwatch_stop() } /internal/timer/stopwatch_stop
time_timer: stopwatch_lap { return self.stopwatch_lap() } /internal/timer/stopwatch_lap
time_timer: stopwatch_reset { return self.stopwatch_reset() } /internal/timer/stopwatch_reset
time_timer: countdown_start { return self.countdown_start(seconds) } /internal/timer/countdown_start
time_timer: countdown_pause { return self.countdown_pause() } /internal/timer/countdown_pause
time_timer: countdown_resume { return self.countdown_resume() } /internal/timer/countdown_resume
time_timer: countdown_cancel { return self.countdown_cancel() } /internal/timer/countdown_cancel
time_timer: countup_start { return self.countup_start() } /internal/timer/countup_start
time_timer: countup_pause { return self.countup_pause() } /internal/timer/countup_pause
time_timer: countup_reset { return self.countup_reset() } /internal/timer/countup_reset
time_timer: pomodoro_start { return self.pomodoro_start(work, rest, rounds) } /internal/timer/pomodoro_start
time_timer: pomodoro_pause { return self.pomodoro_pause() } /internal/timer/pomodoro_pause
time_timer: pomodoro_skip { return self.pomodoro_skip() } /internal/timer/pomodoro_skip
time_timer: pomodoro_stop { return self.pomodoro_stop() } /internal/timer/pomodoro_stop

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    timer_fired(kind: str) ｜ /internal/event/timer_fired
    lap(index: int, ms: int) ｜ /internal/event/lap
    pomodoro_phase(phase: str) ｜ /internal/event/pomodoro_phase

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    stopwatch_lap: 5次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    stopwatch_start: user_foreground
    countdown_start: user_foreground
    pomodoro_start: user_foreground
```
```text
# time.schedule.api 日历 / 日程 / 闹钟对外接口契约
# API版本标识，主版本不兼容变更+1，次版本新增能力+1
version = 1.0
# 绑定目标子系统标识
target = time.schedule
env = python
exec = /system/SystemApps/time/time_svc.service
internal_loc = map + symbol + register
# 最低兼容系统内核版本
min_os_version = 1.0
# 全局默认ACL权限标签，单接口可单独覆写
global_acl = user_foreground
# 全局默认调用限流规则
global_limit = 20次/秒

# ---- 申请对外对象：object <对象目录> <对象名称>，空格分隔 ----
object /system/api/time/schedule time_schedule

# ---- 只读条目：无转换逻辑使用空大括号 {}，直接透传内部数据 ----
time_schedule: month { } /internal/schedule/month
time_schedule: events { } /internal/schedule/events
time_schedule: alarms { } /internal/schedule/alarms
time_schedule: next_event { } /internal/schedule/next
time_schedule: alarm_state { } /internal/schedule/alarm_state

# ---- 可写控制条目：转换代码为Python，实现自动化控制 ----
time_schedule: event_create { return self.event_create(title, start, end, all_day, repeat, remind) } /internal/schedule/event_create
time_schedule: event_update { return self.event_update(id, field, value) } /internal/schedule/event_update
time_schedule: event_delete { return self.event_delete(id, scope) } /internal/schedule/event_delete
time_schedule: alarm_create { return self.alarm_create(time, repeat, sound) } /internal/schedule/alarm_create
time_schedule: alarm_update { return self.alarm_update(id, field, value) } /internal/schedule/alarm_update
time_schedule: alarm_delete { return self.alarm_delete(id) } /internal/schedule/alarm_delete
time_schedule: alarm_snooze { return self.alarm_snooze(minutes) } /internal/schedule/alarm_snooze
time_schedule: alarm_dismiss { return self.alarm_dismiss() } /internal/schedule/alarm_dismiss

# ---- 对外可监听标准事件（支持程序自动化感知） ----
EVENT:
    event_created(id: str) ｜ /internal/event/event_created
    event_reminder(id: str) ｜ /internal/event/event_remind
    alarm_fired(id: str) ｜ /internal/event/alarm_fired
    alarm_snoozed(id: str, minutes: int) ｜ /internal/event/alarm_snooze

# ---- 单接口独立限流规则（覆写全局限流） ----
LIMIT_OVERRIDE:
    event_create: 5次/秒

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    event_create: user_foreground
    event_update: user_foreground
    event_delete: user_foreground
    alarm_create: user_foreground
    alarm_update: user_foreground
    alarm_delete: user_foreground
```
契约要点：

1. 基础对象 `/object/time/app`，校时对象 `/object/time/sync`，计时对象 `/object/time/timer`。
2. 基础接口读当前时间、日期、时区、地区列表与时钟源；写切换时区、增删地区、打开页签。
3. 校时接口读状态、上次成功、偏差、服务器列表与服务端状态；写立即同步、增删服务器、自动校时、启停服务端。
4. 计时接口读秒表 / 倒计时 / 正计时 / 番茄状态；写各计时器的开始、暂停、复位、计次与阶段跳过。
5. **改系统时间、校时与开启时间服务器 ACL 为 `user_admin`**；地区时间与计时工具为 `user_foreground`。
6. 校时与时间服务器事件全部审计；服务端只应答只读时间，不接受任何写入。
7. 主版本变更代表接口不兼容；内部实现可任意重构，只要版本号不变，外部无感知。
8. 日程 / 闹钟对象读写月视图、日程列表、闹钟列表、下一次提醒与闹钟状态；写日程增改删、闹钟增改删 / 贪睡 / 关闭。
9. 日程与闹钟按账户隔离；提醒与闹钟到点由 `time_svc.service` 后台触发，不依赖应用是否打开。
10. 日程底层存 UTC + 时区 ID，显示按当前时区换算；重复日程可按实例或整体删除。

## 11 设计取舍

1. **应用只读 RTC**：改系统时间必须走 `time_svc.service` + 管理员提权 + 审计，防止应用静默篡改系统时钟。
2. **计时全部用单调时钟**：校时调整墙上时间不会打断秒表 / 倒计时 / 番茄计时。
3. **小偏差缓慢调整、大偏差需确认跳变**：避免时间突变影响运行中的任务与日志顺序。
4. **时间服务器默认关闭**：开启需管理员授权，并限定来源网段、请求上限，防止被用于放大攻击。
5. **计时由服务托管**：应用退出后计时仍在跑，到点仍能提醒。
6. **地区时间按账户隔离**：不同用户的地区列表互不可见，配置视图各自独立。
7. **系统自带应用不可卸载**：本应用拥有独立 API、组件与配置，卸载会连带破坏依赖它的应用（`pkg.md`）。
8. **日历 / 日程 / 闹钟分离建模**：日历只管日期视图与浏览，日程承载事件与提醒，闹钟只做纯时刻提醒，互不混用。
9. **日程存 UTC**：跨时区显示不漂移，底层统一 UTC + 时区 ID。
10. **错过补提醒**：关机 / 休眠期间错过的日程与闹钟，开机后补提示并标注，不静默丢弃。