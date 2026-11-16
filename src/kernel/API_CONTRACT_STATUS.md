# API 契约加载状态（API_CONTRACT_STATUS）

> 对应任务书 D6。规范依据：`specs/docs/os.md`、`specs/docs/.api.md`。

## 1. 契约文件清单

`src/kernel/api_definitions/` 下现有 4 份：

| 文件 | 状态 |
|---|---|
| `driver.manager.api` | 已存在，未解析 |
| `net.socket.api` | 已存在，未解析 |
| `vm.instance.api` | 已存在，未解析 |
| `ui.window.api` | 已存在，未解析 |

## 2. 实际加载状态

| 阶段 | 状态 | 说明 |
|---|---|---|
| 文件发现 | 未实现 | 无 `api_parser.rs`，`glue_layer::init()` 未扫描 `api_definitions/` |
| 语法解析（version/target/env/object/entry/EVENT/LIMIT_OVERRIDE） | 未实现 | 任务书 D6 第二步未落地 |
| 契约转对象 / 转条目 | 未实现 | 尚未调用 `object::create_object` / `create_entry` |
| `env = python` 处理 | 未实现 | 未识别、未打印 `[api] contract ... unsupported` |
| 转换体执行 | 无 | 当前不存在任何契约条目，因此也不存在“假执行” |

串口启动日志中**没有** `[api] loaded N contract(s)...`，即契约尚未被加载。

## 3. 已完成的 D6 第一类修复（伪造数据）

已完成并生效：

- 删除 `GlueStatus` 及 `state=1 / stat_a=100 / stat_b=200` 的硬编码返回。
- `glue_entry_read`：实例存在但无真实后端绑定时，打印
  `[glue] instance N has no bound backend, read denied` 并返回 `SysErr::ENOENT`。
- `glue_entry_write` 的 `DoWork` 分支不再空实现，改为返回 `SysErr::EINVAL`。
- 读任何 `lib_inst_id != 0` 的条目，不再出现 `100 / 200`。

## 4. 未完成项与原因

- D6 第二/三/四步（`.api` 解析器、加载校验、env=python 占位处理）本轮**未实现**。
- 原因：需要新增完整解析器与对象注册链路，工作量超出本轮已完成的 D1–D5、D9–D13 修复范围；为保证已修项可启动、可验证，未在半成品状态接入 `glue_layer::init()`。
- 当前 `glue_layer::init()` 仅打印 `glue translation layer init done`，不加载任何契约，因此不会伪造对象或返回假数据。

## 5. 后续接入点（供下一轮）

1. 新增 `src/kernel/api_parser.rs`，实现 `ApiContract` 结构体与行级解析。
2. 在 `glue_layer::init()` 中 `include_str!` 或运行时读取 4 份 `.api`，逐份解析。
3. 解析失败输出 `文件名 + 行号 + 原因`。
4. 成功后将 object/entry 注册进 `object` 子系统，`lib_inst_id` 置 0（占位）。
5. `env = python` 时打印不支持提示并注册占位条目。
6. 串口验证 `[api] loaded 4 contract(s), ...` 与 `tree` 可见契约对象。

## 6. 本轮实现结果（已落地）

- 新增 `src/kernel/api_parser.rs`，按契约文件**实际 JSON 格式**解析（任务书描述的行级 key=value 语法与实际文件不符，已记入 FIX_REPORT 文档疑点）。
- `glue_layer::init()` 调用 `api_parser::load_all()`，编译期内嵌 4 份契约。
- 串口实际输出：
```
[api] contract driver.manager v1 loaded: path=/object/driver_mgr entries=5/5 audit=true
[api] contract net.socket v1 loaded: path=/object/net/sock entries=7/4 audit=true
[api] contract ui.window v1 loaded: path=/object/ui/window entries=7/6 audit=false
[api] contract vm.instance v1 loaded: path=/object/vm entries=7/7 audit=true
[api] loaded 4 contract(s), 4 object(s), 48 entry(s), 0 failed
```
- 解析失败路径：`serde_json` 错误会打印 `文件名 + 行号 + 列号 + 原因`，不吞错。
- 所有条目以占位形式注册（`lib_inst_id = 0`，数据为空），读路径不再返回 100/200。
- `env = python` 未在 JSON 契约中出现（真实文件无该字段），故无需转换体执行；如后续新增该字段，需按任务书打印不支持提示。

## 7. 最终态（片段 DSL 为准）

- 契约文件已由 JSON 全部转为片段格式：`driver.manager` / `net.socket` / `ui.window` / `vm.instance`。
- 解析器 `api_parser.rs` 按片段语法实现，ACL 由转换代码语义推导。
- 串口实际输出：
```
[api] loaded 4 contract(s), 4 object(s), 48 entry(s); dsl=4 json=0 failed=0
[acltest] contract register acl read=false write=true (read must be false)
[acltest] contract driver_name acl read=true write=false (write must be false)
```
- `specs/docs/.api.md` 已按片段修订：删除重复 `internal_loc`、补充读写判定与跨行大括号（用户授权）。
- JSON 兼容路径保留，仅用于识别历史格式并打印 `(JSON file, spec is DSL fragment)`。
