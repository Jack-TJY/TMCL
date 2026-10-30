# GitHub 标签

Issue 与 PR 统一使用下列标签。标签名、含义改动需在 PR 中说明；新增模块时追加对应的 `module:*` 标签。

## 模块 `module:*`

与 AGENTS.md §4 的模块标识一一对应（`widget-<名称>` 统一用 `module:widget`）。颜色 `#1d76db`。

| 标签 | 路径 |
| --- | --- |
| `module:common` | `core/common/` |
| `module:net` | `core/net/` |
| `module:game` | `core/game/` |
| `module:java` | `core/java/` |
| `module:auth` | `core/auth/` |
| `module:instance` | `core/instance/` |
| `module:launch` | `core/launch/` |
| `module:loader` | `core/loader/` |
| `module:resources` | `modules/resources/` |
| `module:skin` | `modules/skin/` |
| `module:sync` | `modules/sync/` |
| `module:p2p` | `modules/net-p2p/` |
| `module:sakura` | `modules/net-sakura/` |
| `module:seed` | `modules/seed/` |
| `module:crash` | `modules/crash/` |
| `module:app` | `src-tauri/` |
| `module:ui` | `ui/` |
| `module:widget` | `ui/src/widgets/<名称>/` |
| `module:theme` | `ui/src/theme/`、`ui/src/components/app-background/` |
| `module:ci` | `.github/`、`scripts/`、`deny.toml` |
| `module:docs` | `docs/` |

## 优先级

| 标签 | 含义 | 颜色 |
| --- | --- | --- |
| `P0` | M1 Alpha 必需 | `#b60205` |
| `P1` | M2 Beta | `#d93f0b` |
| `P2` | M3 正式版 | `#fbca04` |

## 缺陷等级 `severity:*`

Beta 公测中给 `bug` Issue 标缺陷等级，含义见 [testing/beta-plan.md](testing/beta-plan.md) §2。注意与上面表示里程碑的 `P0`/`P1`/`P2` 区分。

| 标签 | 含义 | 颜色 |
| --- | --- | --- |
| `severity:P0` | 主流程不可用、数据丢失或泄露凭据 | `#b60205` |
| `severity:P1` | 功能有明显问题，或只影响个别版本 / 平台 | `#d93f0b` |
| `severity:P2` | 体验问题 | `#fbca04` |

## 状态与流程

| 标签 | 含义 | 何时加 / 去掉 | 颜色 |
| --- | --- | --- | --- |
| `status:claimed` | 已被认领 | 认领时加在步骤 Issue 上；完成或放弃时去掉（AGENTS.md §7） | `#0e8a16` |
| `status:blocked` | 被外部事项阻塞 | 写明等待什么、由谁处理；解除后去掉（AGENTS.md §16） | `#e11d21` |
| `status:handoff` | 等待其他 Agent 接手 | 交接时加；接手方评论“已接手”后去掉（AGENTS.md §12） | `#5319e7` |
| `contract-change` | 修改接口契约 | 契约 PR 必加，维护者须逐行评审（AGENTS.md §6.2、§10.1） | `#f9d0c4` |
| `needs-user` | 需要维护者决定 | 问题写成选择题；维护者答复后去掉（AGENTS.md §16） | `#c5def5` |
| `needs-info` | 等待报告者补充信息 | 分诊时信息不足就加；报告者补充后去掉，14 天无回复关闭（beta-plan.md §4） | `#d4c5f9` |

## 类型

沿用 GitHub 默认标签：`bug`（问题反馈模板自动添加）、`enhancement`（功能建议模板自动添加）、`documentation`、`duplicate`、`question`、`wontfix` 等。
