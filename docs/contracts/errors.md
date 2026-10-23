# 错误码

> 契约文档（受保护）。规则见 [PLAN.md §3.1](../PLAN.md) 与 [AGENTS.md §6](../../AGENTS.md)。

- 错误以 `AppError { code, params, detail }` 返回；后端不返回面向用户的文字。
- 前端用 `errors.<CODE>` 作为 i18n key，`params` 作为插值参数；`detail` 只用于日志与“复制详情”，已脱敏。
- 格式 `<模块>_<原因>`，全大写。本表**只追加行，按错误码字母排序**；删除、改名、改含义属于契约变更。
- 新增错误码必须同时在 `ui/src/locales/zh-CN/errors.json` 与 `en-US/errors.json` 加文案（i18n 框架由 M0-07 建立，在此之前以本表的文案列为准）。

| 错误码 | 模块 | 含义 | params | zh-CN | en-US |
| --- | --- | --- | --- | --- | --- |
| `AUTH_OFFLINE_INVALID_NAME` | auth | 离线账号用户名不符合 `^[A-Za-z0-9_]{3,16}$` | `name` | 用户名只能包含 3–16 个字母、数字或下划线 | Username must be 3–16 letters, digits or underscores |
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
| `LAUNCH_SPAWN_FAILED` | launch | 无法启动游戏进程（Java 路径无效或无权限） | `java` | 无法启动 Java：{java} | Failed to start Java: {java} |
| `NET_CONNECT` | net | 无法连接服务器（DNS、TCP 或 TLS 失败） | `url` | 无法连接到服务器 {url} | Cannot connect to {url} |
| `NET_HASH_MISMATCH` | net | 下载文件的 SHA1 与期望值不一致 | `url`、`expected`、`actual` | 文件校验失败 {url} | File verification failed for {url} |
| `NET_HTTP_STATUS` | net | 服务器返回非 2xx 状态码 | `url`、`status` | 服务器返回错误 {status}：{url} | Server returned {status} for {url} |
| `NET_REQUEST` | net | 其他 HTTP 请求错误（构造请求、读取响应、解码失败等） | `url`（可选） | 网络请求失败 {url} | Network request failed for {url} |
| `NET_SIZE_MISMATCH` | net | 下载文件的大小与期望值不一致 | `url`、`expected`、`actual` | 文件大小不符 {url} | File size mismatch for {url} |
| `NET_TIMEOUT` | net | 连接或读取超时 | `url` | 连接超时 {url} | Connection timed out: {url} |
