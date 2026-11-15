# OOS（Object Operating System）

OOS 是一套自研操作系统的**设计规范文档集**，以「对象‑条目」为统一抽象，采用混合内核架构，面向 x86‑64。

> 当前版本：**v0.1**（基础内核已实现，可引导运行）

## 核心理念

1. **万物皆内核虚拟对象**：系统资源统一表示为对象，对象由多个条目组成；外部通过条目访问，内部实现可自由重构。
2. **混合内核 = 微内核（原生内核）+ 内核扩展文件**：微内核是原生内核，核心常驻不可卸载；内核扩展文件就是其他系统所称的驱动，作为混合内核中除微内核之外的全部部分。
3. **用户态服务即本系统的驱动**：性能收益大且出错率低的功能进内核扩展文件，其余一律作为 Ring3 用户态服务运行，崩溃只影响自身。
4. **接口契约化**：每个子系统对外条目由 `.api` 契约文件声明，经桥接层加载；文档内嵌契约全文，文档与契约同步维护。
5. **权限面切分**：系统账户（`system`、`update`）与用户账户（`user`、`admin`、`root`）权限互不重叠；`system` 几乎可操作任何资源但不能读写自身文件，`update` 窄而深且是系统重要文件唯一写入者。

## 文档导航

| 分类 | 文档 |
| --- | --- |
| 总纲与模型 | `os.md`、`dirtree.md`、`objmgr.md`、`.api.md`、`srjg.md`、`cmd.md`、`config.md` |
| 内核与启动 | `kernel.md`、`boot.md`、`asm.md`、`hardware_svc.md`、`devmgr.md`、`rtc.md` |
| 进程 / 内存 / CPU / IPC | `process.md`、`memory.md`、`cpu.md`、`ipc.md`、`procmon.md` |
| 存储与文件 | `fs.md`、`storage.md`、`filemgr.md`、`backup.md`、`vm.md`、`clipboard.md` |
| 安全与账户 | `security.md`、`acl.md`、`auth.md`、`account.md`、`secure_island.md`、`session.md`、`log.md` |
| 服务与任务 | `service.md`、`trigger_task.md`、`pkg.md`、`install.md`、`update.md` |
| 网络与外设 | `net.md`、`external_dev.md`、`input.md` |
| 图形与界面 | `ui.md`、`uio.md`、`font.md`、`locale.md` |
| 多媒体 | `audio.md`、`media.md`、`img.md`、`cam.md`、`rec.md`、`video.md` |
| 系统应用 | `note.md`、`time.md`、`map.md` |
| 对象与格式 | `dllo.md`、`exeo.md`、`tpmeo.md`、`lerf.md`、`tmmf.md` |
| 兼容层与电源 | `Linux.md`、`power.md` |

共 **59 份**规范文档，全部位于 [`docs/`](docs/)。真实代码位于 [`src/kernel/`](src/kernel/)。

## 仓库结构

```
.
├─ docs/                  59 份设计规范
├─ src/kernel/            Rust 内核工程（可引导基础内核）
├─ LICENSE                MPL 2.0 全文
├─ README.md              项目说明
└─ RELEASE_NOTES_v0.md    v0 发布公告
```

## 阅读顺序

1. [`docs/os.md`](docs/os.md) — 架构总纲与混合内核三层归属
2. [`docs/objmgr.md`](docs/objmgr.md) — 全局内核对象管理器
3. [`docs/.api.md`](docs/.api.md) — 对象条目与接口契约模型
4. [`docs/kernel.md`](docs/kernel.md)、[`docs/boot.md`](docs/boot.md) — 内核镜像、内核扩展文件与启动流程
5. [`docs/process.md`](docs/process.md)、[`docs/memory.md`](docs/memory.md)、[`docs/cpu.md`](docs/cpu.md)、[`docs/ipc.md`](docs/ipc.md) — 执行与通信基础
6. [`docs/fs.md`](docs/fs.md)、[`docs/storage.md`](docs/storage.md)、[`docs/filemgr.md`](docs/filemgr.md) — 存储与文件
7. [`docs/security.md`](docs/security.md)、[`docs/acl.md`](docs/acl.md)、[`docs/auth.md`](docs/auth.md)、[`docs/account.md`](docs/account.md) — 安全、权限与账户
8. [`docs/ui.md`](docs/ui.md)、[`docs/uio.md`](docs/uio.md)、[`docs/font.md`](docs/font.md) — 图形界面
9. 其余子系统与应用文档按需查阅

## 版本

- **v0**：首个规范版本，完整覆盖架构、内核、执行、存储、安全、界面、应用与格式，共 59 份文档。
- **v0.1**：首个实现版本，发布 Rust 基础内核与 UEFI bootloader，可引导并交互。

## 许可

本项目采用 **Mozilla Public License 2.0（MPL 2.0）**，全文见 [`LICENSE`](LICENSE)。

- **文件级传染**：修改本项目任何源文件并以源码形式分发时，被修改的文件必须以 MPL 2.0 公开。
- **不传染上层代码**：把本项目与专有代码组合成更大作品（Larger Work）不受限制，无需公开你自己的上层代码。
- 分发二进制时，须以 MPL 2.0 提供对应源文件，并告知获取方式。
- 未修改的文件可继续按原许可使用；专利授权随 MPL 2.0 一并授予。

## 贡献

文档以「行为语义 + `.api` 契约全文」成对维护。修改接口时，正文与契约必须同步更新；契约版本号按主版本不兼容变更 +1、次版本新增能力 +1 维护。

## 发行版与许可证

- 本仓库是 **OOS 的官方内核**，也是 **OOS 的官方发行版**：包含 59 份设计规范（`docs/`）与 Rust 基础内核实现（`src/kernel/`）。
- OOS 是本系统的唯一基础与官方来源。任何以 OOS 为基础构建的操作系统、发行版或衍生系统，均视为 **OOS 发行版**，必须遵循本仓库的 **MPL 2.0** 许可证。
- 发行版须履行：保留 [`LICENSE`](LICENSE) 与版权声明；对修改过的 OOS 源文件继续以 MPL 2.0 公开并提供获取方式；在发行版说明中注明其基于 OOS 及所对应的版本号（如 v0.1）。
- 发行版自身新增的代码可以另行选择许可证，但不得削减、替代或限制本仓库对 OOS 源文件已经授予的权利。
- **官方发行版仅以本仓库发布的版本为准**。未经维护者授权，任何衍生发行版不得自称「官方 OOS 发行版」或使用官方名义发布。
