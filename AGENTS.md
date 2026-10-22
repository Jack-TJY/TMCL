# AGENTS.md：TMCL 多 Agent 协作规则

本文件约束所有参与 TMCL 开发的 AI Agent（Claude Code、Codex、Copilot 等）以及人类贡献者。与 [docs/PLAN.md](docs/PLAN.md) 一起构成开发的唯一依据；两者冲突时以维护者最新的明确指示为准，其次是本文件。

## 1. 开工前必读

1. 读完本文件。
2. 读 `docs/PLAN.md` 第 0–4 节（前提、阶段、目录结构、接口契约、通用完成标准），以及你要做的那个步骤。
3. 读 `docs/contracts/` 下与你的模块相关的契约文档。
4. 运行 `git fetch origin && git log --oneline origin/main -20`，了解最近的改动。

没读完以上内容不得修改代码。

## 2. 红线（任何情况下都不允许）

1. **不提交密钥**：不得把 `.env.local` 的内容、Client ID、API Key、令牌、签名私钥写进任何被跟踪的文件、提交信息、PR、Issue、日志或聊天回复。不得 `cat`/打印 `.env.local`。
2. **不推送 `main`**：只能通过 PR 合并；不得 force push `main`，不得改写别人分支的历史。
3. **不自行合并**：只有维护者（JIANYU TANG）能合并 PR。Agent 不得批准或合并任何 PR，包括自己的。
4. **不越界改文件**：只改你认领步骤的“文件”范围内的路径，例外按 §5 执行。
5. **不绕过检查**：不得跳过、删除、`#[ignore]` 或放宽已有测试与 Lint 来让 CI 变绿；不得使用 `--no-verify`。
6. **不分发游戏文件**：仓库、测试样本、安装包中不得出现 Minecraft 客户端 jar、资源文件、Mojang Logo 或其他受版权保护的素材。测试样本只放元数据 JSON。
7. **不代表维护者对外沟通**：不得给 Mojang、Microsoft、CurseForge、SakuraFrp 等第三方发邮件、提交表单或开 Issue；只能起草。
8. **不修改受保护文件**（§5.1），除非维护者在当前任务中明确要求。

## 3. 角色

| 角色 | 谁 | 职责 |
| --- | --- | --- |
| 维护者 | JIANYU TANG | 拍板、分配步骤、审核并合并 PR、处理外部事项、管理 Secrets |
| 实现 Agent | 任一 AI Agent | 认领一个步骤，在自己的分支上实现、测试、提 PR、写交接 |
| 评审 Agent（可选） | 另一个 AI Agent | 按 §10.2 清单评审 PR，只能留评论，不能批准合并 |

每个 Agent 会话开始时给自己取一个稳定的名字，格式 `<工具>-<短标识>`，例如 `claude-a`、`codex-b`，在分支名、提交尾注和 PR 里使用同一个名字。

## 4. 模块所有权与目录边界

所有权**按步骤临时分配**：认领一个步骤，就在该步骤 PR 存续期间拥有该步骤“文件”栏列出的路径。同一路径同一时间只能有一个未合并的 PR 在改（共享文件除外，见 §5）。

| 模块标识（提交 scope） | 路径 | 说明 |
| --- | --- | --- |
| `common` | `core/common/` | 错误码、路径、配置、任务、日志、数据库。改动多为契约变更 |
| `net` | `core/net/` | HTTP、下载引擎、下载源 |
| `game` | `core/game/` | 版本元数据与文件补全 |
| `java` | `core/java/` | |
| `auth` | `core/auth/` | 含 `build.rs` 的密钥注入，评审要求更高（§10） |
| `instance` | `core/instance/` | |
| `launch` | `core/launch/` | |
| `loader` | `core/loader/` | |
| `resources` | `modules/resources/` | Modrinth、CurseForge、整合包 |
| `skin` | `modules/skin/` | |
| `sync` | `modules/sync/` | |
| `p2p` | `modules/net-p2p/` | |
| `sakura` | `modules/net-sakura/` | |
| `seed` | `modules/seed/` | |
| `crash` | `modules/crash/` | |
| `app` | `src-tauri/` | 命令层，每个模块只改 `src-tauri/src/commands/<模块>.rs` |
| `ui` | `ui/` | 页面按 `ui/src/pages/<页面>/` 划分 |
| `widget-<名称>` | `ui/src/widgets/<名称>/` | 主页组件，每个组件一个目录，契约见 `docs/contracts/widgets.md` |
| `theme` | `ui/src/theme/`、`ui/src/components/app-background/` | Win11 风格主题、背景与透明度 |
| `ci` | `.github/`、`scripts/`、`deny.toml` | |
| `docs` | `docs/`（受保护文件除外） | |

