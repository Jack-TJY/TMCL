# 数据目录与 config.json

> 契约文档（受保护）。依据 [PLAN.md §3.5、§3.6](../PLAN.md)，实现于 `core/common/src/{paths.rs,config.rs,db.rs}`。

## 1. 数据根目录 `<data_root>`

由 `DataPaths::resolve()` 决定，固定为**启动器安装目录下的 `tmcl-data/`**（维护者 2026-10-22 决定，不使用系统应用数据目录，也不设便携模式）：

| 平台 | 安装目录 | `<data_root>` |
| --- | --- | --- |
| Windows / Linux | exe 所在目录 | `<exe 所在目录>/tmcl-data/` |
| macOS | `TMCL.app` 所在目录（不写进应用包，避免破坏签名） | `<.app 所在目录>/tmcl-data/` |

- 安装目录不可写时，`DataPaths::ensure_dirs()` 返回 `COMMON_DATA_DIR_UNAVAILABLE`。打包配置需保证默认安装到用户可写的位置（例如 NSIS 的 `currentUser` 模式）。

## 2. 目录结构

```
<data_root>/
├─ config.json               # 启动器设置，见第 3 节
├─ tmcl.db                   # SQLite，见第 5 节
├─ shared/
│  ├─ versions/              # 多实例共享的版本文件
│  ├─ libraries/
│  └─ assets/
├─ java/<runtime-name>/      # 启动器下载的 Java
├─ instances/<instance-id>/
│  ├─ instance.json          # 结构见 PLAN.md §3.5
│  └─ game/                  # --gameDir
├─ trash/<unix 秒>-<instance-id>/  # 已删除的实例，7 天后清理
├─ ui/
│  ├─ layout.json            # 主页组件布局
│  └─ backgrounds/           # 用户背景图
├─ cache/                    # 可随时清空
└─ logs/                     # 启动器日志，已脱敏
```

代码中一律通过 `DataPaths` 的方法取路径（`config_file()`、`instance_game_dir(id)` 等），不手写拼接。`DataPaths::ensure_dirs()` 创建上述固定目录，可重复调用。

## 3. config.json

当前 `schema` 为 **2**。所有字段都有默认值；缺失字段取默认值，未知字段被忽略。

```json
{
  "schema": 2,
  "language": "zh-CN",
  "theme": "system",
  "appearance": {
    "accent": null,
    "window_material": true
  },
  "memory": {
    "max_mb": 4096
  },
  "download": {
    "max_concurrency": 16
  }
}
```

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `schema` | 整数 | `2` | 结构版本，写入时总是当前版本 |
| `language` | 字符串 | `"zh-CN"` | 界面语言，BCP 47 标签 |
| `theme` | `"system"` \| `"light"` \| `"dark"` | `"system"` | 界面主题 |
| `appearance.accent` | 字符串 \| `null` | `null` | 强调色 `#RRGGBB`；`null` 跟随系统强调色（取不到时用 `#0067C0`） |
| `appearance.window_material` | 布尔 | `true` | 平台支持时使用窗口材质（Win11 Mica、macOS vibrancy），否则纯色背景 |
| `memory.max_mb` | 正整数 | `4096` | 新建实例的默认最大内存（MiB），512–32768 |
| `download.max_concurrency` | 正整数 | `16` | 同时下载数 |

读写规则：

1. **读取**：文件不存在 → 默认值，不写盘。JSON 损坏、字段类型错误、`schema` 为 0 或高于当前版本 → 把原文件重命名为 `config.json.corrupt-<unix 毫秒>`，回退默认值，并在返回值 `LoadedConfig::backup` 中给出备份路径供界面提示。只有读取或备份本身失败时返回错误。
2. **写入**：原子写。先写同目录 `config.json.<pid>.tmp` 并 `fsync`，再 rename 覆盖。
3. **迁移**：结构变化时提升 `schema`，在 `config::migrate` 中按顺序追加 `n → n+1` 的迁移步骤并补测试（AGENTS.md §6.2 第 5 条）。新增带默认值的字段不需要提升 `schema`，但需在本表追加。

迁移历史：

| 迁移 | 内容 |
| --- | --- |
| 1 → 2 | `locale` 改名为 `language`（与 PLAN.md M0-07 一致）；已有 `language` 时丢弃 `locale` |

## 4. 日志

- `init_logging(&paths.logs_dir(), &LogOptions)` 初始化全局 `tracing`：文件名 `tmcl.<YYYY-MM-DD>.log`，按天滚动，保留 7 个文件。
- 所有输出（文件与 stderr）都经过脱敏层：`access_token`、`refresh_token`（含 camelCase）、`Authorization`、`x-api-key` 的值以及 `--accessToken` 后的参数替换为 `***`。`AppError::with_detail` 也会脱敏。

## 5. tmcl.db

- 打开时启用 `journal_mode=WAL`、`foreign_keys=ON`、`busy_timeout=5s`。
- 迁移文件放在 `core/common/migrations/NNNN_<模块>_<描述>.sql`，在 `core/common/src/db.rs` 的 `MIGRATIONS` 末尾追加一行登记；已发布的迁移不得修改。
- 迁移记录在 `schema_migrations(version, name, applied_at)`；每条迁移在独立事务中执行，失败整体回滚，重复执行安全。
- 访问令牌、刷新令牌不进数据库，只存系统密钥库（键名 `tmcl:account:<account-id>`）。

| 版本 | 文件 | 内容 |
| --- | --- | --- |
| 1 | `0001_common_init.sql` | `kv(key, value, updated_at)` 通用键值表 |
| 2 | `0002_instance_index.sql` | `instance_index`：实例列表缓存，启动时按 `instance.json` 的修改时间与大小对账 |
