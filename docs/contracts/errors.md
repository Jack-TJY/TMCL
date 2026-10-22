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