**分层依赖**（违反即打回）：`ui → src-tauri（仅 IPC）→ modules/* → core/*`；`modules/*` 之间互不依赖；`core/*`、`modules/*` 不依赖 `tauri`；`ui` 不直接访问网络和文件系统。详见 PLAN.md 第 2 节。

## 5. 共享文件与受保护文件

### 5.1 受保护文件（只有维护者明确同意才能改）

- `AGENTS.md`、`docs/PLAN.md`、`LICENSE`
- `docs/contracts/**`（接口契约）
- `core/common/src/error.rs` 中已有错误码的名称与含义（新增错误码不算修改，但要走 §6.2）
- `.github/workflows/**`、`deny.toml`、`rust-toolchain.toml`
- `core/auth/build.rs` 以及任何读取环境变量、`.env.local` 的代码
- `.gitignore` 中的密钥规则段

### 5.2 共享文件（允许多个 PR 同时改，但必须遵守格式）

| 文件 | 规则 |
| --- | --- |
| 根 `Cargo.toml` 的 `members` / `[workspace.dependencies]` | 按字母排序，一行一项；只追加不重排 |
| `src-tauri/src/lib.rs` 的 `generate_handler!` | 按模块分组，组内字母排序，一行一个命令 |
| `src-tauri/src/commands/mod.rs` | 一行一个 `pub mod`，字母排序 |
| `ui/src/router.ts` | 每个页面一个路由块，按路径字母排序 |
| `ui/src/widgets/registry.ts` | 一行登记一个组件，按组件 ID 字母排序 |
| `ui/src/locales/<lang>/<模块>.json` | 每个模块独立文件，**只改自己模块的文件**；`common.json` 和 `errors.json` 只追加 key |
| `core/common/migrations/` | 文件名 `NNNN_<模块>_<描述>.sql`，在 PR 标题旁写明占用的编号；冲突时后合并的一方改号 |
| `Cargo.lock`、`pnpm-lock.yaml` | 不手动合并：冲突时用 `main` 版本，再运行 `cargo update -w` / `pnpm install` 重新生成 |
| `docs/contracts/errors.md` | 只追加行，按错误码字母排序 |

## 6. 接口契约

### 6.1 契约是什么

PLAN.md 第 3 节和 `docs/contracts/` 中的内容：错误类型与错误码、Tauri 命令名与参数/返回结构、事件名与负载、`TaskCtx`、数据目录结构、`instance.json`、`config.json` 字段、跨 crate 的公开 trait 与结构体。

### 6.2 契约变更流程

1. 先单独开一个 PR，标题以 `contract:` 开头，加 `contract-change` 标签，只包含：契约文档修改 + 类型定义修改 + 重新生成的 `ui/src/bindings/` + 受影响调用方的最小适配。
2. PR 描述写清：为什么改、是否向后兼容、受影响的模块和正在进行中的 PR。
3. 维护者合并后，各功能 PR rebase 到新 `main` 再继续。
4. **新增**命令、事件、错误码、配置字段属于“追加”，可以和功能 PR 一起提交，但仍需在 PR 描述的“契约变更”一栏列出；**改名、删除、改类型、改语义**必须走单独的契约 PR。
5. `config.json` 与 `instance.json` 的任何结构变化都要提升 `schema` 并写迁移函数和测试。

### 6.3 依赖未完成的接口时

先在自己模块里按契约写一个 `mock` / `fake` 实现（Rust 用 trait + 测试替身，前端用 `ui/src/api/__mocks__/`），不要去改别人模块的代码来“先凑合用”。

## 7. 任务认领

一个步骤（PLAN.md 中的 `M?-??`）就是一个任务。复杂步骤可在 Issue 中拆成子任务 `M1-04a`、`M1-04b`，每个子任务列出自己的文件范围。

**认领流程**

1. 确认该步骤的所有依赖步骤已合并到 `main`（或维护者明确允许基于 mock 开工）。
2. 检查是否已被认领：
   ```
   git fetch origin
   git ls-remote --heads origin "*<步骤ID小写>-*"
   ```
   并查看 GitHub 上该步骤的 Issue 与 PR（如果当前环境能访问）。已有分支或标记 `status:claimed` 的未关闭 Issue 即为已被认领。
3. 从最新 `main` 创建分支（§8.1），立刻推送：`git push -u origin <分支>`。**推送成功即认领成功**；若因同名分支已存在而失败，说明别人先认领了，换一个任务。
4. 立即开 Draft PR，标题 `[M1-04] 游戏文件补全`，并在对应 Issue 下评论“已认领：<Agent 名>，分支 <分支名>”，加 `status:claimed` 标签。
5. 认领后 **48 小时**内没有新提交、也没有交接说明的，维护者可以收回，其他 Agent 可在 PR 下留言后接手（按 §12 交接）。

同一个 Agent 同时最多持有 **1 个**进行中的步骤（等待评审的不计入）。

