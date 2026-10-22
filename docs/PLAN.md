# TMCL 实施执行计划

> 版本：v1（2026-10-22）· 依据：[TMCL 项目大纲](../TMCL%20项目大纲.md)、[README](../README.md)
> 协作规则见仓库根目录 [AGENTS.md](../AGENTS.md)。任何人（包括 AI Agent）开始写代码前，必须先读完本文第 1–4 节和 AGENTS.md。

## 0. 已确定的前提

| 事项 | 结论 |
| --- | --- |
| 许可证 | GPL-3.0（已在仓库 `LICENSE`） |
| 技术栈 | Tauri 2 + Rust 核心 + Vue 3 / TypeScript 前端 |
| 游戏范围 | 只支持 Minecraft Java 版，不做基岩版 |
| 界面语言 | 首发简体中文（zh-CN）和英文（en-US），预留扩展机制 |
| 界面风格 | Windows 11 式简约圆润风格；主界面由用户可拖拽、可增删的组件组成；背景可自定义并调节透明度，默认提供亮色与暗色两套背景（透明度可调）。组件库用 Naive UI（主题覆盖实现 Win11 风格），拖拽布局用 grid-layout-plus，窗口材质用 Tauri 的 Mica / Acrylic 效果 |
| 下载镜像 | 未定。M1-02 先实测多个镜像，再由维护者选默认源 |
| 团队 | 个人开发（维护者：JIANYU TANG），可调度多个 AI Agent 并行开发 |
| 微软登录 | Azure Client ID 存于 `.env.local` 的 `TMCL_MS_CLIENT_ID`，构建时注入。2026-10-22 实测 MS / Xbox / XSTS 三步通过，`login_with_xbox` 返回 403，等待 Mojang 的 API 使用审核 |
| P2P 中继服务器 | 挂起，暂不决定。先只做直连 + 可插拔中继接口 |
| SakuraFrp 合作 | 需调研条款后再决定（见 M2-13） |
| 工期 | 每周投入时间未定，本计划只给规模估算（S ≈ 0.5 天，M ≈ 2 天，L ≈ 5 天），不写日期 |

## 1. 阶段总览

开发分四个阶段，每个阶段结束有一道门槛，未通过不进入下一阶段的**发布**（下一阶段的开发可以提前开始，见各门槛说明）。

| 阶段 | 内容 | 门槛 |
| --- | --- | --- |
| M0 原型验证 | 仓库与 CI、基础库、IPC 契约、i18n、密钥注入、端到端下载并启动原版、微软登录 | G1：正版账号能启动游戏 |
| M1 Alpha（P0） | 下载引擎与镜像、版本文件补全、Java 管理、实例管理、启动、账号、加载器、P0 界面、打包 | G2：P0 完成，三平台可安装 |
| M2 Beta（P1） | 整合包与资源浏览、皮肤、设置同步、P2P 直连、樱花穿透（视调研结果） | G3：公测问题收敛 |
| M3 正式版 1.0（P2） | 种子地图、崩溃分析、自动更新、主题与多语言扩展、用户文档、合规复核 | 发布 1.0：文档齐备 |

步骤编号规则：`M<阶段>-<两位序号>`，例如 `M1-03`。编号一经发布不再复用；新增步骤追加序号，废弃步骤标注“已废弃”保留原位。

## 2. 仓库结构（M0-01 建立后即为强制约定）

```
TMCL/
├─ AGENTS.md                 # 多 Agent 协作规则（受保护文件）
├─ CLAUDE.md                 # 一行：请阅读 AGENTS.md
├─ Cargo.toml                # Rust workspace 根
├─ package.json / pnpm-workspace.yaml
├─ rust-toolchain.toml
├─ core/                     # L4 核心层（Rust crates，不依赖 tauri）
│  ├─ common/   (tmcl-common)    错误码、路径、配置、日志脱敏、任务与进度、SQLite
│  ├─ net/      (tmcl-net)       HTTP 客户端、下载引擎、下载源与镜像
│  ├─ game/     (tmcl-game)      版本清单、版本 JSON、依赖库/资源/natives 补全
│  ├─ java/     (tmcl-java)      Java 探测、下载、版本匹配
│  ├─ auth/     (tmcl-auth)      微软 / 离线 / authlib-injector 账号
│  ├─ instance/ (tmcl-instance)  实例 CRUD、导入导出
│  ├─ launch/   (tmcl-launch)    启动参数拼装、进程管理、日志流
│  └─ loader/   (tmcl-loader)    Fabric / Quilt / Forge / NeoForge 安装
├─ modules/                  # L3 功能模块（Rust crates，只依赖 core/*）
│  ├─ resources/ (tmcl-resources) Modrinth / CurseForge 客户端、资源浏览、整合包
│  ├─ skin/      (tmcl-skin)
│  ├─ sync/      (tmcl-sync)      设置与按键同步
│  ├─ net-p2p/   (tmcl-p2p)
│  ├─ net-sakura/(tmcl-sakura)
│  ├─ seed/      (tmcl-seed)      cubiomes FFI
│  └─ crash/     (tmcl-crash)
├─ src-tauri/                # L2 应用壳：Tauri 命令、状态、事件、打包配置
│  └─ src/commands/<模块>.rs
├─ ui/                       # L1 界面：Vue 3 + TS + Vite
│  └─ src/{api,bindings,components,pages,stores,theme,widgets/<组件>,locales/<lang>/<模块>.json}
├─ tests/fixtures/           # 版本 JSON 等测试样本（禁止放游戏本体文件）
├─ scripts/                  # 开发脚本（绑定生成、i18n 校验等）
└─ docs/
   ├─ PLAN.md                # 本文件（受保护文件）
   ├─ contracts/             # 接口契约文档（受保护目录）
   ├─ research/              # 调研报告
   ├─ testing/               # 测试矩阵
   └─ user/                  # 用户文档
```

**分层依赖规则（CI 用 `cargo deny` / 脚本检查）**

1. `ui/` 只通过 `ui/src/api/` 调用 Tauri 命令，不直接访问网络或文件系统（不使用前端 `fetch` 访问外部服务，不使用 fs 插件）。
2. `src-tauri/` 可以依赖 `core/*` 和 `modules/*`，只做参数转换、状态持有和事件转发，不写业务逻辑。
3. `modules/*` 只依赖 `core/*`，模块之间互不依赖。需要共享的能力下沉到 `core/`。
4. `core/*` 之间允许依赖，但不得形成环；`tmcl-common` 不依赖任何内部 crate。
5. `core/*` 和 `modules/*` 不依赖 `tauri`，必须能用 `cargo test` 独立测试。

## 3. 接口契约（M0-05 / M0-06 实现，之后修改需走“契约变更”流程，见 AGENTS.md §6）

### 3.1 错误

```rust
// core/common/src/error.rs
#[derive(Debug, thiserror::Error, Serialize, ts_rs::TS)]
pub struct AppError {
    pub code: ErrorCode,                 // 稳定的机器可读错误码
    pub params: BTreeMap<String, String>,// 供前端 i18n 插值
    pub detail: Option<String>,          // 仅用于日志与“复制详情”，已脱敏
}
```

