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
