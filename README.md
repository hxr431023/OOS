# OOS（Object Operating System）

OOS 是一套自研操作系统的**设计规范文档集**，以「对象‑条目」为统一抽象，采用混合内核架构，面向 x86‑64。

> 当前版本：**v0**（设计规范阶段，尚无实现代码）

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

共 **59 份**规范文档。

## 阅读顺序

1. `os.md` — 架构总纲与混合内核三层归属
2. `objmgr.md` — 全局内核对象管理器
3. `.api.md` — 对象条目与接口契约模型
4. `kernel.md`、`boot.md` — 内核镜像、内核扩展文件与启动流程
5. `process.md`、`memory.md`、`cpu.md`、`ipc.md` — 执行与通信基础
6. `fs.md`、`storage.md`、`filemgr.md` — 存储与文件
7. `security.md`、`acl.md`、`auth.md`、`account.md` — 安全、权限与账户
8. `ui.md`、`uio.md`、`font.md` — 图形界面
9. 其余子系统与应用文档按需查阅

## 版本

- **v0**：首个规范版本，完整覆盖架构、内核、执行、存储、安全、界面、应用与格式，共 59 份文档。

## 许可

暂未指定开源许可证；在明确之前，保留所有权利。

## 贡献

文档以「行为语义 + `.api` 契约全文」成对维护。修改接口时，正文与契约必须同步更新；契约版本号按主版本不兼容变更 +1、次版本新增能力 +1 维护。