- `ErrorCode` 是字符串枚举，格式 `<模块>_<原因>`，全大写，例如 `NET_TIMEOUT`、`NET_HASH_MISMATCH`、`AUTH_XSTS_NO_XBOX_ACCOUNT`、`AUTH_MC_FORBIDDEN`、`JAVA_NOT_FOUND`、`LAUNCH_PROCESS_CRASHED`。
- 后端**不返回面向用户的文字**；前端用 `errors.<CODE>` 作为 i18n key 渲染。新增错误码必须同时在 zh-CN、en-US 加文案。
- 所有错误码登记在 `docs/contracts/errors.md`，删除或改名属于契约变更。

### 3.2 Tauri 命令

- 命名：`<模块>_<动作>`，蛇形，例如 `instance_list`、`instance_create`、`game_version_list`、`auth_ms_begin`、`task_cancel`。
- 签名：`async fn xxx(state: State<'_, AppState>, args: XxxArgs) -> Result<XxxResult, AppError>`，参数和返回值都是带 `#[derive(ts_rs::TS)]` 的结构体。
- 耗时操作（下载、安装、启动）**立即返回 `TaskId`**，进度通过事件推送。
- 每个模块的命令写在 `src-tauri/src/commands/<模块>.rs`，在 `lib.rs` 的 `generate_handler!` 里按模块分组、组内按字母排序，一行一个命令。
- TS 类型由 `ts-rs` 生成到 `ui/src/bindings/`（生成物，禁止手改），前端封装写在 `ui/src/api/<模块>.ts`。CI 检查生成物与源码一致。

### 3.3 事件

| 事件名 | 负载 | 说明 |
| --- | --- | --- |
| `task:progress` | `TaskProgress { task_id, kind, stage, done, total, bytes_done, bytes_total, speed_bps }` | 最多每 200 ms 一次 |
| `task:finished` | `TaskFinished { task_id, state: "succeeded" \| "failed" \| "cancelled", error?: AppError }` | 每个任务恰好一次 |
| `game:log` | `GameLog { instance_id, pid, stream: "stdout" \| "stderr", lines: string[] }` | 批量推送 |
| `game:exited` | `GameExited { instance_id, pid, exit_code, crashed: bool }` | |
| `auth:device-code` | `DeviceCode { user_code, verification_uri, expires_in }` | 微软设备码 |

### 3.4 任务与进度（核心层）

```rust
// core/common/src/task.rs
pub struct TaskCtx {
    pub id: TaskId,
    pub cancel: tokio_util::sync::CancellationToken,
    pub progress: Arc<dyn ProgressSink>,   // 测试中可用 NoopSink / VecSink
}
```

核心层与模块层的所有长耗时函数都接收 `&TaskCtx`，并在合适的节点检查 `cancel`。`src-tauri` 负责把 `ProgressSink` 接到 Tauri 事件。

### 3.5 数据目录

```
<data_root>/                 # 默认：系统应用数据目录下的 TMCL；exe 旁有 tmcl.portable 文件时为便携模式
├─ config.json               # 启动器设置（schema 字段做版本迁移）
├─ tmcl.db                   # SQLite：账号元数据、缓存、实例索引
├─ shared/{versions,libraries,assets}/   # 多实例共享的游戏文件
├─ java/<runtime-name>/      # 启动器下载的 Java
├─ instances/<instance-id>/
│  ├─ instance.json
│  └─ game/                  # --gameDir：mods、saves、config、options.txt 等
├─ ui/{layout.json,backgrounds/}   # 主页布局与用户背景图
├─ cache/
└─ logs/                     # 启动器日志（已脱敏），保留 7 天
```

`instance.json`（schema 1）：

```json
{
  "schema": 1,
  "id": "0b6f…（UUID v4）",
  "name": "生存 1.20.1",
  "game_version": "1.20.1",
  "loader": { "kind": "fabric", "version": "0.16.5" },
  "java": { "mode": "auto" },
  "memory": { "min_mb": 1024, "max_mb": 4096 },
  "jvm_args": [],
  "game_args": [],
  "window": { "width": 854, "height": 480, "fullscreen": false },
  "group": null,
  "created_at": "2026-10-22T00:00:00Z",
  "last_played_at": null
}
```

`loader` 为 `null` 表示原版；`kind` 取值 `fabric | quilt | forge | neoforge`；`java.mode` 取值 `auto | path`（`path` 时带 `"path"` 字段）。

### 3.6 敏感数据

- 访问令牌、刷新令牌只存系统密钥库（`keyring` crate，键名 `tmcl:account:<account-id>`），SQLite 只存账号 ID、类型、名称、UUID。
- 日志通过 `tmcl-common` 的脱敏层输出：`access_token`、`refresh_token`、`Authorization`、`x-api-key`、`--accessToken` 后的值一律替换为 `***`。
- 构建期常量：`TMCL_MS_CLIENT_ID`、`TMCL_CURSEFORGE_API_KEY`、`TAURI_SIGNING_PRIVATE_KEY`（仅 CI），规则见 AGENTS.md §11。

## 4. 通用完成标准（每一步都要满足，下文不再重复）

1. `cargo fmt --check`、`cargo clippy --workspace -- -D warnings`、`cargo test --workspace` 通过。
2. 涉及前端时 `pnpm lint`、`pnpm typecheck`、`pnpm test` 通过。
3. 新增的公开函数有文档注释；新增错误码登记 `docs/contracts/errors.md` 并有中英文案。
4. 新增界面文字全部走 i18n，`pnpm i18n:check` 通过（两种语言 key 一致）。
5. 需要联网的测试标记为 `#[ignore]` 或放在 `online-tests` feature 下，默认 CI 不跑；每个联网逻辑至少有一个离线 fixture 测试。
6. 新增第三方依赖的许可证与 GPL-3.0 兼容（`cargo deny check licenses`、`pnpm licenses:check` 通过）。
7. PR 描述按模板填写，含验证方式与交接信息。

---

## 5. M0 原型验证

目标：把仓库、CI、契约、密钥注入和“下载 → 登录 → 启动”的最短链路打通。

### M0-01 仓库脚手架
- **目标**：建立 Tauri 2 + Vue 3 + TS 工程和 Rust workspace，目录与第 2 节完全一致（`modules/` 先放空占位 `README.md`）。
- **文件**：`Cargo.toml`、`package.json`、`pnpm-workspace.yaml`、`src-tauri/**`、`ui/**`、`core/*/Cargo.toml` + `src/lib.rs`（空 crate）、`CLAUDE.md`。
- **输入 → 输出**：现有仓库 → 可编译的空壳应用，窗口标题“TMCL”。
- **验收**：Windows 上 `pnpm install && pnpm tauri dev` 打开窗口；`cargo build --workspace` 通过；`.gitignore` 覆盖所有构建产物。
- **依赖**：无 · **规模**：M

### M0-02 工具链与代码规范
- **目标**：统一格式化、Lint 和版本。
- **文件**：`rust-toolchain.toml`（固定为当时的 stable 版本号）、`rustfmt.toml`、`.cargo/config.toml`、`clippy.toml`、`.editorconfig`、`.nvmrc`（Node 22 LTS）、`ui/eslint.config.js`、`ui/.prettierrc`、`package.json` 的 `packageManager` 字段（pnpm）。
- **输出**：根目录脚本 `pnpm check` 一次跑完 Rust 与前端的格式、Lint、类型检查、测试。
- **验收**：在干净克隆上 `pnpm check` 通过；故意制造格式错误时失败。
- **依赖**：M0-01 · **规模**：S

