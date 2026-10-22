# IPC 契约：Tauri 命令、事件与类型生成

本文件是受保护的契约文档（AGENTS.md §5.1），对应 [PLAN.md](../PLAN.md) 第 3.2、3.3 节，由 M0-06 建立。**新增**命令或事件可随功能 PR 追加到下表，并在 PR 描述的“契约变更”一栏列出；改名、删除、改类型或改语义须走单独的契约 PR（AGENTS.md §6.2）。

## 1. 分层与文件

| 层 | 文件 | 职责 |
| --- | --- | --- |
| 后端命令 | `src-tauri/src/commands/<模块>.rs` | 参数转换、调用 core / modules，不写业务逻辑 |
| 命令注册 | `src-tauri/src/lib.rs` 的 `handlers()` | `generate_handler!` 内按模块分组、组内字母排序、一行一个 |
| 状态 | `src-tauri/src/state.rs` | `AppState`，命令通过 `State<'_, AppState>` 取用 |
| 事件 | `src-tauri/src/events.rs` | 事件名常量、`spawn_task`、`EventSink` |
| 类型生成 | `src-tauri/src/bindings.rs` | 登记要导出的根类型 |
| 生成物 | `ui/src/bindings/*.ts` | ts-rs 生成，**禁止手改** |
| 前端封装 | `ui/src/api/<模块>.ts` | 每个命令一个函数；页面只通过这里调用后端 |
| 前端基础 | `ui/src/api/invoke.ts`、`ui/src/api/events.ts` | 统一错误规整、事件订阅 |

## 2. 命令

- 命名 `<模块>_<动作>`，蛇形。
- 签名：`async fn xxx(state: State<'_, AppState>, args: XxxArgs) -> Result<XxxResult, AppError>`；无需状态或参数时可省略对应形参。参数统一放在名为 `args` 的结构体里，前端以 `invoke("xxx", { args })` 调用。
- 参数与返回值都 `#[derive(ts_rs::TS)]`，字段保持蛇形（与 serde 一致）；`u64` 等大整数用 `#[ts(type = "number")]`；可省略的字段用 `Option<T>` + `#[serde(default)]` + `#[ts(optional)]`。
- 失败一律返回 `AppError`，不返回面向用户的文字。前端 `invoke()` 的拒绝值保证是 `AppError`：后端错误原样透传，IPC 层错误（命令不存在、无权限、参数反序列化失败）规整为 `COMMON_INTERNAL`，原始信息放入 `detail`。
- 耗时操作（下载、安装、启动）调用 `events::spawn_task`，**立即返回 `TaskStarted { task_id }`**，进度与结果通过事件推送。

| 命令 | 参数 | 返回 | 说明 |
| --- | --- | --- | --- |
| `app_info` | 无 | `AppInfo { version, os, arch, ms_login_enabled }` | 版本号、平台与功能开关 |
| `app_demo_task` | `AppDemoTaskArgs { steps, step_ms, fail_at? }` | `TaskStarted` | 演示任务：`steps` 1–100、`step_ms` ≤ 5000，越界返回 `COMMON_INTERNAL`；走到 `fail_at` 步时以 `failed` 结束 |
| `settings_get_locale` | 无 | `SettingsLocale { locale: string \| null }` | `config.json` 中保存的界面语言；尚无可用的 `config.json`（首次启动或文件损坏）时为 `null`，由前端按系统语言选择后写回 |
| `settings_set_locale` | `SettingsSetLocaleArgs { locale }` | 无 | 写入界面语言，其他设置不变；`locale` 须为由字母、数字、连字符组成的 BCP 47 标签（≤ 35 字符），否则返回 `COMMON_INTERNAL` |

## 3. 事件

| 事件名 | 负载 | 说明 |
| --- | --- | --- |
| `task:progress` | `TaskProgress { task_id, kind, stage, done, total, bytes_done, bytes_total, speed_bps }` | 经 `ThrottledSink` 限流，同一阶段最多每 200 ms 一次；阶段切换、首条和完成总会推送 |
| `task:finished` | `TaskFinished { task_id, state, error? }` | 每个任务恰好一次；`state` 为 `succeeded` \| `failed` \| `cancelled`，仅 `failed` 带 `error` |
| `game:log` | `GameLog { instance_id, pid, stream, lines }` | M1-07 定义类型并追加 |
| `game:exited` | `GameExited { instance_id, pid, exit_code, crashed }` | M1-07 定义类型并追加 |
| `auth:device-code` | `DeviceCode { user_code, verification_uri, expires_in }` | M0-10 定义类型并追加 |

- 事件向所有窗口广播；发送失败只记日志。
- 首条进度可能早于命令返回，前端须**先订阅再调用命令**，并按 `task_id`（或 `kind`）过滤。
- 任务取消：任务函数返回 `AppError::cancelled()` 时结束状态为 `cancelled`。全局任务列表与 `task_cancel` 命令由 M1-10 的 `TaskManager` 提供。

## 4. 类型生成

1. 新增的命令参数、返回值或事件负载类型在 `src-tauri/src/bindings.rs` 的 `export_all` 中登记根类型（依赖类型自动导出）。
2. 运行 `pnpm gen:bindings`，提交 `ui/src/bindings/` 的变化。
3. `cargo test`（因而 `pnpm check` 与 CI）会比对生成结果与仓库文件，不一致即失败，提示运行 `pnpm gen:bindings`。比对忽略换行符差异。
