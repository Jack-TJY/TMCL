# 错误码

> 契约文档（受保护）。规则见 [PLAN.md §3.1](../PLAN.md) 与 [AGENTS.md §6](../../AGENTS.md)。

- 错误以 `AppError { code, params, detail }` 返回；后端不返回面向用户的文字。
- 前端用 `errors.<CODE>` 作为 i18n key，`params` 作为插值参数；`detail` 只用于日志与“复制详情”，已脱敏。
- 格式 `<模块>_<原因>`，全大写。本表**只追加行，按错误码字母排序**；删除、改名、改含义属于契约变更。
- 新增错误码必须同时在 `ui/src/locales/zh-CN/errors.json` 与 `en-US/errors.json` 加文案（i18n 框架由 M0-07 建立，在此之前以本表的文案列为准）。

| 错误码 | 模块 | 含义 | params | zh-CN | en-US |
| --- | --- | --- | --- | --- | --- |
| `APPEARANCE_IMAGE_MISSING` | app | 背景图片已被删除或无法读取 | `image` | 背景图片 {image} 不存在，已改用默认背景 | Background image {image} not found, using the default background |
| `APPEARANCE_IMAGE_UNSUPPORTED` | app | 背景图片不是 png/jpg/webp，或超过大小上限 | `max_mb` | 只支持不超过 {max_mb} MB 的 PNG、JPG、WebP 图片 | Only PNG, JPG and WebP images up to {max_mb} MB are supported |
| `AUTH_ACCOUNT_NOT_FOUND` | auth | 找不到该账号（已被删除或 ID 无效） | `id` | 找不到该账号，请刷新账号列表 | Account not found, please refresh the account list |
| `AUTH_AUTHLIB_SERVER_INVALID` | auth | 地址不是有效的 Yggdrasil 皮肤站（无法解析或元数据缺少 `meta`、`skinDomains`） | `url` | {url} 不是支持外置登录的皮肤站 | {url} is not a skin site that supports external sign-in |
| `AUTH_MC_APP_NOT_APPROVED` | auth | Minecraft 服务拒绝本启动器的 Azure 应用（`login_with_xbox` 返回 403，应用未获 Mojang 批准或批准被撤销） | — | Minecraft 服务暂未允许本启动器登录，请先使用离线账号或稍后再试 | Minecraft services have not approved this launcher yet. Use an offline account or try again later |
| `AUTH_MC_NOT_OWNED` | auth | 账号没有购买 Minecraft: Java Edition（权益列表为空且没有玩家档案） | — | 这个微软账号没有购买 Minecraft: Java 版 | This Microsoft account does not own Minecraft: Java Edition |
| `AUTH_MC_NO_PROFILE` | auth | 账号拥有 Minecraft 但还没有创建玩家档案（`/minecraft/profile` 返回 404） | — | 这个账号还没有设置玩家名，请先在 minecraft.net 创建档案 | This account has no player name yet. Create a profile on minecraft.net first |
| `AUTH_MS_CODE_EXPIRED` | auth | 设备码在用户完成登录前过期（`expired_token`） | — | 登录代码已过期，请重新开始登录 | The sign-in code has expired, please start again |
| `AUTH_MS_DECLINED` | auth | 用户在微软登录页拒绝了授权（`authorization_declined`） | — | 你拒绝了授权，登录已取消 | Authorization was declined, sign-in cancelled |
| `AUTH_MS_DISABLED` | auth | 构建时没有注入 `TMCL_MS_CLIENT_ID`，微软登录不可用 | — | 此版本未启用微软登录 | Microsoft sign-in is not available in this build |
| `AUTH_MS_SESSION_EXPIRED` | auth | 微软账号的刷新令牌已失效（`invalid_grant`）或密钥库中没有令牌，需要重新登录 | `name` | {name} 的登录已过期，请在“账号”页重新登录微软账号 | Sign-in for {name} has expired, please sign in with Microsoft again on the Accounts page |
| `AUTH_NO_ACCOUNT` | auth | 没有可用于启动的账号（账号列表为空或未选默认账号） | — | 还没有可用的账号，请先在“账号”页添加 | No account available, please add one on the Accounts page |
| `AUTH_OFFLINE_INVALID_NAME` | auth | 离线账号用户名不符合 `^[A-Za-z0-9_]{3,16}$` | `name` | 用户名只能包含 3–16 个字母、数字或下划线 | Username must be 3–16 letters, digits or underscores |
| `AUTH_SECRET_STORE` | auth | 无法读写系统密钥库（Windows 凭据管理器、macOS 钥匙串、Linux Secret Service） | — | 无法访问系统密钥库，登录信息没有保存 | Cannot access the system keychain, sign-in data was not saved |
| `AUTH_XSTS_CHILD_ACCOUNT` | auth | XSTS 返回 `XErr` 2148916238：未成年账号需要加入家庭组 | — | 这是未成年人账号，需要由家长将其加入 Microsoft 家庭组后才能登录 | This is a child account. An adult must add it to a Microsoft family group before it can sign in |
| `AUTH_XSTS_DENIED` | auth | XSTS 以其他 `XErr` 拒绝授权（如地区不可用、需要成人验证） | `xerr` | Xbox Live 拒绝了登录（错误 {xerr}），请在 xbox.com 登录一次检查账号状态 | Xbox Live refused the sign-in (error {xerr}). Sign in once at xbox.com to check your account |
| `AUTH_XSTS_NO_XBOX_ACCOUNT` | auth | XSTS 返回 `XErr` 2148916233：微软账号还没有 Xbox 档案 | — | 这个微软账号还没有 Xbox 档案，请先在 xbox.com 登录一次创建 | This Microsoft account has no Xbox profile yet. Sign in once at xbox.com to create one |
| `AUTH_YGGDRASIL_INVALID_CREDENTIALS` | auth | 皮肤站拒绝登录（账号或密码错误，或登录过于频繁） | — | 账号或密码错误，或登录太频繁，请稍后再试 | Wrong email or password, or too many attempts. Please try again later |
| `AUTH_YGGDRASIL_NO_PROFILE` | auth | 皮肤站账号下没有角色 | — | 这个账号还没有角色，请先在皮肤站创建角色 | This account has no player profile yet. Create one on the skin site first |
| `AUTH_YGGDRASIL_SESSION_EXPIRED` | auth | 外置登录的令牌已失效且无法刷新，需要重新输入密码 | `name` | {name} 的登录已过期，请在“账号”页重新登录 | Sign-in for {name} has expired, please sign in again on the Accounts page |
| `COMMON_CANCELLED` | common | 任务被用户或上层取消 | — | 操作已取消 | The operation was cancelled |
| `COMMON_CONFIG_SCHEMA_UNSUPPORTED` | common | `config.json` 的 `schema` 为 0 或高于当前程序支持的版本 | `schema`、`supported` | 配置文件版本 {schema} 不受支持（当前支持 {supported}） | Config file version {schema} is not supported (supported: {supported}) |
| `COMMON_DATA_DIR_UNAVAILABLE` | common | 无法确定或创建数据目录 | `path`（可选） | 无法访问数据目录 {path} | Cannot access data directory {path} |
| `COMMON_DB` | common | SQLite 打开、迁移或查询失败 | `path`、`migration`（均可选） | 数据库出错 | Database error |
| `COMMON_INTERNAL` | common | 未归类的内部错误（程序缺陷） | — | 发生内部错误，请反馈给开发者 | An internal error occurred, please report it |
| `COMMON_IO` | common | 文件系统读写失败 | `path`（可选） | 读写文件失败 {path} | Failed to read or write file {path} |
| `COMMON_JSON` | common | JSON 序列化或反序列化失败 | — | 数据格式错误 | Invalid data format |
| `COMMON_LOG_INIT` | common | 日志系统初始化失败 | — | 日志系统初始化失败 | Failed to initialize logging |
| `GAME_FILE_MISSING` | game | 运行该版本需要的文件缺失，且没有下载地址（由加载器安装器生成，需重新安装加载器） | `version`、`path` | 版本 {version} 缺少文件 {path}，请重新安装模组加载器 | Version {version} is missing {path}, please reinstall the mod loader |
| `GAME_VERSION_JSON_INVALID` | game | 版本 JSON 格式错误或缺少必需字段 | `version` | 版本 {version} 的描述文件已损坏 | The metadata of version {version} is invalid |
| `GAME_VERSION_NOT_FOUND` | game | 版本清单中没有该版本 | `version` | 找不到版本 {version} | Version {version} was not found |
| `INSTANCE_CORRUPTED` | instance | 实例的 `instance.json` 缺失或格式错误 | `id`（可选） | 实例 {id} 的配置文件已损坏 | The configuration of instance {id} is corrupted |
| `INSTANCE_INVALID_NAME` | instance | 实例名称为空、超过 64 个字符或含控制字符 | `max` | 实例名称不能为空，且不超过 {max} 个字符 | Instance name must not be empty and at most {max} characters |
| `INSTANCE_NOT_FOUND` | instance | 找不到该实例（目录不存在或 ID 非法） | `id` | 找不到实例 {id} | Instance {id} was not found |
| `INSTANCE_PACK_INVALID` | instance | `.tmclpack` 不是有效的 zip，或缺少 / 损坏 `instance.json` | — | 不是有效的 TMCL 实例包 | Not a valid TMCL instance pack |
| `INSTANCE_SCHEMA_UNSUPPORTED` | instance | `instance.json` 的 `schema` 缺失、为 0 或高于当前程序支持的版本 | `schema`、`supported` | 实例配置版本 {schema} 不受支持（当前支持 {supported}），请更新启动器 | Instance config version {schema} is not supported (supported: {supported}); please update the launcher |
| `JAVA_ARCH_MISMATCH` | java | Java 的架构与启动器不一致（如 64 位系统上的 32 位 Java）；自动选择时为错误，手动指定时为警告 | `java`、`expected`、`actual` | Java 架构不匹配：需要 {expected}，{java} 为 {actual} | Java architecture mismatch: {expected} required, but {java} is {actual} |
| `JAVA_NOT_FOUND` | java | 找不到满足主版本要求的 Java | `major` | 找不到 Java {major} | Java {major} was not found |
| `JAVA_PROBE_FAILED` | java | 无法识别 Java 的版本或架构（文件不存在、不是 Java 或运行超时） | `java` | 无法识别 Java：{java} | Cannot identify Java: {java} |
| `JAVA_RUNTIME_UNAVAILABLE` | java | 官方源与 Adoptium 都没有该平台、该主版本的运行时 | `major`、`platform` | 没有适用于 {platform} 的 Java {major} 可供下载，请手动安装 | No downloadable Java {major} is available for {platform}; please install it manually |
| `JAVA_VERSION_MISMATCH` | java | 手动指定的 Java 主版本与游戏要求不一致，仅作警告 | `java`、`expected`、`actual` | 所选 Java 为 {actual}，该版本游戏需要 Java {expected}，可能无法启动 | The selected Java is {actual}, but this game version needs Java {expected} and may not start |
| `LAUNCH_ALREADY_RUNNING` | launch | 该实例已在启动或运行中，同一实例同时只能运行一个游戏 | `id` | 该实例已在运行 | This instance is already running |
| `LAUNCH_SPAWN_FAILED` | launch | 无法启动游戏进程（Java 路径无效或无权限） | `java` | 无法启动 Java：{java} | Failed to start Java: {java} |
| `LAUNCH_VERSION_NOT_INSTALLED` | launch | 实例需要的游戏版本尚未安装；目前用于带模组加载器的实例（加载器安装由 M1-09 提供） | `version`、`loader`（可选） | 实例需要的版本 {version} {loader} 尚未安装 | The version {version} {loader} required by this instance is not installed |
| `LOADER_INSTALLER_INVALID` | loader | 加载器安装器或安装描述（install_profile.json、profile JSON）损坏或格式不受支持 | `kind`、`version` | 安装器损坏或格式不受支持（{kind} {version}） | The installer of {kind} {version} is corrupted or unsupported |
| `LOADER_PROCESSOR_FAILED` | loader | Forge/NeoForge 安装器的处理步骤（processor）退出码非 0、输出缺失或 SHA1 不符 | `kind`、`version`、`processor` | {kind} {version} 安装失败：处理步骤 {processor} 出错 | Failed to install {kind} {version}: processor {processor} failed |
| `LOADER_UNSUPPORTED_VERSION` | loader | 该加载器不支持这个游戏版本（如 Forge 1.7.10 以前的版本） | `kind`、`game` | {kind} 不支持 Minecraft {game} | {kind} does not support Minecraft {game} |
| `LOADER_VERSION_NOT_FOUND` | loader | 加载器元数据中没有该游戏版本对应的加载器版本 | `kind`、`game`、`version` | 找不到适用于 Minecraft {game} 的 {kind} {version} | {kind} {version} for Minecraft {game} was not found |
| `NET_CONNECT` | net | 无法连接服务器（DNS、TCP 或 TLS 失败） | `url` | 无法连接到服务器 {url} | Cannot connect to {url} |
| `NET_HASH_MISMATCH` | net | 下载文件的 SHA1 与期望值不一致 | `url`、`expected`、`actual` | 文件校验失败 {url} | File verification failed for {url} |
| `NET_HTTP_STATUS` | net | 服务器返回非 2xx 状态码 | `url`、`status` | 服务器返回错误 {status}：{url} | Server returned {status} for {url} |
| `NET_REQUEST` | net | 其他 HTTP 请求错误（构造请求、读取响应、解码失败等） | `url`（可选） | 网络请求失败 {url} | Network request failed for {url} |
| `NET_SIZE_MISMATCH` | net | 下载文件的大小与期望值不一致 | `url`、`expected`、`actual` | 文件大小不符 {url} | File size mismatch for {url} |
| `NET_TIMEOUT` | net | 连接或读取超时 | `url` | 连接超时 {url} | Connection timed out: {url} |
| `RESOURCES_CURSEFORGE_DISABLED` | resources | 本次构建没有 CurseForge API Key，CurseForge 来源不可用 | — | 此版本未启用 CurseForge | CurseForge is not enabled in this build |
| `RESOURCES_DEPENDENCY_UNAVAILABLE` | resources | 必需前置在 Modrinth 上没有适用于该实例游戏版本与加载器的版本，或前置项目已不存在 | `project` | 找不到适用于此实例的前置 {project} | No version of the required dependency {project} fits this instance |
| `RESOURCES_FILE_NOT_FOUND` | resources | 实例中找不到要启用、禁用或删除的资源文件（已被移走或文件名非法） | `file` | 找不到资源文件 {file}，请刷新列表 | Resource file {file} was not found, please refresh the list |
| `RESOURCES_INCOMPATIBLE` | resources | 要安装的资源与已安装或同批安装的资源不兼容（Modrinth `incompatible` 依赖） | `project`、`with` | {project} 与 {with} 不兼容，已取消安装 | {project} is incompatible with {with}, installation cancelled |
| `RESOURCES_MANUAL_DOWNLOAD` | resources | CurseForge 上的作者禁止第三方下载该文件（`downloadUrl` 为空），需要在网页上手动下载 | `name`、`url` | {name} 的作者不允许第三方下载，请在 CurseForge 网页上手动下载 | The author of {name} does not allow third-party downloads, please download it from the CurseForge page |
| `RESOURCES_MODPACK_INVALID` | resources | 整合包文件无效：不是 zip、缺少或损坏 `modrinth.index.json`，或其中的文件路径、哈希、下载地址非法 | — | 整合包文件已损坏或格式不正确 | The modpack file is damaged or not a valid modpack |
| `RESOURCES_MODPACK_UNSUPPORTED` | resources | 整合包的格式版本、游戏或依赖（加载器）不受支持 | `name` | 不支持此整合包：{name} | This modpack is not supported: {name} |
| `RESOURCES_VERSION_NOT_FOUND` | resources | Modrinth 上找不到要安装的版本，或该版本没有文件 | `version` | 找不到资源版本 {version} | Resource version {version} was not found |
| `RESOURCES_WORLD_NOT_FOUND` | resources | 数据包要安装到的存档不存在或名称非法 | `world` | 找不到存档 {world} | World {world} was not found |
| `SAKURA_ACCOUNT_BANNED` | sakura | SakuraFrp 账号已被冻结（`/user/info` 返回 `ban`） | — | 你的樱花穿透账号已被冻结，请到 SakuraFrp 官网查看原因 | Your SakuraFrp account is suspended. Check the reason on the SakuraFrp website |
| `SAKURA_ALREADY_RUNNING` | sakura | 已有一个樱花穿透房间在运行，同一时间只能有一个 | — | 已经在联机中，请先停止当前房间 | Already hosting a room. Stop it first |
| `SAKURA_API_ERROR` | sakura | SakuraFrp 接口拒绝请求（4xx，如隧道数已满、节点权限不足），`msg` 为服务端返回的说明，原样显示 | `msg`、`status` | 樱花穿透返回错误：{msg} | SakuraFrp returned an error: {msg} |
| `SAKURA_FRPC_FAILED` | sakura | frpc 无法启动、提前退出，或 60 秒内没有给出联机地址；详情为脱敏后的最后几行日志 | — | 穿透客户端启动失败，请稍后重试或换一个节点 | The tunnel client failed to start. Try again later or pick another node |
| `SAKURA_FRPC_UNAVAILABLE` | sakura | 官方下载信息中没有适用于当前平台的 frpc | `platform` | 樱花穿透暂不支持 {platform} | SakuraFrp does not support {platform} yet |
| `SAKURA_KEY_INVALID` | sakura | SakuraFrp 访问密钥格式不对、无效或已被重置（接口返回 401） | — | 访问密钥无效，请到 SakuraFrp 官网的用户信息页复制最新的访问密钥 | The access key is invalid. Copy the current access key from your SakuraFrp user page |
| `SAKURA_KEY_MISSING` | sakura | 还没有保存 SakuraFrp 访问密钥 | — | 请先填写樱花穿透的访问密钥 | Please enter your SakuraFrp access key first |
| `SAKURA_NODE_NOT_PUBLIC` | sakura | 公开房间选了不允许穿透公开游戏服务器的节点（`flag & 2048` 为 0 且名称不含“多线”） | `node` | 公开的服务器只能使用允许公开游戏的节点，{node} 不行，请换一个 | Public servers may only use nodes that allow public games, and {node} does not. Pick another node |
| `SAKURA_NODE_UNAVAILABLE` | sakura | 所选节点不存在、离线或已满载 | `node` | 节点 {node} 暂时不可用，请换一个 | Node {node} is not available right now. Pick another node |
| `SAKURA_REALNAME_REQUIRED` | sakura | SakuraFrp 账号未完成实名认证（`/user/info` 的 `realname` 不大于 0）；TMCL 不收集身份信息 | — | 樱花穿透账号还没有实名认证，请先到 SakuraFrp 官网完成实名 | Your SakuraFrp account is not verified yet. Complete identity verification on the SakuraFrp website first |
| `SKIN_INVALID_IMAGE` | skin | 皮肤文件不是 PNG，或尺寸不是 64×64 / 64×32；正版上传时服务器拒收（400）也用此码 | `width`、`height`（可识别时） | 皮肤必须是 64×64 或 64×32 像素的 PNG 图片 | A skin must be a 64×64 or 64×32 PNG image |
| `SKIN_NOT_FOUND` | skin | 本地皮肤库中没有该皮肤（已被删除或 ID 无效） | `id` | 皮肤库中没有这个皮肤，请刷新列表 | This skin is not in the library, please refresh the list |
| `SYNC_MASTER_INVALID` | sync | 设置同步主配置无效：`master.json` 损坏或格式版本不受支持，或要保存的选项键/值非法 | — | 设置同步的主配置无效，请重新采集或编辑 | The settings sync master profile is invalid. Capture or edit it again |
| `SYNC_NO_MASTER` | sync | 预览或应用前还没有保存主配置 | — | 还没有设置同步的主配置，请先从实例采集或手动编辑 | No settings sync master profile yet. Capture one from an instance or edit it first |
| `SYNC_SOURCE_NO_OPTIONS` | sync | 作为来源的实例还没有 `options.txt`（从未启动过游戏） | `id` | 实例 {id} 还没有游戏设置，请先启动一次游戏 | Instance {id} has no game settings yet. Launch the game once first |