### M0-03 CI 流水线
- **目标**：每个 PR 在三平台跑检查，手动或打 tag 时出安装包。
- **文件**：`.github/workflows/ci.yml`（matrix：windows-latest、macos-latest、ubuntu-22.04；跑 `pnpm check`、`cargo deny`、gitleaks）、`.github/workflows/build.yml`（`tauri-apps/tauri-action`，产物上传为 artifact）、`deny.toml`。
- **输入**：GitHub Secrets `TMCL_MS_CLIENT_ID`（仅 build.yml 使用；fork 的 PR 拿不到，构建应照常通过且微软登录处于禁用状态）。
- **验收**：一个示例 PR 三平台全绿；`build.yml` 产出 Windows `.exe`/`.msi`、macOS `.dmg`、Linux `.AppImage`/`.deb`。
- **依赖**：M0-02 · **规模**：M

### M0-04 协作模板
- **目标**：让人和 Agent 按同一格式提 Issue 与 PR。
- **文件**：`.github/ISSUE_TEMPLATE/{bug.yml,feature.yml,task.yml}`、`.github/pull_request_template.md`（字段见 AGENTS.md §8.3）、`docs/contracts/README.md`、`docs/labels.md`（标签：`module:*`、`P0/P1/P2`、`status:claimed`、`status:blocked`、`status:handoff`、`contract-change`、`needs-user`）。
- **验收**：在 GitHub 上新建 Issue / PR 能看到模板；标签已创建。
- **依赖**：M0-01 · **规模**：S

### M0-05 tmcl-common 基础库
- **目标**：实现第 3.1、3.4、3.5、3.6 节的公共能力。
- **文件**：`core/common/src/{error.rs,paths.rs,config.rs,task.rs,log.rs,db.rs}`、`core/common/migrations/0001_common_init.sql`、`docs/contracts/{errors.md,data-layout.md}`。
- **输入 → 输出**：无 → `AppError`/`ErrorCode`、`DataPaths::resolve()`（含便携模式）、`Config::load/save`（原子写：先写临时文件再 rename）、`TaskCtx`/`ProgressSink`、`init_logging()`（tracing + 脱敏 + 按天滚动）、`Db::open()`（rusqlite + 迁移）。
- **验收**：单元测试覆盖：便携模式判定、配置损坏时回退默认值并备份原文件、脱敏（含 `--accessToken xxx` 形式）、迁移可重复执行。
- **依赖**：M0-01 · **规模**：M

### M0-06 IPC 契约与类型生成
- **目标**：固定命令、事件、类型生成的机制（第 3.2、3.3 节）。
- **文件**：`src-tauri/src/{lib.rs,state.rs,events.rs,commands/mod.rs,commands/app.rs}`、`scripts/gen-bindings.*`、`ui/src/bindings/`（生成）、`ui/src/api/{invoke.ts,events.ts,app.ts}`、`docs/contracts/ipc.md`。
- **输出**：示例命令 `app_info`（返回版本号、平台、是否启用微软登录）和一个示例任务 `app_demo_task`（模拟进度），前端能显示进度条。
- **验收**：`pnpm gen:bindings` 后无 diff 才能通过 CI；前端调用失败时拿到结构化 `AppError`。
- **依赖**：M0-05 · **规模**：M

### M0-07 i18n 框架
- **目标**：中英双语切换，文案按模块拆分避免多人冲突。
- **文件**：`ui/src/i18n.ts`、`ui/src/locales/{zh-CN,en-US}/{common,errors}.json`、`scripts/i18n-check.*`。
- **输出**：`vue-i18n` 接入；首次启动按系统语言选择，设置里可切换并写入 `config.json` 的 `language`；`pnpm i18n:check` 校验两种语言 key 集合一致、无未使用 key。
- **验收**：切换语言后界面即时更新；删掉任一语言的一个 key 时 CI 失败。
- **依赖**：M0-06 · **规模**：S

### M0-08 构建期密钥注入
- **目标**：`TMCL_MS_CLIENT_ID` 不进仓库、不进日志，缺失时功能优雅降级。
- **文件**：`core/auth/build.rs`、`core/auth/src/config.rs`、`README.md`（开发说明，若需更新）。
- **实现**：`build.rs` 优先读环境变量，其次读仓库根 `.env.local`（`dotenvy`），通过 `cargo:rustc-env` 传入；声明 `cargo:rerun-if-env-changed=TMCL_MS_CLIENT_ID` 和 `cargo:rerun-if-changed=<根>/.env.local`。代码中 `pub fn ms_client_id() -> Option<&'static str>`（用 `option_env!`）。
- **验收**：有 `.env.local` 时 `app_info` 显示微软登录可用；删除后重新构建显示不可用且其他功能正常；`git grep` 与构建日志中不出现 Client ID 的值；gitleaks 规则覆盖 `.env*`。
- **依赖**：M0-06 · **规模**：S

### M0-09 端到端骨架：下载并启动原版
- **目标**：用最小实现跑通“选版本 → 下载 → 离线账号启动”，后续 M1 在同一批 crate 上加固，不写一次性代码。
- **文件**：`core/net/src/{client.rs,download.rs}`（单线程 + SHA1 校验即可）、`core/game/src/{manifest.rs,version.rs,install.rs}`、`core/launch/src/{args.rs,process.rs}`、`core/auth/src/offline.rs`、`core/game/examples/launch_vanilla.rs`。
- **输入 → 输出**：版本号 `1.20.1`、本机 Java 17 路径、离线用户名 → 游戏进入主菜单。
- **验收**：`cargo run -p tmcl-launch --example launch_vanilla -- 1.20.1 --java <路径> --name Dev` 在 Windows 上进入主菜单；第二次运行不重复下载已校验文件。
- **依赖**：M0-05 · **规模**：L

### M0-10 微软正版登录全链路
- **目标**：设备码登录拿到 Minecraft 令牌和档案，令牌安全存储。
- **文件**：`core/auth/src/{microsoft.rs,xbox.rs,minecraft.rs,store.rs}`、`docs/contracts/errors.md`（追加 AUTH_*）。
- **流程**：`login.microsoftonline.com/consumers/oauth2/v2.0/devicecode`（scope `XboxLive.signin offline_access`）→ 轮询 token → `user.auth.xboxlive.com/user/authenticate` → `xsts.auth.xboxlive.com/xsts/authorize`（RelyingParty `rp://api.minecraftservices.com/`）→ `api.minecraftservices.com/authentication/login_with_xbox` → `/entitlements/mcstore` → `/minecraft/profile`。XSTS 错误 2148916233、2148916238 分别映射为 `AUTH_XSTS_NO_XBOX_ACCOUNT`、`AUTH_XSTS_CHILD_ACCOUNT`；`login_with_xbox` 的 403 映射为 `AUTH_MC_APP_NOT_APPROVED`。
- **输出**：`MicrosoftAccount { id, name, uuid }`，令牌写入 keyring；`refresh()` 用刷新令牌续期。
- **验收**：前三步在真实账号上通过（已于 2026-10-22 验证过一次，需用新代码复测）；`login_with_xbox` 在审核通过前返回 `AUTH_MC_APP_NOT_APPROVED` 并在界面给出说明；审核通过后整条链路拿到档案；日志中无令牌。所有 HTTP 交互有基于录制响应的离线测试。
- **依赖**：M0-05、M0-08 · **规模**：M · **外部阻塞**：Mojang 审核（维护者跟进）

