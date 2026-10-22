# 接口契约

本目录存放 TMCL 各模块之间、前后端之间的接口契约。契约是并行开发的基础：每个模块只依赖这里写明的接口，不依赖别人模块的内部实现。

本目录为**受保护目录**（AGENTS.md §5.1），修改规则见 AGENTS.md §6 与 [PLAN.md](../PLAN.md) 第 3 节。

## 文件索引

| 文件 | 内容 | 建立步骤 |
| --- | --- | --- |
| `errors.md` | 错误码清单（`<模块>_<原因>`，按字母排序，只追加） | M0-05 |
| `data-layout.md` | 数据目录结构、`config.json`、`instance.json` 字段与 schema 版本 | M0-05 |
| `ipc.md` | Tauri 命令、事件名与负载、类型生成机制 | M0-06 |
| `widgets.md` | 主页组件的注册、尺寸、配置与生命周期 | M1-18 |
| `third-party.md` | 二进制组件（frpc、authlib-injector、cubiomes 等）的来源、版本、许可证、校验和、是否随安装包分发；也登记随包分发图片等素材的许可 | M1-19 或首个引入第三方组件的步骤 |

尚未建立的文件由“建立步骤”一栏的步骤创建。

## 什么算契约

- 错误类型 `AppError` 与错误码 `ErrorCode`
- Tauri 命令名、参数与返回结构
- 事件名与负载
- `TaskCtx` / `ProgressSink`
- 数据目录结构、`instance.json`、`config.json` 字段
- 跨 crate 的公开 trait 与结构体

## 怎么改

| 变更类型 | 流程 |
| --- | --- |
| **追加**：新增命令、事件、错误码、配置字段 | 可随功能 PR 一起提交，在 PR 描述“契约变更”一栏列出 |
| **修改**：改名、删除、改类型、改语义 | 单独开契约 PR：标题以 `contract:` 开头，加 `contract-change` 标签，只含契约文档 + 类型定义 + 重新生成的 `ui/src/bindings/` + 受影响调用方的最小适配 |
| `config.json` / `instance.json` 结构变化 | 提升 `schema`，写迁移函数和测试 |

契约 PR 的描述须写清：为什么改、是否向后兼容、受影响的模块与正在进行中的 PR。维护者合并后，各功能 PR rebase 到新 `main` 再继续。

## 依赖尚未实现的接口

按契约在自己模块里写 mock / fake（Rust 用 trait + 测试替身，前端用 `ui/src/api/__mocks__/`），不要改别人模块的代码。