## 8. 分支、提交与 PR

### 8.1 分支

- 格式：`<agent名>/<步骤ID小写>-<英文短描述>`，例如 `claude-a/m1-04-game-install`。
- 契约 PR：`<agent名>/contract-<描述>`；修复：`<agent名>/fix-<描述>`。
- 始终基于最新 `origin/main` 创建；开发过程中用 `git rebase origin/main` 同步（只 rebase 自己的分支），推送用 `git push --force-with-lease`。

### 8.2 提交信息

[Conventional Commits](https://www.conventionalcommits.org/)，scope 用 §4 的模块标识，描述用中文或英文均可，但同一 PR 内统一：

```
feat(net): 支持分片下载与断点续传

<为什么这样做，必要时说明取舍>

Refs: M1-01
Agent: claude-a
```

- 类型：`feat`、`fix`、`refactor`、`perf`、`test`、`docs`、`build`、`ci`、`chore`、`contract`。
- 每个提交应能编译、测试通过；一个提交只做一件事。
- 尾注 `Refs:` 和 `Agent:` 必填；工具要求的署名尾注（如 `Co-Authored-By`）照常保留。

### 8.3 PR

- 标题：`[步骤ID] 简短描述`，例如 `[M1-01] 下载引擎`。
- 一个 PR 对应一个步骤或子任务；diff 超过约 800 行（不含生成物和 lock 文件）时应拆分。
- PR 描述必须包含以下各节（模板见 `.github/pull_request_template.md`）：
  1. **步骤**：步骤 ID 与 PLAN.md 链接
  2. **改动**：做了什么，改了哪些路径
  3. **契约变更**：无 / 列出新增或修改的命令、事件、错误码、配置字段
  4. **验证**：跑了哪些命令、在哪些平台和游戏版本上手测，结果如何；对照该步骤的“验收”逐条说明
  5. **未完成与已知问题**
  6. **交接**：下一步、坑点、相关文件（见 §12）
  7. **自查清单**：PLAN.md 第 4 节通用完成标准逐条勾选
- 准备好评审时把 Draft 转为 Ready，并在 Issue 中去掉 `status:claimed` 之外的阻塞标签。

## 9. 每个任务的标准流程

1. 按 §7 认领。
2. 先写测试或 fixture（能离线就离线），再写实现。
3. 本地跑 `pnpm check`（等价于 PLAN.md 第 4 节的全部检查）。
4. 涉及游戏启动的步骤，至少在一个真实游戏版本上手测，并在 PR“验证”里写明版本和平台。
5. 自查 diff：没有越界文件、没有调试输出、没有密钥、没有注释掉的大段代码。
6. 推送并把 PR 转为 Ready。
7. 处理评审意见：每条意见要么改掉，要么回复说明不改的理由。
8. 合并后删除远程分支，在 Issue 下写一句完成说明并关闭。

## 10. 评审与合并

### 10.1 合并条件

- CI 三平台全绿。
- 维护者批准。以下 PR 维护者必须亲自逐行看：`contract-change`、`core/auth/**`、任何涉及令牌/密钥/签名/更新的改动、`.github/workflows/**`、新增第三方依赖。
- 合并方式：Squash merge，合并后的提交信息沿用 §8.2 格式。

### 10.2 评审清单（评审 Agent 和维护者共用）

- [ ] 只改了步骤范围内的文件，分层依赖没有被破坏
- [ ] 满足该步骤的每一条“验收”
- [ ] 错误用 `AppError` + 错误码返回，后端没有返回界面文字
- [ ] 长耗时操作接收 `TaskCtx`，可取消、有进度
- [ ] 没有 `unwrap()` / `expect()` 出现在非测试代码的可失败路径上
- [ ] 日志里没有令牌、密钥、完整的用户主目录路径以外的隐私信息
- [ ] 新依赖许可证兼容 GPL-3.0，且确有必要
- [ ] 测试覆盖了失败路径（网络错误、校验失败、取消）
- [ ] 新增文字已 i18n，中英文案都通顺

评审 Agent 只用 PR 评论提意见，不得推送到被评审的分支，除非 PR 作者在评论中明确请它修改。

## 11. 密钥与隐私

| 名称 | 用途 | 存放 | 谁能看 |
| --- | --- | --- | --- |
| `TMCL_MS_CLIENT_ID` | 微软登录 | 本地 `.env.local`；CI 的 GitHub Secrets | 维护者 |
| `TMCL_CURSEFORGE_API_KEY` | CurseForge API | 同上 | 维护者 |
| `TAURI_SIGNING_PRIVATE_KEY` 及密码 | 更新包签名 | 只在 GitHub Secrets | 维护者 |
| 用户的令牌、樱花访问密钥 | 运行时 | 用户本机系统密钥库 | 用户本人 |

规则：

1. 所有构建期密钥都通过 `build.rs` + `option_env!` 注入，缺失时对应功能禁用而不是编译失败。读取密钥的代码只能放在 `core/auth/build.rs`、`modules/resources/build.rs` 和 `src-tauri` 的签名配置中。
2. Agent 不得读取、打印、复制 `.env.local`。需要知道某个密钥是否配置时，只运行应用的 `app_info` 或检查 `option_env!` 是否为 `Some`。
3. 测试中使用明显的假值，如 `00000000-0000-0000-0000-000000000000`、`test-api-key`。录制的 HTTP 响应在提交前必须去掉令牌、邮箱、真实 UUID 和 Xbox 用户哈希。
4. CI 运行 gitleaks；一旦发现密钥进入提交历史，立即停止工作并通知维护者，由维护者轮换密钥。不要自己尝试改写历史。
5. 日志统一走 `tmcl-common` 的脱敏层；禁止 `println!` / `console.log` 输出请求头或响应体。
6. 诊断信息导出、崩溃报告样本、截图在提交或上传前都要脱敏。

## 12. 交接

任何时候停止工作（会话结束、上下文快满、被阻塞、转交他人），都必须：

1. 把当前进度提交并推送（WIP 提交用 `wip(<scope>): ...`，合并前会被 squash）。
2. 在 PR 描述的“交接”一节更新以下内容（覆盖旧内容，保留历史时间戳）：
   ```
   交接 · 2026-10-22 14:00 · claude-a
   状态：进行中 / 阻塞 / 待评审
   已完成：……
   未完成：……
   下一步：（具体到文件和函数）
   验证方法：（命令）
   坑点：……
   阻塞原因：（如有）
   ```
3. 需要别人接手时加 `status:handoff` 标签。
4. 接手方先读完交接说明和整个 PR diff，在 PR 下评论“已接手：<Agent 名>”，继续在**同一分支**上提交（这是唯一允许在别人分支上提交的情况）；原 Agent 不得再推送该分支。

## 13. 冲突处理

| 情况 | 处理 |
| --- | --- |
| 两个 Agent 认领了同一步骤 | 远程分支先推送成功的一方保留；另一方关闭自己的 PR，把有价值的代码以评论形式留给对方 |
| 需要改别人正在改的文件 | 不要直接改。在对方 PR 下留言说明需求；对方合并后再基于 `main` 继续；紧急时找维护者决定顺序 |
| 合并冲突 | 由**后合并**的 PR 负责解决：`git rebase origin/main`，按 §5.2 处理共享文件，重新跑 `pnpm check` |
| 发现 `main` 上的 bug 不在自己范围内 | 不在当前 PR 里顺手修。开 Issue，或在维护者同意后开独立的 `fix` PR |
| 契约与需求冲突 | 停止相关部分，按 §6.2 提契约 PR，或在 Issue 加 `needs-user` 请维护者决定 |
| 计划与现实不符 | 不要自行修改 PLAN.md。在 Issue 中说明并加 `needs-user` |
| 两个 Agent 对实现方案意见不一 | 各自在 PR / Issue 中写清理由和取舍，由维护者决定，不互相覆盖提交 |

## 14. 第三方依赖与许可证

1. 新增依赖前先查是否已有能用的依赖；优先使用 `[workspace.dependencies]` 中已有的版本。
2. 许可证必须与 GPL-3.0 兼容（MIT、Apache-2.0、BSD、MPL-2.0、LGPL、GPL-3.0 等）。不兼容或不确定的（如 GPL-2.0-only、SSPL、无许可证、自定义条款）一律不引入，并在 PR 中说明。
3. 二进制组件（frpc、authlib-injector、cubiomes 等）在 `docs/contracts/third-party.md` 中登记来源、版本、许可证、校验和、是否随安装包分发。
4. 不引入遥测、广告或会上传用户数据的依赖。

## 15. 合规

- 界面、README、安装包信息中注明“非官方启动器，与 Mojang Studios、Microsoft 无关”。
- 不使用 Minecraft 名称作为产品名的一部分，不使用官方 Logo 与素材。
- 只从官方或其授权镜像下载游戏文件，不修改游戏本体文件。
- 离线账号在界面中明确标注“仅限本地测试”，默认引导正版登录。
- 调用第三方 API 前确认其条款，按要求设置 `User-Agent`、遵守速率限制。

## 16. 阻塞与求助

- 被外部事项阻塞（审核、API Key、第三方授权）：在 Issue 和 PR 中加 `status:blocked`，写明等待什么、由谁处理，然后换一个不依赖它的任务。
- 需要维护者决定：加 `needs-user`，把问题写成可以直接回答的选择题（给出选项、各自后果和你的推荐）。
- 不确定某个改动是否越界：默认视为越界，先问。