### 门槛 G1：正版账号能启动游戏
- 检查人：维护者。条件：用 M0-10 的正版账号运行 M0-09 示例进入主菜单，并能进入单人世界。
- **审核未通过时的规则**：允许以“离线账号启动 + 微软登录前三步通过”作为 G1 临时通过，M1 正常开工；但 G2（Alpha 发布）前必须完成正版启动的复测。

---

## 6. M1 Alpha（P0）

M1 可以拆成四条并行泳道（详见第 10 节）：**下载**（M1-01→02→03→04）、**运行时**（M1-05、M1-06）、**账号与加载器**（M1-08、M1-09）、**界面**（M1-11 起，先用 mock 数据）。

### M1-01 下载引擎
- **目标**：稳定、可取消、可续传的并发下载。
- **文件**：`core/net/src/{client.rs,download.rs,chunk.rs,retry.rs,progress.rs}`。
- **输入 → 输出**：`Vec<DownloadItem { url, path, size?, sha1? }>` + `TaskCtx` → 全部文件落盘并校验，或返回第一个不可恢复错误。
- **要求**：全局并发数可配置（默认 16）；> 8 MB 且服务器支持 Range 的文件分片下载；`.part` 临时文件 + 完成后 rename；断点续传；按 1 s、2 s、4 s 指数退避重试 3 次；已存在且 SHA1 匹配的文件跳过；进度聚合为字节数与速度；`User-Agent: TMCL/<版本>`。
- **验收**：本地测试服务器（`wiremock` 或 `axum`）覆盖：中途断开后续传、校验失败重下、取消后无残留 `.part` 以外的文件、并发上限生效。
- **依赖**：M0-09 · **规模**：L

### M1-02 下载源与镜像
- **目标**：官方源与国内镜像可切换，失败自动回退。
- **文件**：`core/net/src/source.rs`、`core/common/src/config.rs`（追加 `download.source` 字段，属于契约变更）。
- **设计**：`trait DownloadSource { fn name(&self) -> &str; fn rewrite(&self, url: &Url) -> Option<Url>; }`；先实现 `Official` 和候选镜像（如 BMCLAPI `bmclapi2.bangbang93.com`，以及测试时仍可用的其他公开镜像），镜像可通过配置增删；策略 `auto | official | <镜像名>`，`auto` 先默认镜像后官方。镜像返回的文件同样校验 SHA1，不一致时回退官方源。
- **镜像实测**：写 `core/net/examples/bench_sources.rs`，对每个候选源测同一组文件（版本清单、1.20.1 版本 JSON 与 client.jar、100 个资源文件、Forge/Fabric 库）的成功率、平均速度、首字节延迟、SHA1 一致率，分别在国内和海外网络各跑三次，结果写入 `docs/research/mirrors.md`，同时核对各镜像的使用条款与 User-Agent 要求。**默认源由维护者看完报告后决定**，决定前 `auto` 暂按官方源处理。
- **验收**：URL 改写表有单元测试（版本清单、版本 JSON、client.jar、libraries、assets、Forge/Fabric maven）；模拟镜像 404/校验失败时自动回退；测速报告已提交。
- **依赖**：M1-01 · **规模**：M

### M1-03 版本清单与版本 JSON
- **目标**：正确解析所有历史版本的元数据。
- **文件**：`core/game/src/{manifest.rs,version.rs,rules.rs,arguments.rs}`、`tests/fixtures/versions/*.json`。
- **要点**：`piston-meta.mojang.com/mc/game/version_manifest_v2.json`，按 `release | snapshot | old_beta | old_alpha` 分类并缓存（SQLite，1 小时过期）；版本 JSON 支持 `inheritsFrom` 合并、`rules`（os 名称/架构/版本、features）、旧式 `minecraftArguments` 与新式 `arguments.{game,jvm}`、`javaVersion`、`logging.client`。
- **验收**：fixture 至少包含 a1.0.4、b1.7.3、1.7.10、1.12.2、1.16.5、1.20.1、最新正式版、一个快照，以及 Fabric/Forge 继承版本；对每个 fixture 断言解析出的库数量与参数。
- **依赖**：M0-09 · **规模**：M

### M1-04 游戏文件补全
- **目标**：根据版本 JSON 下载并校验 client.jar、依赖库、natives、资源文件、日志配置。
- **文件**：`core/game/src/{install.rs,libraries.rs,natives.rs,assets.rs}`。
- **要点**：1.19 以前的 `natives` + `classifiers` 方式与 LWJGL3 新方式都要处理；natives 解压到每次启动的临时目录并遵守 `extract.exclude`；资源索引支持 `legacy`/`pre-1.6`（`map_to_resources`、`virtual`）；提供 `verify(instance)`（只校验不下载）和 `repair(instance)`。
- **输出**：`InstallReport { downloaded, skipped, bytes }`。
- **验收**：在三平台分别补全 1.7.10、1.12.2、1.20.1、最新正式版并启动到主菜单；删除任意一个库文件后 `repair` 能补回。
- **依赖**：M1-02、M1-03 · **规模**：L

### M1-05 Java 管理
- **目标**：自动选出或下载合适的 Java。
- **文件**：`core/java/src/{detect.rs,probe.rs,runtime.rs,select.rs}`。
- **要点**：探测来源：`JAVA_HOME`、`PATH`、Windows 注册表（JavaSoft / Eclipse Adoptium / Microsoft）、常见安装目录、启动器自己下载的 `java/`；用 `release` 文件或运行 `java -XshowSettings:properties -version` 获取版本与架构。下载使用 Mojang 官方启动器的 java-runtime 清单（URL 在实现时从官方元数据核实并写入 `docs/contracts/endpoints.md`），失败时回退 Adoptium API。选择规则：版本 JSON 有 `javaVersion.majorVersion` 用它；没有的旧版本用 8；用户手动指定时只给警告不阻止。
- **输出**：`JavaInstall { path, major, version, arch, vendor, managed }`。
- **验收**：在无 Java 的干净 Windows 虚拟机上启动 1.20.1 时自动下载 Java 17，1.12.2 自动下载 Java 8；架构不匹配（32 位 Java）给出 `JAVA_ARCH_MISMATCH`。
- **依赖**：M1-01 · **规模**：M

### M1-06 实例管理
- **目标**：实现第 3.5 节的实例模型。
- **文件**：`core/instance/src/{model.rs,store.rs,import_export.rs}`、`core/common/migrations/0002_instance_index.sql`。
- **功能**：创建、列表（读 SQLite 索引，启动时与目录对账）、读取、修改、复制（复制 `game/`，可选是否含存档）、重命名、删除（先移到回收站目录，7 天后清理）、导出为 `.tmclpack`（zip：`instance.json` + 选中的 `game/` 子目录）、导入；`schema` 字段做迁移。
- **验收**：手动删改实例目录后重启，列表与磁盘一致；导出再导入后启动成功；`instance.json` 损坏时该实例标记为“损坏”而不是让列表崩溃。
- **依赖**：M0-05 · **规模**：M

