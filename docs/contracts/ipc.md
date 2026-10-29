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
| `app_releases` | 无 | `ReleaseList { releases: ReleaseNote { tag, name, published_at, prerelease, url }[] }` | 本项目 GitHub Releases 中最近的至多 10 个版本（不含草稿），按发布时间从新到旧；1 小时内使用 SQLite `kv` 表中的缓存（键 `app.releases`），获取失败时退回旧缓存，没有缓存则返回 `NET_*` 或 `COMMON_JSON` |
| `appearance_import_background` | `AppearanceImportArgs { path }` | `AppearanceImported { image }` | 把用户选中的图片复制到 `ui/backgrounds/`，文件名为 `bg-<unix 毫秒>.<png\|jpg\|webp>`（按文件头识别格式）；不是 png/jpg/webp 或超过 32 MB 返回 `APPEARANCE_IMAGE_UNSUPPORTED`；同时删除除新图片与 `config.json` 当前所用图片外的旧背景图。不修改 `config.json`，由前端随后经 `settings_set` 写入 `appearance.background.image` |
| `appearance_read_background` | `AppearanceImageArgs { image }` | 二进制（前端为 `ArrayBuffer`） | 读取背景图片内容；文件不存在或文件名不是启动器生成的格式时返回 `APPEARANCE_IMAGE_MISSING` |
| `app_demo_task` | `AppDemoTaskArgs { steps, step_ms, fail_at? }` | `TaskStarted` | 演示任务：`steps` 1–100、`step_ms` ≤ 5000，越界返回 `COMMON_INTERNAL`；走到 `fail_at` 步时以 `failed` 结束 |
| `auth_add_authlib` | `AuthAddAuthlibArgs { api_url, username, password }` | `Account` | 外置登录：读取皮肤站并用 Yggdrasil 登录，令牌存系统密钥库，账号设为默认；角色有多个且未选定时用第一个。密码不保存。错误：`AUTH_AUTHLIB_SERVER_INVALID`、`AUTH_YGGDRASIL_INVALID_CREDENTIALS`、`AUTH_YGGDRASIL_NO_PROFILE`、`AUTH_SECRET_STORE`、`NET_*` |
| `auth_add_offline` | `AuthAddOfflineArgs { name }` | `Account` | 添加离线账号（仅限本地测试）并设为默认；UUID 与原版离线 UUID 一致，同名账号已存在时返回该账号；用户名不合法返回 `AUTH_OFFLINE_INVALID_NAME` |
| `auth_authlib_server` | `AuthAuthlibServerArgs { api_url }` | `AuthlibServer { api_url, name }` | 读取皮肤站名称，并按 `X-Authlib-Injector-API-Location` 规整 API 地址（输入缺少协议时补 `https://`）；不是皮肤站返回 `AUTH_AUTHLIB_SERVER_INVALID` |
| `auth_list` | 无 | `AccountList { accounts: Account { id, kind, name, uuid, server? }[], default_id? }` | 按添加顺序列出账号；`kind` 为 `microsoft` \| `offline` \| `authlib`；`default_id` 为启动时使用的账号，列表为空时为 `null` |
| `auth_ms_begin` | 无 | `TaskStarted` | 任务种类 `auth_ms`：微软设备码登录。拿到设备码后推送 `auth:device-code`，按设备码给出的间隔轮询，用户完成登录后依次换取 Xbox Live、XSTS 与 Minecraft 令牌并读取档案；令牌存系统密钥库，账号新增或更新并设为默认，任务成功（前端随后重新读取 `auth_list`）。取消任务即放弃登录。构建时没有 Client ID 时命令直接返回 `AUTH_MS_DISABLED`；任务错误：`AUTH_MS_DECLINED`、`AUTH_MS_CODE_EXPIRED`、`AUTH_XSTS_NO_XBOX_ACCOUNT`、`AUTH_XSTS_CHILD_ACCOUNT`、`AUTH_XSTS_DENIED`、`AUTH_MC_APP_NOT_APPROVED`、`AUTH_MC_NOT_OWNED`、`AUTH_MC_NO_PROFILE`、`AUTH_SECRET_STORE`、`NET_*` |
| `auth_ms_open_verification` | `AuthMsOpenArgs { uri }` | 无 | 在系统浏览器中打开 `auth:device-code` 给出的验证页；只接受 `microsoft.com`、`live.com` 及其子域名下的 https 地址，否则返回 `COMMON_INTERNAL`（`params.uri`） |
| `auth_remove` | `AuthIdArgs { id }` | 无 | 删除账号及其密钥库中的令牌；删除默认账号后默认改为列表第一个；不存在返回 `AUTH_ACCOUNT_NOT_FOUND` |
| `auth_set_default` | `AuthIdArgs { id }` | 无 | 设为启动时使用的账号；不存在返回 `AUTH_ACCOUNT_NOT_FOUND` |
| `game_install` | `GameInstallArgs { version_id }` | `TaskStarted` | 任务种类 `game_install`：下载并校验该版本的版本 JSON、资源索引、依赖库与资源文件，阶段依次为 `version_json`、`asset_index`、`libraries`、`assets`；`version_id` 为空、超过 64 字符或含 `/`、`\`、`:`、`..` 时直接返回 `COMMON_INTERNAL`；版本不存在时任务以 `GAME_VERSION_NOT_FOUND` 失败 |
| `game_version_list` | `GameVersionListArgs { refresh }`（可省略，等同 `refresh: false`） | `GameVersionList { latest_release, latest_snapshot, versions: GameVersion { id, kind, release_time }[], stale }` | 官方版本清单，新版本在前；`kind` 为 `release` \| `snapshot` \| `old_beta` \| `old_alpha` \| `other`。1 小时内使用缓存，`refresh` 为真时强制联网；联网失败但有旧缓存时返回旧缓存，`stale` 为获取时的 `AppError`，否则为 `null` |
| `instance_create` | `InstanceCreateArgs { name, game_version, loader? }` | `InstanceConfig` | 新建实例（只写 `instance.json` 与空的 `game/`，不下载游戏文件）；名称非法返回 `INSTANCE_INVALID_NAME` |
| `instance_delete` | `InstanceIdArgs { id }` | 无 | 把实例目录移到回收站 `trash/`，7 天后清理；损坏的实例也可删除 |
| `instance_duplicate` | `InstanceDuplicateArgs { id, name, include_saves }` | `TaskStarted` | 任务种类 `instance_duplicate`：复制为新实例（新 ID），`include_saves` 为 `false` 时跳过 `game/saves/`；完成后重新调用 `instance_list` 获取新实例 |
| `instance_export` | `InstanceExportArgs { id, dest, include }` | `TaskStarted` | 任务种类 `instance_export`：导出为 `.tmclpack`，`include` 为要包含的 `game/` 顶层条目名；`dest` 须为绝对路径，否则直接返回 `COMMON_INTERNAL` |
| `instance_game_entries` | `InstanceIdArgs { id }` | `InstanceGameEntries { entries }` | `game/` 下的顶层条目名（按名称排序），供导出时选择 |
| `instance_get` | `InstanceIdArgs { id }` | `InstanceConfig` | 读取 `instance.json`；不存在返回 `INSTANCE_NOT_FOUND`，损坏返回 `INSTANCE_CORRUPTED` / `INSTANCE_SCHEMA_UNSUPPORTED` |
| `instance_import` | `InstanceImportArgs { pack }` | `TaskStarted` | 任务种类 `instance_import`：从 `.tmclpack` 导入为新实例；`pack` 须为绝对路径；包无效时任务以 `INSTANCE_PACK_INVALID` 失败 |
| `instance_list` | 无 | `InstanceList { instances: InstanceSummary[] }` | 读取 SQLite 实例索引：最近启动的在前，从未启动的按创建时间倒序；损坏的实例 `error` 非空。应用首次访问数据库时先与磁盘对账并清理过期回收站 |
| `instance_open_dir` | `InstanceIdArgs { id }` | 无 | 在系统文件管理器中打开实例的 `game/` 目录（不存在时先创建）；损坏的实例也可打开；实例目录不存在或 ID 非法返回 `INSTANCE_NOT_FOUND` |
| `instance_refresh` | 无 | `InstanceList` | 先与 `instances/` 目录重新对账，再返回同 `instance_list` |
| `instance_rename` | `InstanceRenameArgs { id, name }` | `InstanceConfig` | 只改显示名称，ID 与目录不变 |
| `instance_save` | `InstanceSaveArgs { config: InstanceConfig }` | `InstanceConfig` | 保存修改后的完整配置（实例须已存在），返回规整后的配置（名称去掉首尾空白） |
| `launch_kill` | `LaunchInstanceArgs { instance_id }` | `LaunchKillResult { killed }` | 强制结束该实例的游戏进程，进程退出后照常推送 `game:exited`（`crashed` 为 `false`）；未在运行或仍在启动前准备中时 `killed` 为 `false`，不报错 |
| `launch_running` | 无 | `LaunchRunning { games: RunningGame { instance_id, pid }[] }` | 运行中的游戏（按实例 ID 排序），不含仍在准备中的实例；供界面重新打开后恢复状态 |
| `launch_start` | `LaunchStartArgs { instance_id, account: { kind: "offline", name } }` | `TaskStarted` | 任务种类 `launch`：校验并补全游戏文件（阶段 `verify`，需要时 `version_json`…`assets`）、选择或下载 Java、解压 natives，创建游戏进程后任务成功并记录 `last_played_at`；之后推送 `game:log` / `game:exited`。实例不存在返回 `INSTANCE_NOT_FOUND`，用户名不合法返回 `AUTH_OFFLINE_INVALID_NAME`，该实例已在启动或运行返回 `LAUNCH_ALREADY_RUNNING`。`account` 为离线账号 `{ kind: "offline", name }`（仅限本地测试），或已保存的账号 `{ kind: "account", id }`（M1-08，不存在返回 `AUTH_ACCOUNT_NOT_FOUND`）；外置登录账号在任务中先校验或刷新令牌（失效且无法刷新时以 `AUTH_YGGDRASIL_SESSION_EXPIRED` 失败），再下载并校验 authlib-injector，加上 `-javaagent` 与 `prefetched` 参数；微软账号（M0-10）的 Minecraft 令牌剩余不足 10 分钟时先用刷新令牌续期并写回密钥库（刷新令牌失效时以 `AUTH_MS_SESSION_EXPIRED` 失败），以 `--userType msa` 启动 |
| `layout_get` | 无 | `LayoutSnapshot { widgets: WidgetPlacement[] \| null, skipped, backup }` | 读取 `<data_root>/ui/layout.json`，格式见 [widgets.md](widgets.md)：从未保存过时 `widgets` 为 `null`；字段缺失、越界或 `uid` 重复的组件被跳过并计入 `skipped`；整个文件无法使用（不是 JSON、`schema` 不是 1、结构不对）时改名为 `layout.json.corrupt-<unix 毫秒>` 备份，`widgets` 为 `null`、`backup` 给出备份路径 |
| `layout_save` | `LayoutSaveArgs { widgets: WidgetPlacement[] }` | 无 | 整体替换保存的布局（原子写入）；任一组件不合法（`uid` 或 `type` 格式不对、`w`/`h` 为 0、`x + w > 12`、`y + h > 256`）、`uid` 重复或超过 64 个组件时返回 `COMMON_INTERNAL`，文件不变 |
| `loader_install` | `LoaderInstallArgs { instance_id, kind, version }` | `TaskStarted` | 任务种类 `loader_install`：在实例上安装或更换加载器，必要时先补全原版（阶段 `version_json`…`assets`，随后 `loader_profile` 或 `loader_installer`、`loader_libraries`、`loader_processors`）；Forge / NeoForge 的处理步骤使用实例的 Java 设置。安装成功后才把实例的 `loader` 改为 `{ kind, version }`，失败或取消时实例不变。实例不存在返回 `INSTANCE_NOT_FOUND`；版本号为空、超过 64 字符或含 `/`、`\`、`:`、`?`、`#`、`..` 时返回 `COMMON_INTERNAL`；加载器版本不存在时任务以 `LOADER_VERSION_NOT_FOUND` 失败。改回原版用 `instance_save` 把 `loader` 置为 `null` |
| `loader_list` | `LoaderListArgs { kind, game_version }` | `LoaderVersionList { versions: LoaderVersion { kind, game_version, version, stable, recommended }[] }` | 该加载器在此游戏版本上可安装的版本，新版本在前；`recommended` 至多一个（Forge 为官方推荐，其余为最新正式版）。Fabric、Quilt、NeoForge 不支持时返回空列表，Forge 不支持 1.7.10 以前的版本时返回 `LOADER_UNSUPPORTED_VERSION`；下载源启用镜像时 Forge、NeoForge、Fabric 的列表经 BMCLAPI 获取 |
| `resources_check_updates` | `ResourcesDirArgs { instance_id, kind, world? }` | `ResourceUpdateList { updates: ResourceUpdate { file_name, project_id, title, current_version_id, current_version_number, latest: ResourceVersion }[] }` | 对目录中能按 SHA1 在 Modrinth 找到来源的文件（含已禁用的），用 `POST /version_files/update` 按实例过滤（资源包按 `minecraft`、光影按 `iris`/`optifine`/`canvas`/`vanilla` 加载器）找最新版本，只报告发布时间晚于当前版本的；网络错误返回 `NET_*` |
| `resources_install` | `ResourcesInstallArgs { instance_id, kind, version_id, world? }` | `TaskStarted` | 任务种类 `resources_install`：重新生成安装计划（同 `resources_install_plan`），有冲突时以 `RESOURCES_INCOMPATIBLE`、有缺失的前置时以 `RESOURCES_DEPENDENCY_UNAVAILABLE` 失败且不写文件；否则在阶段 `resources` 下载并校验全部文件，成功后删除被替换的同项目旧文件。实例不存在直接返回 `INSTANCE_NOT_FOUND` |
| `resources_install_plan` | `ResourcesInstallArgs` | `InstallPlan { files: PlannedFile { project_id, version_id, title, version_number, kind, file_name, size, required_by, replaces }[], conflicts: Conflict { project_id, title, with_project_id, with_title, with_installed }[], unavailable: UnavailableDependency { project_id, title, required_by }[] }` | 只读：解析 Modrinth 版本的必需前置（递归；已安装的项目按项目跳过；未指定版本时选适用于实例的最新正式版），并按 `mods/` 与目标目录中文件的 SHA1 反查已安装资源，报告与已安装或同批资源的 `incompatible` 冲突。`kind` 决定目录：`mod` → `game/mods/`，`resourcepack` → `game/resourcepacks/`，`shader` → `game/shaderpacks/`，`datapack` → `game/saves/<world>/datapacks/`（存档须已存在，否则 `RESOURCES_WORLD_NOT_FOUND`）。版本不存在或没有文件返回 `RESOURCES_VERSION_NOT_FOUND` |
| `resources_installed` | `ResourcesDirArgs` | `InstalledList { resources: InstalledResource { file_name, enabled, size, sha1, source: ResourceSource { project_id, version_id, version_number, title, icon_url } \| null }[], lookup_error }` | 列出该类资源目录（`kind` 与目录的对应同 `resources_install_plan`）中的 `.jar` / `.zip` 文件（含 `.disabled`），按文件名排序，用 `POST /version_files` 按 SHA1 反查来源；反查失败（如断网）时 `source` 全为 `null`、`lookup_error` 为该错误，列表照常返回 |
| `resources_remove` | `ResourcesRemoveArgs { instance_id, kind, world?, file_name }` | 无 | 删除资源文件；文件不存在、不是资源文件或文件名含路径时返回 `RESOURCES_FILE_NOT_FOUND` |
| `resources_search` | `ResourcesSearchArgs { instance_id, kind, query, sort?, offset?, limit? }` | `ResourcePage { hits: ResourceHit { project_id, slug, title, description, author, icon_url, downloads, follows, categories, date_modified }[], offset, limit, total_hits }` | 在 Modrinth 搜索 `kind`（`mod` \| `resourcepack` \| `shader` \| `datapack`）类项目，按实例游戏版本过滤；`mod` 另按实例加载器过滤（Quilt 实例同时接受 Fabric，原版不过滤），`datapack` 按 `datapack` 加载器过滤。`sort` 为 `relevance`（默认）\| `downloads` \| `follows` \| `newest` \| `updated`；`limit` 默认 20、最多 100 |
| `resources_set_enabled` | `ResourcesSetEnabledArgs { instance_id, kind, world?, file_name, enabled }` | `ResourcesSetEnabledResult { file_name }` | 启用（去掉 `.disabled` 后缀）或禁用（加上后缀），返回新文件名；已是目标状态时不改动；文件不存在返回 `RESOURCES_FILE_NOT_FOUND`，目标文件名已存在返回 `COMMON_IO` |
| `resources_update` | `ResourcesUpdateArgs { instance_id, kind, world?, file_names? }` | `TaskStarted` | 任务种类 `resources_update`：重新检查更新（同 `resources_check_updates`），只更新 `file_names` 中列出的文件（省略时全部），在阶段 `resources` 下载并校验新文件后删除旧文件；已禁用的资源更新后仍带 `.disabled`。新版本新增的必需前置不会自动安装 |
| `resources_versions` | `ResourcesVersionsArgs { instance_id, kind, project_id }` | `ResourceVersionList { versions: ResourceVersion { id, project_id, name, version_number, channel, date_published, downloads, game_versions, loaders, file_name, file_size }[] }` | 项目中适用于该实例的版本（过滤规则同 `resources_search`），新版本在前；`channel` 为 `release` \| `beta` \| `alpha`；没有文件的版本被跳过，项目不存在时为空列表 |
| `resources_worlds` | `ResourcesWorldsArgs { instance_id }` | `ResourceWorldList { worlds }` | 实例 `game/saves/` 下的存档目录名，按名称排序，供选择数据包的安装目标；实例不存在返回 `INSTANCE_NOT_FOUND` |
| `settings_get` | 无 | `SettingsSnapshot { settings: LauncherSettings, data_dir, config_backup }` | 读取 `config.json` 中除 `schema`、`language` 外的设置：`LauncherSettings { theme, appearance, memory, download }`，字段同 [data-layout.md](data-layout.md) 第 3 节；`data_dir` 为数据目录绝对路径；`config.json` 损坏时按读取规则备份并回退默认值，`config_backup` 给出备份路径，否则为 `null` |
| `settings_get_language` | 无 | `SettingsLanguage { language: string \| null }` | `config.json` 中保存的界面语言；尚无可用的 `config.json`（首次启动或文件损坏）时为 `null`，由前端按系统语言选择后写回 |
| `settings_open_data_dir` | 无 | 无 | 在系统文件管理器中打开数据目录（不存在时先创建） |
| `settings_set` | `SettingsSetArgs { settings: LauncherSettings }` | `LauncherSettings` | 整体保存设置，界面语言不变，返回保存后的设置；`appearance.accent` 须为 `#RRGGBB` 或 `null`、`download.max_concurrency` 1–64、`memory.max_mb` 512–32768、`appearance.background` 各项在下表范围内（`background`），否则返回 `COMMON_INTERNAL`（`params` 中对应项为 `false`）；下载设置变化时重建下载引擎，已在运行的任务不受影响 |
| `settings_set_language` | `SettingsSetLanguageArgs { language }` | 无 | 写入界面语言，其他设置不变；`language` 须为由字母、数字、连字符组成的 BCP 47 标签（≤ 35 字符），否则返回 `COMMON_INTERNAL` |
| `skin_file_read` | `SkinFileArgs { path }` | 二进制（前端为 `ArrayBuffer`） | 读取用户拖入的 PNG 供预览，不加入皮肤库；校验规则同 `skin_library_import`（不是 PNG、尺寸不是 64×64 / 64×32 或超过 1 MB 返回 `SKIN_INVALID_IMAGE`），文件读不到返回 `COMMON_IO`（`params.path`） |
| `skin_library_import` | `SkinImportArgs { path, name?, variant }` | `SkinEntry { id, name, variant, height, added_at }` | 把用户选中的 PNG 加入本地皮肤库 `skins/`；`variant` 为 `classic` \| `slim`，`name` 省略时用文件名（去掉首尾空白后 1–64 字符，否则 `COMMON_INTERNAL`）；不是 PNG、尺寸不是 64×64 / 64×32 或超过 1 MB 返回 `SKIN_INVALID_IMAGE`；同一张图片（SHA-256 相同）已在库中时返回已有条目 |
| `skin_library_list` | 无 | `SkinLibraryList { skins: SkinEntry[] }` | 本地皮肤库，最近加入的在前；`id` 为 PNG 内容的 SHA-256，`added_at` 为 unix 毫秒 |
| `skin_library_read` | `SkinIdArgs { id }` | 二进制（前端为 `ArrayBuffer`） | 读取皮肤 PNG；不存在返回 `SKIN_NOT_FOUND` |
| `skin_library_remove` | `SkinIdArgs { id }` | 无 | 从皮肤库删除皮肤及其图片；不存在返回 `SKIN_NOT_FOUND` |
| `skin_library_update` | `SkinUpdateArgs { id, name, variant }` | `SkinEntry` | 修改名称与模型；不存在返回 `SKIN_NOT_FOUND`，名称不合法返回 `COMMON_INTERNAL` |
| `skin_open_site` | `SkinAccountArgs { account_id }` | `SkinSite { url }` | 在系统浏览器中打开外置登录账号的皮肤站页面（同 `skin_site_url`），返回打开的地址 |
| `skin_profile` | `SkinAccountArgs { account_id }` | `SkinProfile { id, name, skins: ProfileSkin { id, url, variant, active }[], capes: ProfileCape { id, alias, url, active }[] }` | 正版账号档案中的皮肤与披风（`GET /minecraft/profile`）；没有档案返回 `AUTH_MC_NO_PROFILE`。令牌剩余不足 10 分钟时先续期并写回密钥库；账号不存在返回 `AUTH_ACCOUNT_NOT_FOUND`，不是微软账号返回 `COMMON_INTERNAL`（`params.kind`），令牌失效或被拒（401）返回 `AUTH_MS_SESSION_EXPIRED`（`params.name`），其余 `NET_*` |
| `skin_set_cape` | `SkinSetCapeArgs { account_id, cape_id? }` | `SkinProfile` | 显示指定披风（`PUT /minecraft/profile/capes/active`），`cape_id` 省略或为 `null` 时隐藏披风（`DELETE`）；返回新档案。令牌剩余不足 10 分钟时先续期并写回密钥库；账号不存在返回 `AUTH_ACCOUNT_NOT_FOUND`，不是微软账号返回 `COMMON_INTERNAL`（`params.kind`），令牌失效或被拒（401）返回 `AUTH_MS_SESSION_EXPIRED`（`params.name`），其余 `NET_*` |
| `skin_site_url` | `SkinAccountArgs { account_id }` | `SkinSite { url }` | 外置登录账号的皮肤站页面：API 元数据的 `meta.links.homepage`，没有或读取失败时为 API 地址的站点根；只返回 http(s) 且不含 `&`、引号等字符的地址。账号不存在返回 `AUTH_ACCOUNT_NOT_FOUND`，不是外置登录账号返回 `COMMON_INTERNAL`（`params.kind`） |
| `skin_texture` | `SkinTextureArgs { url }` | 二进制（前端为 `ArrayBuffer`） | 下载档案中的皮肤或披风图片供预览：只接受 `http(s)://textures.minecraft.net/texture/<十六进制>`（不带端口与查询串），一律改用 HTTPS 请求；其它地址返回 `COMMON_INTERNAL`（`params.url`），内容不是 PNG 或超过 1 MB 返回 `SKIN_INVALID_IMAGE`，其余 `NET_*` |
| `skin_upload` | `SkinUploadArgs { account_id, skin_id }` | `SkinProfile` | 把皮肤库中的皮肤上传到正版账号并设为当前皮肤（`POST /minecraft/profile/skins`，模型取该皮肤的 `variant`），返回新档案；皮肤不存在返回 `SKIN_NOT_FOUND`，服务器拒收图片返回 `SKIN_INVALID_IMAGE`。令牌剩余不足 10 分钟时先续期并写回密钥库；账号不存在返回 `AUTH_ACCOUNT_NOT_FOUND`，不是微软账号返回 `COMMON_INTERNAL`（`params.kind`），令牌失效或被拒（401）返回 `AUTH_MS_SESSION_EXPIRED`（`params.name`），其余 `NET_*` |
| `sync_apply` | `SyncTargetsArgs { instance_ids, categories }` | `SyncApplyResult { targets: SyncTargetApplied { instance_id, changed, backup }[] }` | 把主配置中选中类别（`SyncCategory`：`keys` \| `video` \| `audio` \| `language` \| `other`）的选项写入各实例的 `game/options.txt`，按键与语言代码按实例游戏版本转换（1.13 / 17w43a 起为键名，更早为 LWJGL2 键码）；有修改时先把原文件备份为 `options.txt.tmcl-bak-<unix 毫秒>`，`backup` 为其路径（无修改或原文件不存在时为 `null`）。先确认全部实例存在，任一不存在返回 `INSTANCE_NOT_FOUND` 且不写任何文件；尚未保存主配置返回 `SYNC_NO_MASTER` |
| `sync_capture` | `SyncCaptureArgs { instance_id }` | `SyncProfile` | 从实例的 `options.txt` 采集并覆盖保存主配置（规范化为 1.13 以后的格式，丢弃 `version`、`resourcePacks`、`incompatibleResourcePacks`、`lastServer`）；文件不存在返回 `SYNC_SOURCE_NO_OPTIONS`（`params.id`） |
| `sync_get_master` | 无 | `SyncMaster { profile: SyncProfile { source_instance, source_version, updated_at, options: SyncOption { key, value }[] } \| null }` | 读取主配置 `sync/master.json`；从未保存时为 `null`，文件损坏或格式版本不受支持返回 `SYNC_MASTER_INVALID` |
| `sync_preview` | `SyncTargetsArgs` | `SyncPreview { targets: SyncTargetPreview { instance_id, game_version, changes: SyncOptionChange { key, category, old, new }[], has_backup }[] }` | 同 `sync_apply` 但不写文件；`has_backup` 表示可用 `sync_restore` 撤销 |
| `sync_restore` | `SyncRestoreArgs { instance_id }` | `SyncRestoreResult { restored }` | 用最近一份备份覆盖 `options.txt` 并移除该备份（再次调用撤销更早的一次）；没有备份时 `restored` 为 `false` |
| `sync_save_master` | `SyncSaveMasterArgs { options: SyncOption[] }` | `SyncProfile` | 用手动编辑的选项整体替换主配置（`source_instance`、`source_version` 置为 `null`）；键为空或含 `:`、换行，值含换行，或超过 4096 项时返回 `SYNC_MASTER_INVALID` |
| `task_cancel` | `TaskCancelArgs { task_id }` | `TaskCancelResult { cancelled }` | 请求取消运行中的任务，任务随后以 `task:finished`（`cancelled`）结束；任务不存在或已结束时 `cancelled` 为 `false`，不报错 |
| `task_list` | 无 | `TaskList { tasks: TaskInfo[] }` | 全局任务列表：运行中的按开始时间先后在前，随后是最近结束的至多 50 条。`TaskInfo { task_id, kind, status, started_at, finished_at, progress, error }`，`status` 为 `running` \| `succeeded` \| `failed` \| `cancelled`，时间为 unix 毫秒，`progress` 为最近一次推送的 `TaskProgress` |

