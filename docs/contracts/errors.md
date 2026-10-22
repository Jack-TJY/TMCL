# 错误码

> 契约文档（受保护）。规则见 [PLAN.md §3.1](../PLAN.md) 与 [AGENTS.md §6](../../AGENTS.md)。

- 错误以 `AppError { code, params, detail }` 返回；后端不返回面向用户的文字。
- 前端用 `errors.<CODE>` 作为 i18n key，`params` 作为插值参数；`detail` 只用于日志与“复制详情”，已脱敏。
- 格式 `<模块>_<原因>`，全大写。本表**只追加行，按错误码字母排序**；删除、改名、改含义属于契约变更。
- 新增错误码必须同时在 `ui/src/locales/zh-CN/errors.json` 与 `en-US/errors.json` 加文案（i18n 框架由 M0-07 建立，在此之前以本表的文案列为准）。

| 错误码 | 模块 | 含义 | params | zh-CN | en-US |
| --- | --- | --- | --- | --- | --- |
| `COMMON_CANCELLED` | common | 任务被用户或上层取消 | — | 操作已取消 | The operation was cancelled |
| `COMMON_CONFIG_SCHEMA_UNSUPPORTED` | common | `config.json` 的 `schema` 为 0 或高于当前程序支持的版本 | `schema`、`supported` | 配置文件版本 {schema} 不受支持（当前支持 {supported}） | Config file version {schema} is not supported (supported: {supported}) |
| `COMMON_DATA_DIR_UNAVAILABLE` | common | 无法确定或创建数据目录 | `path`（可选） | 无法访问数据目录 {path} | Cannot access data directory {path} |
| `COMMON_DB` | common | SQLite 打开、迁移或查询失败 | `path`、`migration`（均可选） | 数据库出错 | Database error |
| `COMMON_INTERNAL` | common | 未归类的内部错误（程序缺陷） | — | 发生内部错误，请反馈给开发者 | An internal error occurred, please report it |
| `COMMON_IO` | common | 文件系统读写失败 | `path`（可选） | 读写文件失败 {path} | Failed to read or write file {path} |
| `COMMON_JSON` | common | JSON 序列化或反序列化失败 | — | 数据格式错误 | Invalid data format |
| `COMMON_LOG_INIT` | common | 日志系统初始化失败 | — | 日志系统初始化失败 | Failed to initialize logging |
| `GAME_VERSION_JSON_INVALID` | game | 版本 JSON 格式错误或缺少必需字段 | `version` | 版本 {version} 的描述文件已损坏 | The metadata of version {version} is invalid |
| `GAME_VERSION_NOT_FOUND` | game | 版本清单中没有该版本 | `version` | 找不到版本 {version} | Version {version} was not found |
| `NET_CONNECT` | net | 无法连接服务器（DNS、TCP 或 TLS 失败） | `url` | 无法连接到服务器 {url} | Cannot connect to {url} |
| `NET_HASH_MISMATCH` | net | 下载文件的 SHA1 与期望值不一致 | `url`、`expected`、`actual` | 文件校验失败 {url} | File verification failed for {url} |
| `NET_HTTP_STATUS` | net | 服务器返回非 2xx 状态码 | `url`、`status` | 服务器返回错误 {status}：{url} | Server returned {status} for {url} |
| `NET_REQUEST` | net | 其他 HTTP 请求错误（构造请求、读取响应、解码失败等） | `url`（可选） | 网络请求失败 {url} | Network request failed for {url} |
| `NET_SIZE_MISMATCH` | net | 下载文件的大小与期望值不一致 | `url`、`expected`、`actual` | 文件大小不符 {url} | File size mismatch for {url} |
| `NET_TIMEOUT` | net | 连接或读取超时 | `url` | 连接超时 {url} | Connection timed out: {url} |