### M1-07 启动器
- **目标**：根据实例 + 账号 + Java 拼装命令并管理游戏进程。
- **文件**：`core/launch/src/{args.rs,classpath.rs,placeholders.rs,process.rs,precheck.rs}`。
- **要点**：替换 `${auth_player_name}`、`${version_name}`、`${game_directory}`、`${assets_root}`、`${assets_index_name}`、`${auth_uuid}`、`${auth_access_token}`、`${user_type}`、`${version_type}`、`${natives_directory}`、`${launcher_name}`、`${launcher_version}`、`${classpath}` 等；内存参数、自定义 JVM/游戏参数、窗口大小；启动前检查（文件 verify、Java 匹配、账号令牌有效或刷新）；进程 stdout/stderr 按行转发，退出码 ≠ 0 或日志含崩溃特征时 `crashed = true`；支持强制结束。拼装出的命令写日志时令牌脱敏。
- **验收**：M1-04 的四个版本都能启动；从界面强制结束后进程确实退出；日志中搜索不到真实令牌。
- **依赖**：M1-04、M1-05、M1-06 · **规模**：L

### M1-08 账号管理
- **目标**：多账号与三种账号类型。
- **文件**：`core/auth/src/{accounts.rs,offline.rs,authlib.rs,yggdrasil.rs}`、`core/common/migrations/0003_accounts.sql`。
- **功能**：账号列表、设默认、删除（同时删 keyring 条目）；离线账号 UUID 与原版服务端一致（`UUID.nameUUIDFromBytes("OfflinePlayer:" + name)`，MD5 v3），用户名校验 `^[A-Za-z0-9_]{3,16}$`，界面标注“仅限本地测试”；authlib-injector：输入皮肤站 API 地址 → 获取元数据 → Yggdrasil 登录/刷新 → 启动时下载并校验 `authlib-injector.jar`，添加 `-javaagent:<jar>=<api>` 与 `-Dauthlibinjector.yggdrasil.prefetched=<base64 元数据>`。
- **验收**：离线 UUID 与已知样本一致（单元测试）；用 LittleSkin 测试账号能登录并在游戏内显示皮肤；删除账号后 keyring 中无残留。
- **依赖**：M0-10 · **规模**：M

### M1-09 模组加载器安装
- **目标**：Fabric、Quilt、Forge、NeoForge 一键安装，并自动列出与游戏版本匹配的加载器版本。
- **文件**：`core/loader/src/{fabric.rs,quilt.rs,forge.rs,neoforge.rs,processors.rs,list.rs}`。
- **要点**：Fabric `meta.fabricmc.net/v2/versions/loader/<game>/<loader>/profile/json`，Quilt `meta.quiltmc.org/v3/...`，直接写入继承版本 JSON；Forge（`maven.minecraftforge.net`，推荐版本来自 `promotions_slim.json`）与 NeoForge（`maven.neoforged.net`）下载安装器 jar，解析 `install_profile.json`，下载其依赖库并在本地 Java 上依次运行 `processors`（处理 `{SIDE}`、`{MINECRAFT_JAR}`、`data` 变量和输出 SHA1 校验）。支持范围：Forge 1.7.10–最新（1.12.2 及以前走旧格式安装器分支），更早版本不支持并给出 `LOADER_UNSUPPORTED_VERSION`。
- **验收**：在 1.20.1 上分别安装四种加载器并启动到主菜单；Forge 1.12.2、1.16.5 也能启动；安装失败时实例保持安装前状态。
- **依赖**：M1-04、M1-05 · **规模**：L

### M1-10 Tauri 命令层
- **目标**：把 M1-01 至 M1-09 暴露给前端。
- **文件**：`src-tauri/src/commands/{game,java,instance,launch,auth,loader,task,settings}.rs`、`src-tauri/src/state.rs`、`ui/src/api/*.ts`、`docs/contracts/ipc.md`。
- **要求**：严格遵守第 3.2、3.3 节；一个全局 `TaskManager` 管理所有任务（列表、取消、完成后保留 50 条历史）；应用退出时提示仍在运行的任务。
- **验收**：`docs/contracts/ipc.md` 中列出的每个命令都有前端封装和至少一个集成测试（`tauri::test` mock runtime）。
- **依赖**：M0-06；各命令依赖对应 core 步骤，可按模块分多个 PR · **规模**：L

### M1-11 界面外壳
- **目标**：应用框架、导航、主题基础与设置页。
- **文件**：`ui/src/{App.vue,router.ts,layouts/,stores/settings.ts,pages/settings/}`。
- **选型**：Naive UI 组件库、Pinia、vue-router；左侧导航：首页 / 实例 / 下载 / 账号 / 联机（M2 显示）/ 设置；“非官方启动器，与 Mojang、Microsoft 无关”说明放在“关于”页。
- **Win11 风格**：`ui/src/theme/fluent.ts` 用 Naive UI 的 `themeOverrides` 统一设定：圆角（控件 4 px、卡片与对话框 8 px）、Segoe UI Variable / 系统字体栈（中文回退微软雅黑 / 苹方）、Fluent 配色与强调色（默认跟随系统强调色，取不到时用 `#0067C0`）、轻阴影与 1 px 半透明描边、悬停与按下的柔和过渡；自绘标题栏（`decorations: false` + 拖拽区域 + 最小化/最大化/关闭按钮，Win11 样式）。
- **窗口材质**：Windows 11 上用 Tauri `windowEffects` 的 Mica（不可用时 Acrylic），macOS 用 vibrancy，Linux 与 Windows 10 回退为纯色背景；材质与 M1-19 的自定义背景互斥，由用户在设置里选择。
- **设置项**：语言、主题（亮 / 暗 / 跟随系统）、强调色、下载源、并发数、数据目录（只读显示 + 打开文件夹）、默认内存、Java 列表。
- **验收**：窗口最小 960×600 布局不溢出；深浅色跟随系统；在 Win11 上与系统设置、资源管理器的视觉对比截图附在 PR 中；所有文字已 i18n。
- **依赖**：M0-07 · **规模**：M

### M1-18 可自定义主界面（组件与拖拽布局）
- **目标**：首页由组件（widget）组成，用户可在编辑模式下添加、删除、拖动、缩放组件，布局持久保存。
- **文件**：`ui/src/pages/home/`、`ui/src/widgets/{registry.ts,types.ts,WidgetFrame.vue}`、`ui/src/widgets/<组件名>/`、`ui/src/stores/layout.ts`、`src-tauri/src/commands/layout.rs`、`docs/contracts/widgets.md`。
- **机制**：
  - 布局引擎用 `grid-layout-plus`（Vue 3、MIT）：12 列栅格，组件吸附对齐、自动避让；编辑模式下可拖动和拉伸，非编辑模式锁定。
  - 组件契约：`defineWidget({ id, titleKey, icon, defaultSize: {w,h}, minSize, maxSize, settingsSchema?, component })`，每个组件独立目录、独立 i18n 文件 `locales/<lang>/widget-<id>.json`；`registry.ts` 一行登记一个组件（字母排序）。组件只能通过 `ui/src/api/` 取数据。
  - 布局数据存 `<data_root>/ui/layout.json`：`{ "schema": 1, "widgets": [{ "uid", "type", "x", "y", "w", "h", "settings": {} }] }`，经 `layout_get` / `layout_save` 命令读写（UI 不直接写文件）；损坏或含未知组件时跳过该项并保留其他组件。
  - 编辑模式：首页右上角“编辑”按钮进入；顶部出现“添加组件”（组件库面板，带预览）、“恢复默认布局”、“完成”；每个组件右上角有设置和删除按钮；`Esc` 取消本次修改。
