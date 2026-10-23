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
| `game_install` | `GameInstallArgs { version_id }` | `TaskStarted` | 任务种类 `game_install`：下载并校验该版本的版本 JSON、资源索引、依赖库与资源文件，阶段依次为 `version_json`、`asset_index`、`libraries`、`assets`；`version_id` 为空、超过 64 字符或含 `/`、`\`、`:`、`..` 时直接返回 `COMMON_INTERNAL`；版本不存在时任务以 `GAME_VERSION_NOT_FOUND` 失败 |
| `game_version_list` | 无 | `GameVersionList { latest_release, latest_snapshot, versions: GameVersion { id, kind, release_time }[] }` | 官方版本清单，新版本在前；`kind` 为 `release` \| `snapshot` \| `old_beta` \| `old_alpha` \| `other` |
| `instance_create` | `InstanceCreateArgs { name, game_version, loader? }` | `InstanceConfig` | 新建实例（只写 `instance.json` 与空的 `game/`，不下载游戏文件）；名称非法返回 `INSTANCE_INVALID_NAME` |
| `instance_delete` | `InstanceIdArgs { id }` | 无 | 把实例目录移到回收站 `trash/`，7 天后清理；损坏的实例也可删除 |
| `instance_duplicate` | `InstanceDuplicateArgs { id, name, include_saves }` | `TaskStarted` | 任务种类 `instance_duplicate`：复制为新实例（新 ID），`include_saves` 为 `false` 时跳过 `game/saves/`；完成后重新调用 `instance_list` 获取新实例 |
| `instance_export` | `InstanceExportArgs { id, dest, include }` | `TaskStarted` | 任务种类 `instance_export`：导出为 `.tmclpack`，`include` 为要包含的 `game/` 顶层条目名；`dest` 须为绝对路径，否则直接返回 `COMMON_INTERNAL` |
| `instance_game_entries` | `InstanceIdArgs { id }` | `InstanceGameEntries { entries }` | `game/` 下的顶层条目名（按名称排序），供导出时选择 |
| `instance_get` | `InstanceIdArgs { id }` | `InstanceConfig` | 读取 `instance.json`；不存在返回 `INSTANCE_NOT_FOUND`，损坏返回 `INSTANCE_CORRUPTED` / `INSTANCE_SCHEMA_UNSUPPORTED` |
| `instance_import` | `InstanceImportArgs { pack }` | `TaskStarted` | 任务种类 `instance_import`：从 `.tmclpack` 导入为新实例；`pack` 须为绝对路径；包无效时任务以 `INSTANCE_PACK_INVALID` 失败 |
| `instance_list` | 无 | `InstanceList { instances: InstanceSummary[] }` | 读取 SQLite 实例索引：最近启动的在前，从未启动的按创建时间倒序；损坏的实例 `error` 非空。应用首次访问数据库时先与磁盘对账并清理过期回收站 |
| `instance_refresh` | 无 | `InstanceList` | 先与 `instances/` 目录重新对账，再返回同 `instance_list` |
| `instance_rename` | `InstanceRenameArgs { id, name }` | `InstanceConfig` | 只改显示名称，ID 与目录不变 |
| `instance_save` | `InstanceSaveArgs { config: InstanceConfig }` | `InstanceConfig` | 保存修改后的完整配置（实例须已存在），返回规整后的配置（名称去掉首尾空白） |
| `settings_get` | 无 | `SettingsSnapshot { settings: LauncherSettings, data_dir, config_backup }` | 读取 `config.json` 中除 `schema`、`language` 外的设置：`LauncherSettings { theme, appearance, memory, download }`，字段同 [data-layout.md](data-layout.md) 第 3 节；`data_dir` 为数据目录绝对路径；`config.json` 损坏时按读取规则备份并回退默认值，`config_backup` 给出备份路径，否则为 `null` |
| `settings_get_language` | 无 | `SettingsLanguage { language: string \| null }` | `config.json` 中保存的界面语言；尚无可用的 `config.json`（首次启动或文件损坏）时为 `null`，由前端按系统语言选择后写回 |
| `settings_open_data_dir` | 无 | 无 | 在系统文件管理器中打开数据目录（不存在时先创建） |
| `settings_set` | `SettingsSetArgs { settings: LauncherSettings }` | `LauncherSettings` | 整体保存设置，界面语言不变，返回保存后的设置；`appearance.accent` 须为 `#RRGGBB` 或 `null`、`download.max_concurrency` 1–64、`memory.max_mb` 512–32768，否则返回 `COMMON_INTERNAL`（`params` 中对应项为 `false`）；下载设置变化时重建下载引擎，已在运行的任务不受影响 |
| `settings_set_language` | `SettingsSetLanguageArgs { language }` | 无 | 写入界面语言，其他设置不变；`language` 须为由字母、数字、连字符组成的 BCP 47 标签（≤ 35 字符），否则返回 `COMMON_INTERNAL` |
| `task_cancel` | `TaskCancelArgs { task_id }` | `TaskCancelResult { cancelled }` | 请求取消运行中的任务，任务随后以 `task:finished`（`cancelled`）结束；任务不存在或已结束时 `cancelled` 为 `false`，不报错 |
| `task_list` | 无 | `TaskList { tasks: TaskInfo[] }` | 全局任务列表：运行中的按开始时间先后在前，随后是最近结束的至多 50 条。`TaskInfo { task_id, kind, status, started_at, finished_at, progress, error }`，`status` 为 `running` \| `succeeded` \| `failed` \| `cancelled`，时间为 unix 毫秒，`progress` 为最近一次推送的 `TaskProgress` |

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
- 任务取消：任务函数返回 `AppError::cancelled()` 时结束状态为 `cancelled`。所有经 `spawn_task` 启动的任务都登记在全局 `TaskManager` 中，见 `task_list`、`task_cancel`。

## 4. 类型生成

1. 新增的命令参数、返回值或事件负载类型在 `src-tauri/src/bindings.rs` 的 `export_all` 中登记根类型（依赖类型自动导出）。
2. 运行 `pnpm gen:bindings`，提交 `ui/src/bindings/` 的变化。
3. `cargo test`（因而 `pnpm check` 与 CI）会比对生成结果与仓库文件，不一致即失败，提示运行 `pnpm gen:bindings`。比对忽略换行符差异。