## 3. 事件

| 事件名 | 负载 | 说明 |
| --- | --- | --- |
| `task:progress` | `TaskProgress { task_id, kind, stage, done, total, bytes_done, bytes_total, speed_bps }` | 经 `ThrottledSink` 限流，同一阶段最多每 200 ms 一次；阶段切换、首条和完成总会推送 |
| `task:finished` | `TaskFinished { task_id, state, error? }` | 每个任务恰好一次；`state` 为 `succeeded` \| `failed` \| `cancelled`，仅 `failed` 带 `error` |
| `game:log` | `GameLog { instance_id, pid, stream, lines }` | `stream` 为 `stdout` \| `stderr`；同一来源的连续行最多每 100 ms 批量推送一次，访问令牌已替换为 `***` |
| `game:exited` | `GameExited { instance_id, pid, exit_code, crashed }` | 每次成功启动恰好一次；`exit_code` 可能为 `null`；`crashed` 为未被强制结束且（退出码非 0 或日志出现崩溃特征） |
| `auth:device-code` | `DeviceCode { task_id, user_code, verification_uri, expires_in }` | 每次 `auth_ms_begin` 一次；`task_id` 为该登录任务，`expires_in` 单位秒 |

- 事件向所有窗口广播；发送失败只记日志。
- 首条进度可能早于命令返回，前端须**先订阅再调用命令**，并按 `task_id`（或 `kind`）过滤。
- 任务取消：任务函数返回 `AppError::cancelled()` 时结束状态为 `cancelled`。所有经 `spawn_task` 启动的任务都登记在全局 `TaskManager` 中，见 `task_list`、`task_cancel`。

## 4. 类型生成

1. 新增的命令参数、返回值或事件负载类型在 `src-tauri/src/bindings.rs` 的 `export_all` 中登记根类型（依赖类型自动导出）。
2. 运行 `pnpm gen:bindings`，提交 `ui/src/bindings/` 的变化。
3. `cargo test`（因而 `pnpm check` 与 CI）会比对生成结果与仓库文件，不一致即失败，提示运行 `pnpm gen:bindings`。比对忽略换行符差异。