- **首批组件**：开始游戏（最近实例 + 启动按钮）、实例快捷列表、当前账号、下载任务、新闻/更新日志（只读本项目 Releases）、快捷操作（打开游戏目录、新建实例等）。M2 之后各模块可追加组件（如皮肤预览、联机房间、种子地图）。
- **验收**：拖动、拉伸、增删组件后重启布局不变；窗口缩放到最小 960×600 时布局自动重排不重叠；“恢复默认布局”可用；新增一个组件只需新增一个目录并在 `registry.ts` 加一行。
- **依赖**：M1-11、M1-10（`layout_*` 命令） · **规模**：L

### M1-19 自定义背景与透明度
- **目标**：用户可自定义背景，并调节界面透明度；默认提供亮色和暗色两套背景。
- **文件**：`ui/src/theme/background.ts`、`ui/src/components/app-background/`、`ui/src/pages/settings/appearance/`、`ui/src/assets/backgrounds/{light,dark}.*`（原创或可商用许可的图片，许可登记到 `docs/contracts/third-party.md`）、`src-tauri/src/commands/appearance.rs`。
- **功能**：
  - 背景来源：默认（跟随亮/暗主题自动切换对应背景）、纯色、本地图片（复制到 `<data_root>/ui/backgrounds/`，支持 png/jpg/webp）、窗口材质（Mica / Acrylic，见 M1-11）。
  - 可调参数：背景透明度（0–100%）、背景模糊（0–40 px）、遮罩亮度、组件卡片透明度（0–100%，卡片用半透明底 + `backdrop-filter` 毛玻璃）、图片填充方式（填充 / 适应 / 居中 / 平铺）。默认背景的透明度同样可调。
  - 设置写入 `config.json` 的 `appearance` 段（属于契约追加，需在 PR 中列出），实时预览，提供“恢复默认”。
  - 自动保证可读性：卡片透明度低于阈值时给文字加描边阴影；亮/暗主题切换时背景与卡片色同步切换。
- **验收**：四种背景来源在 Win11、macOS、Linux 上都能正确显示与回退；滑块调整实时生效且 60 fps 无明显卡顿；重启后设置保留；图片被删除时回退默认背景并提示。
- **依赖**：M1-11 · **规模**：M

### M1-12 界面：版本下载与任务中心
- **文件**：`ui/src/pages/download/`、`ui/src/components/tasks/`、`ui/src/stores/tasks.ts`。
- **功能**：版本列表（类型筛选、搜索）、选择后填写实例名与加载器（调用 M1-15 组件）一键创建并下载；任务中心显示进度、速度、取消、失败重试、错误详情复制。
- **验收**：从点击到开始下载 ≤ 3 次点击；网络断开时任务显示可读的错误并可重试。
- **依赖**：M1-10、M1-11 · **规模**：M

### M1-13 界面：实例与启动
- **文件**：`ui/src/pages/instances/`、`ui/src/components/game-log/`。
- **功能**：实例卡片列表（分组、最近游玩排序）、详情页（版本、加载器、Java、内存、参数、窗口、打开游戏目录）、复制/重命名/删除/导入/导出、启动按钮、游戏日志窗口（实时滚动、搜索、导出）、崩溃时弹出提示。
- **验收**：首页“开始游戏”一键启动最近实例；启动失败的错误显示为本地化文案。
- **依赖**：M1-10、M1-11 · **规模**：L

### M1-14 界面：账号
- **文件**：`ui/src/pages/accounts/`、`ui/src/components/device-code-dialog/`。
- **功能**：账号列表与切换；微软登录弹窗显示设备码、复制按钮、打开浏览器、倒计时、自动轮询；离线账号创建（带用途提示）；外置登录（输入皮肤站地址）。未配置 Client ID 时微软登录按钮禁用并说明原因。
- **验收**：三种账号都能从界面添加并用于启动；`AUTH_MC_APP_NOT_APPROVED` 有清楚的说明文案。
- **依赖**：M1-08、M1-10 · **规模**：M

### M1-15 界面：加载器选择组件
- **文件**：`ui/src/components/loader-picker/`。
- **功能**：选择游戏版本后列出可用加载器及其版本（默认推荐版本），不兼容的灰显；已有实例可后装/更换加载器（提示会修改实例）。
- **验收**：四种加载器在 1.20.1 上都能从界面装好并启动。
- **依赖**：M1-09、M1-10 · **规模**：S

### M1-16 打包与发布配置
- **目标**：三平台可安装的 Alpha 包。
- **文件**：`src-tauri/tauri.conf.json`（bundle、标识符 `io.github.<用户名>.tmcl`）、`src-tauri/icons/`（原创图标，不使用 Mojang 素材）、`ui/src/pages/about/`、`scripts/gen-licenses.*`。
- **功能**：Windows NSIS 安装包（中英双语安装界面）、macOS dmg、Linux AppImage + deb；“关于”页显示版本、GPL-3.0、第三方许可证列表（`cargo-about` + 前端依赖许可证汇总）。
- **验收**：在三个平台的干净虚拟机上安装、启动、卸载；macOS 未签名时 README 写明打开方式。
- **依赖**：M0-03 · **规模**：M

### M1-17 Alpha 测试与修复
- **文件**：`docs/testing/alpha-matrix.md`。
- **内容**：平台（Win10、Win11、macOS arm64、Ubuntu 22.04）× 版本（a1.0.4、1.7.10、1.12.2、1.16.5、1.20.1、最新）× 加载器 × 账号类型的测试矩阵，逐项记录结果；所有 P0 缺陷修复或登记为已知问题。
- **依赖**：M1-07 至 M1-16、M1-18、M1-19 · **规模**：M

### 门槛 G2：P0 完成，三平台可安装
- 测试矩阵无 P0 级未修复缺陷；正版账号启动已复测通过（见 G1 规则）；打 tag `v0.1.0-alpha.1`，走 `build.yml` 发布到 GitHub Releases 的预发布。

---

## 7. M2 Beta（P1）

### M2-01 Modrinth 客户端
- **文件**：`modules/resources/src/modrinth/{client.rs,models.rs}`。
- **要点**：`api.modrinth.com/v2`；必须设置 `User-Agent: <GitHub 用户名>/TMCL/<版本>`；遵守速率限制（读响应头 `X-Ratelimit-*`，超限时等待）；搜索、项目、版本、依赖、按文件哈希查询（用于更新检查）。
- **验收**：基于录制响应的离线测试；联网测试覆盖搜索与下载。
- **依赖**：M1-01 · **规模**：M

### M2-02 CurseForge 客户端
- **文件**：`modules/resources/src/curseforge/{client.rs,models.rs}`、`modules/resources/build.rs`。
- **前置**：维护者申请 CurseForge API Key，以 `TMCL_CURSEFORGE_API_KEY` 按 M0-08 同样方式注入；没有 key 时 CurseForge 来源隐藏。
- **要点**：`api.curseforge.com/v1`，请求头 `x-api-key`；处理作者禁止第三方下载的文件（`downloadUrl` 为空）：提示用户手动下载并提供打开网页按钮。
- **验收**：同 M2-01；无 key 构建时功能隐藏且无报错。
- **依赖**：M1-01 · **规模**：M · **外部阻塞**：API Key

