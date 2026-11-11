# locale.md 本地化规范

> 
> 前提说明
> 
> 1. 负责系统本地化：显示语言、区域格式（日期 / 数字 / 货币）、键盘布局与排序规则。
> 2. 本地化配置存放于系统配置表（`config.md` 的 sysconfig），本规范定义其结构与生效机制。
> 3. 语言资源以语言包形式发布（文本、翻译、区域数据），安装后立即可切换。
> 4. 字体渲染由 `font.md` / `lerf.md` 负责；本规范只负责"选哪种语言 / 区域规则"。
> 5. 本地化按 **会话级 + 系统级** 两级：系统默认 + 用户会话覆盖。

## 1 总体约束

1. 两级配置：
   - **系统默认**：安装时确定（见 `install.md` 首次配置），未登录界面 / 登录界面使用；
   - **会话覆盖**：登录用户可另选语言与区域，仅影响本会话。
2. 区域格式决定：日期时间格式、数字分隔符、货币符号、排序与大小写规则。
3. 键盘布局属于输入映射（见 `input.md`），由本地化配置引用（如 `zh_CN` → 拼音输入法 + 标准键盘）。
4. 语言包包含翻译资源（界面字符串）与区域数据（CLDR 类）；缺失语言的界面回退到系统默认语言。
5. 切换语言即时生效或下次启动生效由应用决定：系统界面即时生效，常驻应用跟随。

## 2 本地化对象条目布局

### 本地化对象，路径 `/object/locale/main`

| 条目名称 | 权限 | 作用 |
| --- | --- | --- |
| `system` | 只读 | 系统默认本地化（语言 / 区域） |
| `session` | 可读可写 | 当前会话本地化覆盖 |
| `lang_list` | 只读 | 已安装语言列表 |
| `region_list` | 只读 | 可用区域格式列表 |
| `keymap` | 可读可写 | 当前键盘布局引用 |
| `install_pack` | 只写 | 安装语言包（aux_system） |

## 3 核心操作流程示例

### 示例 1：首次配置选择语言（安装器）

1. 安装向导列出可用语言包（`install.md` 第 2 步）；
2. 选定后写入系统默认本地化（sysconfig）；
3. 安装完成重启后登录界面即使用该语言。

### 示例 2：用户切换界面语言

1. 用户修改会话本地化（语言 / 区域）；
2. 广播 `locale_changed`；
3. 界面组件按新语言重载字符串，输入法 / 键盘布局随 keymap 更新。

## 4 和其它文档对接关系

1. **对接 config.md**：本地化配置持久化于 sysconfig。
2. **对接 install.md**：首次配置确定系统默认语言 / 区域 / 键盘布局。
3. **对接 input.md**：键盘布局与输入法随本地化切换。
4. **对接 font.md / lerf.md**：语言渲染所需字体由字体子系统提供。
5. **对接 cmd.md**：命令行提示与错误消息本地化。
6. **对接 rtc.md**：日期时间格式按区域规则显示（时间本身不变）。

## 5 设计取舍

1. 两级配置：系统级保证登录前可用，会话级满足个人习惯。
2. 语言包独立安装：不随系统镜像捆绑全部语言，节省空间（配合 `pkg.md` 分发）。
3. 回退机制：缺失翻译回退默认语言，不因缺包白屏。

## 6 本地化 `.api` 契约文件（locale.api 全文）

本地化对外接口统一由以下契约文件声明，文件存放于 `/system/api/locale.api`，由桥接层加载解析。**本文档内置本契约全文，后续接口增删改必须同步维护此处与契约文件两者一致。**

```text
# locale.api 本地化对外接口契约
version = 1.0
target = locale
env = python
exec = /system/config/locale/locale_svc
internal_loc = map + symbol + register
min_os_version = 1.0
global_acl = user_foreground
global_limit = 50次/秒

object /system/api/locale locale

# ---- 只读条目 ----
locale: locale_system { } /internal/locale/system
locale: locale_langs { } /internal/locale/lang_list
locale: locale_regions { } /internal/locale/region_list

# ---- 可写控制条目 ----
locale: locale_set {
    return self.locale_set(lang, region)
} /internal/locale/session
locale: keymap_set {
    return self.keymap_set(km)
} /internal/locale/keymap
locale: pack_install {
    return self.pack_install(path)
} /internal/locale/pack

# ---- 对外可监听标准事件 ----
EVENT:
    locale_changed(lang: str, region: str) ｜ /internal/event/locale_change

# ---- 单接口独立ACL权限覆写（优先级高于global_acl） ----
ACL_OVERRIDE:
    locale_system: user_foreground
    pack_install: aux_system
```

契约要点：

1. `locale_set` 写会话级覆盖；`pack_install` 安装语言包仅 `aux_system`。
2. `locale_changed` 事件通知界面组件重载字符串、输入法切换。
3. 系统默认本地化只读，由安装流程 / 系统配置写入。
