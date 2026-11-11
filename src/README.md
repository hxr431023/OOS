# src — 真实代码目录

本目录存放 OOS 的实现代码。`docs/` 是对应的设计规范，代码与文档按子系统一一对应。

## 目录约定

| 目录 | 内容 | 对应规范 |
| --- | --- | --- |
| `kernel/` | 微内核、内核扩展文件、启动与体系结构相关代码 | `docs/kernel.md`、`docs/boot.md`、`docs/asm.md` |
| `services/` | 用户态服务（服务管理器、服务加载器、服务运行容器及各服务） | `docs/service.md` |
| `apps/` | 系统应用（时间、便签、截图、录音、视频、地图、文件管理器、资源监视器） | `docs/time.md` 等应用文档 |
| `libs/` | 公共库、对象管理器、IPC 与格式解析 | `docs/objmgr.md`、`docs/ipc.md`、`docs/dllo.md`、`docs/lerf.md`、`docs/tmmf.md` |
| `compat/` | Linux 兼容层（POSIX + VFS 简易版） | `docs/Linux.md` |
| `build/` | 构建脚本与工具链配置 | — |

## 状态

截至 v0，本目录仅包含结构说明，尚无实现代码。首个实现目标是微内核与对象管理器最小原型。

## 许可

本目录代码采用 **MPL 2.0**。修改本目录中的文件并以源码形式分发时，修改后的文件须继续以 MPL 2.0 公开；与专有代码组合成更大作品不受限制，无需公开上层专有代码。