### M2-03 资源浏览与安装
- **文件**：`modules/resources/src/{browse.rs,install.rs,deps.rs,update.rs}`、`src-tauri/src/commands/resources.rs`。
- **功能**：统一搜索 mod / 资源包 / 光影 / 数据包；按实例的游戏版本与加载器过滤；安装时解析必需前置并一起安装，冲突（`incompatible`）安装前提示；已装资源列表（读 `game/mods` 等目录，按哈希反查来源）、启用/禁用（`.disabled` 后缀）、检查更新、一键更新。
- **验收**：在 Fabric 1.20.1 实例中搜索并安装 Sodium、带前置的 mod；更新检查能发现旧版本。
- **依赖**：M2-01（M2-02 可后补） · **规模**：L

### M2-04 整合包安装
- **文件**：`modules/resources/src/modpack/{mrpack.rs,curseforge.rs,install.rs}`。
- **功能**：`.mrpack`（`modrinth.index.json`、`overrides/`、`client-overrides/`，校验文件哈希）、CurseForge 整合包 zip（`manifest.json`、`overrides`）；从 URL / 本地文件 / 搜索结果安装；自动创建实例、安装对应加载器、并行下载 mod；安装失败回滚（删除新实例）。
- **验收**：各选一个热门整合包安装并进入主菜单。
- **依赖**：M2-03、M1-09 · **规模**：L

### M2-05 整合包导出
- **文件**：`modules/resources/src/modpack/export.rs`。
- **功能**：把实例导出为 `.mrpack`：能在 Modrinth 找到的 mod 写成下载链接，其余文件放 `overrides/`；用户勾选包含哪些目录（默认不含存档、截图、日志）。
- **验收**：导出的包能被 TMCL 和 Modrinth 官方 App 导入。
- **依赖**：M2-04 · **规模**：M

### M2-06 界面：资源与整合包
- **文件**：`ui/src/pages/resources/`、`ui/src/pages/instances/<详情>/resources-tab`、`ui/src/locales/*/resources.json`。
- **验收**：从搜索到安装 ≤ 3 次点击；已装列表可启用/禁用/删除/更新。
- **依赖**：M2-03、M2-04、M2-05 · **规模**：L

### M2-07 皮肤后端
- **文件**：`modules/skin/src/{mojang.rs,yggdrasil.rs,library.rs}`、`src-tauri/src/commands/skin.rs`。
- **功能**：正版：读取档案皮肤与披风、上传皮肤（`POST api.minecraftservices.com/minecraft/profile/skins`，模型 classic/slim）、切换披风（`PUT /minecraft/profile/capes/active`）；外置：跳转皮肤站；本地皮肤库（`<data_root>/skins/`）保存历史皮肤；PNG 校验（64×64 或 64×32）。
- **验收**：正版账号上传后游戏内生效；非法图片返回 `SKIN_INVALID_IMAGE`。
- **依赖**：M1-08 · **规模**：M

### M2-08 界面：皮肤预览
- **文件**：`ui/src/pages/skin/`、`ui/src/components/skin-viewer/`。
- **功能**：skinview3d 3D 预览（拖拽旋转、缩放、走路/奔跑/挥手动作、Steve/Alex 切换、披风预览）；本地皮肤库；拖入 PNG 即预览。
- **验收**：在三平台 WebView 上渲染正常，帧率 ≥ 30。
- **依赖**：M2-07 · **规模**：M

### M2-09 设置同步后端
- **文件**：`modules/sync/src/{options.rs,keymap.rs,profile.rs,apply.rs}`、`modules/sync/data/keymap-lwjgl2-glfw.json`。
- **功能**：解析/写回 `options.txt`（保持未知行与顺序）；按键：1.13 以后为 `key.keyboard.w` 形式，1.12.2 及以前为 LWJGL2 数字键码，用映射表双向转换，鼠标键单独处理，模组按键原样保留；分类：按键 / 视频 / 音频 / 语言 / 其他；“主配置”保存在 `<data_root>/sync/master.json`；应用前备份目标 `options.txt` 到 `options.txt.tmcl-bak-<时间>`。
- **验收**：1.20.1 → 1.12.2 → 1.20.1 往返后按键一致；映射表覆盖全部标准键（单元测试遍历）。
- **依赖**：M1-06 · **规模**：M

### M2-10 界面：设置同步
- **文件**：`ui/src/pages/sync/`。
- **功能**：选主配置来源（某实例或手动编辑）、勾选同步类别、勾选目标实例、预览差异、执行、撤销（恢复最近备份）。
- **依赖**：M2-09 · **规模**：M

### M2-11 P2P 联机调研
- **目标**：在“中继挂起”的前提下选定直连方案。
- **产出**：`docs/research/p2p.md`：比较至少三种方案（例如 EasyTier、自研 STUN + UDP 打洞 + KCP/QUIC、libp2p），维度：NAT 穿透成功率、是否需要自建服务器（信令/STUN/中继各自的最低需求与费用）、许可证兼容性、Rust 集成方式、安全性（加密、只转发游戏端口）。结论给出推荐与“中继服务器”需要维护者决定的具体问题。
- **验收**：维护者阅读并在报告末尾的决策表中勾选。
- **依赖**：无（可提前开展） · **规模**：M

### M2-12 P2P 直连实现
- **文件**：`modules/net-p2p/src/{room.rs,signal.rs,punch.rs,tunnel.rs,relay.rs}`、`src-tauri/src/commands/p2p.rs`。
- **功能**：房主开放局域网世界后，启动器读取端口（日志或手动输入）→ 生成房间码（含随机密钥）→ 成员输入房间码 → 打洞建立加密通道 → 成员本地监听一个端口转发到房主游戏端口，并提示“直接连接 127.0.0.1:<端口>”。`trait Relay` 定义但默认无实现；直连失败返回 `P2P_DIRECT_FAILED` 并建议使用樱花穿透。
- **验收**：两台处于不同家庭宽带的电脑能联机；只转发游戏端口（安全测试）。
- **依赖**：M2-11 决策 · **规模**：L

### M2-13 SakuraFrp 调研
- **产出**：`docs/research/sakurafrp.md`：核对 SakuraFrp 的服务条款、API 文档与使用限制、frpc 客户端再分发许可、是否需要申请合作或 API 授权、是否有官方推荐的第三方集成方式；列出可选方案（A：内置下载官方 frpc + 用户自己的访问密钥调用 API；B：只引导用户使用官方启动器；C：申请官方合作）。
- **验收**：维护者决定采用的方案；如需联系樱花官方，由维护者本人发送，Agent 只起草。
- **依赖**：无（可提前开展） · **规模**：S

### M2-14 樱花穿透实现
- **文件**：`modules/net-sakura/src/{api.rs,frpc.rs,tunnel.rs}`、`src-tauri/src/commands/sakura.rs`。
- **功能**（按方案 A 编写，若决策不同则修订本步骤）：用户填写自己的访问密钥（存 keyring）→ 拉取节点与隧道列表 → 一键为当前局域网端口创建 TCP 隧道 → 下载并校验官方 frpc → 启动 frpc 子进程并解析日志 → 显示并复制联机地址 → 关闭时删除临时隧道。
- **依赖**：M2-13 决策 · **规模**：M

### M2-15 界面：联机页
- **文件**：`ui/src/pages/multiplayer/`。
- **功能**：一个页面两个选项“直连”与“樱花穿透”；房主 / 成员两种流程；状态与延迟显示；故障提示。
- **依赖**：M2-12 或 M2-14 任一完成即可开始 · **规模**：M

### M2-16 Beta 公测
- **文件**：`docs/testing/beta-plan.md`、`.github/ISSUE_TEMPLATE/bug.yml`（追加“导出诊断信息”说明）。
- **功能**：设置页“导出诊断信息”（启动器日志 + 系统信息 + 实例列表，全部脱敏）；发布 `v0.2.0-beta.N`；每周整理反馈。
- **依赖**：M2-01 至 M2-15 中已决定实施的步骤 · **规模**：M

### 门槛 G3：公测问题收敛
- 连续两个 Beta 版本没有新的 P0/P1 级缺陷；所有 P1 模块要么完成，要么由维护者明确推迟。

---

## 8. M3 正式版 1.0（P2）

### M3-01 种子地图后端
- **文件**：`modules/seed/{build.rs,src/ffi.rs,src/map.rs,src/structures.rs}`、`modules/seed/vendor/cubiomes`（git submodule，MIT）。
- **功能**：`cc` 编译 cubiomes；按游戏版本选择生成规则；生成指定区域的生物群系瓦片（PNG，按缩放级别缓存）和结构坐标（村庄、要塞、神殿、海底遗迹等）；种子支持数字与字符串（Java `String.hashCode`）。
- **验收**：用公开已知种子比对 1.12.2、1.18、最新版本若干结构坐标。
- **依赖**：M0-05 · **规模**：L

### M3-02 界面：种子地图
- **文件**：`ui/src/pages/seed-map/`。
- **功能**：可拖拽缩放的地图（瓦片懒加载）、结构图层开关、坐标显示、跳转坐标、从实例存档读取种子。
- **依赖**：M3-01 · **规模**：M

### M3-03 崩溃日志分析
- **文件**：`modules/crash/src/{rules.rs,analyze.rs}`、`modules/crash/rules/*.json`。
- **功能**：读取 `crash-reports/` 与最新日志，按规则库匹配常见原因（Java 版本不对、内存不足、缺少前置、mod 冲突、显卡驱动、OpenGL 错误等），给出本地化的原因和建议；规则用 JSON 描述，社区可贡献。
- **验收**：每条规则有至少一个真实日志样本测试（样本脱敏后放 `tests/fixtures/crash/`）。
- **依赖**：M1-07 · **规模**：M

### M3-04 自动更新
- **文件**：`src-tauri/src/updater.rs`、`tauri.conf.json`（updater）、`.github/workflows/release.yml`。
- **功能**：`tauri-plugin-updater`，正式版 / 测试版两个通道；更新清单放 GitHub Releases，并支持一个国内镜像地址（镜像来源由维护者决定）；签名私钥 `TAURI_SIGNING_PRIVATE_KEY` 只存在 GitHub Secrets。
- **验收**：从 `v0.9.0` 自动更新到 `v0.9.1` 成功；签名不符时拒绝更新。
- **依赖**：M1-16 · **规模**：M

### M3-05 主题与语言扩展
- **文件**：`ui/src/theme/`、`ui/src/locales/`、`docs/contributing-translations.md`、`docs/contributing-widgets.md`。
- **功能**：在 M1-11 / M1-19 的基础上支持导入导出“外观方案”（主题、强调色、背景、透明度、主页布局打包为一个 `.tmcltheme` 文件）；新增语言只需添加一个 `locales/<lang>/` 目录并登记到 `ui/src/locales/index.ts`；编写社区开发新组件的文档。
- **依赖**：M1-11 · **规模**：S

### M3-06 用户文档
- **文件**：`docs/user/{zh-CN,en-US}/*.md`。
- **内容**：安装、首次使用、账号（含为什么需要正版、离线模式用途）、实例与加载器、整合包、皮肤、设置同步、联机、常见问题、诊断信息导出。
- **依赖**：各功能完成 · **规模**：M

### M3-07 合规复核与 1.0 发布
- **文件**：`docs/compliance.md`、`README.md`。
- **检查表**：不分发游戏文件；不使用 Mojang 商标与 Logo；所有界面与 README 注明“非官方”；离线模式标注；CurseForge、Modrinth、BMCLAPI、SakuraFrp、authlib-injector 等第三方条款逐条核对；第三方许可证列表完整；隐私说明（启动器收集什么、不收集什么）。
- **验收**：维护者签核检查表；打 tag `v1.0.0`。
- **依赖**：G3 · **规模**：S

---

## 9. 外部事项与负责人

这些事只能由维护者本人处理，Agent 只能起草材料，不得代为提交或联系对方。

| 事项 | 影响步骤 | 状态 |
| --- | --- | --- |
| Mojang Minecraft API 使用审核（Azure 应用） | M0-10、G1、G2 | 等待中（2026-10-22 起） |
| CurseForge API Key 申请 | M2-02 | 未开始 |
| SakuraFrp 条款核对 / 合作沟通 | M2-13、M2-14 | 待调研 |
| P2P 中继服务器部署与费用 | M2-12 | 挂起 |
| 代码签名证书（Windows / macOS） | M1-16、M3-04 | 未决定，Alpha 先不签名 |
| GitHub Secrets 配置 | M0-03、M2-02、M3-04 | 未开始 |

## 10. 依赖关系与并行泳道

```
M0-01 → M0-02 → M0-03
   ├→ M0-04
   └→ M0-05 → M0-06 → M0-07
          │       └→ M0-08 → M0-10 ─┐
          └→ M0-09 ─────────────────┴→ G1

下载泳道:   M1-01 → M1-02 → M1-03* → M1-04 ─┐      (*M1-03 只依赖 M0-09，可与 M1-01 并行)
运行时泳道: M1-05（需 M1-01）、M1-06       ─┼→ M1-07 → M1-17 → G2
账号/加载器: M1-08；M1-09（需 M1-04、M1-05）─┘
界面泳道:   M1-11 →（mock 数据）M1-12/13/14/15、M1-18、M1-19 → 接入 M1-10
发布泳道:   M1-16（需 M0-03）

M2 资源:   M2-01 → M2-03 → M2-04 → M2-05 → M2-06；M2-02 并行
M2 皮肤:   M2-07 → M2-08        M2 同步: M2-09 → M2-10
M2 联机:   M2-11 → [决策] → M2-12；M2-13 → [决策] → M2-14；→ M2-15
M3:       M3-01 → M3-02；M3-03；M3-04；M3-05；M3-06 → M3-07
```

同一时间建议最多 **3 个** Agent 并行，且分属不同泳道；界面泳道可以先用 `ui/src/api/__mocks__/` 中的 mock 数据开发，接口对齐后再切换。

## 11. 计划的维护

- 本文件是受保护文件：只有维护者，或得到维护者明确同意的 PR，才能修改步骤的目标、验收和依赖。
- Agent 完成步骤后**不在本文件里打勾**，进度以 GitHub Issue / PR 状态为准，避免多人同时改这个文件产生冲突。
- 发现计划与现实不符（接口不存在、依赖不对、规模严重低估）时，在对应 Issue 下说明并加 `needs-user` 标签，由维护者修订本文件。
