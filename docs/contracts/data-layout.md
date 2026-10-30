# 数据目录与 config.json

> 契约文档（受保护）。依据 [PLAN.md §3.5、§3.6](../PLAN.md)，实现于 `core/common/src/{paths.rs,config.rs,db.rs}`。

## 1. 数据根目录 `<data_root>`

由 `DataPaths::resolve()` 决定，固定为**启动器安装目录下的 `tmcl-data/`**（维护者 2026-10-22 决定，不使用系统应用数据目录，也不设便携模式）：

| 平台 | 安装目录 | `<data_root>` |
| --- | --- | --- |
| Windows / Linux | exe 所在目录 | `<exe 所在目录>/tmcl-data/` |
| macOS | `TMCL.app` 所在目录（不写进应用包，避免破坏签名） | `<.app 所在目录>/tmcl-data/` |
| Linux AppImage | `.AppImage` 文件所在目录（运行时挂载目录只读） | `<$APPIMAGE 所在目录>/tmcl-data/` |
| Linux 系统包（deb 等，exe 位于 `/usr/` 下） | `/usr/bin`，普通用户不可写 | `$XDG_DATA_HOME/tmcl-data/`，未设置时为 `~/.local/share/tmcl-data/` |

- Linux 上由 `DataPaths::resolve_linux()` 按上表判断：`$APPIMAGE` 为绝对路径时视为 AppImage；否则 exe 位于 `/usr/` 下视为系统包；其余按 exe 所在目录。不是绝对路径的环境变量按未设置处理（维护者 2026-10-24 确认保留 deb 并使用该位置）。
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
├─ sync/master.json          # 设置同步的主配置，见第 6 节
├─ ui/
│  ├─ layout.json            # 主页组件布局
│  └─ backgrounds/           # 用户背景图
├─ skins/                    # 本地皮肤库：<sha256>.png 与 index.json（M2-07）
├─ p2p/easytier/<版本>/      # 运行时下载的 EasyTier 程序（M2-12）
├─ cache/                    # 可随时清空
└─ logs/                     # 启动器日志，已脱敏
```

实例 `game/` 下由资源安装（M2-03）写入的目录沿用游戏自身的约定：`mods/`、`resourcepacks/`、`shaderpacks/`、`saves/<存档>/datapacks/`；被禁用的资源文件名追加 `.disabled` 后缀。

代码中一律通过 `DataPaths` 的方法取路径（`config_file()`、`instance_game_dir(id)` 等），不手写拼接。`DataPaths::ensure_dirs()` 创建上述固定目录，可重复调用。

`skins/`（`DataPaths::skins_dir()`，首次导入皮肤时创建）：每张皮肤存为 `<sha256>.png`（文件内容的 SHA-256，小写十六进制，同一图片只存一份），`index.json` 为 `{ "schema": 1, "skins": [{ "id", "name", "variant": "classic"|"slim", "height": 64|32, "added_at": <unix 毫秒> }] }`；图片已丢失的条目读取时忽略。

`p2p/`（`DataPaths::p2p_dir()`，首次联机时创建）：`easytier/<版本>/` 下是从 EasyTier 官方 GitHub Releases 下载、按内置 SHA-256 校验后解压的 `easytier-core`、`easytier-cli`（Windows 另有 `Packet.dll`），解压完成后写入 `.complete`；没有 `.complete` 的目录视为未安装，下次重新下载。

## 3. config.json

当前 `schema` 为 **2**。所有字段都有默认值；缺失字段取默认值，未知字段被忽略。

```json
{
  "schema": 2,
  "language": "zh-CN",
  "theme": "system",
  "appearance": {
    "accent": null,
    "window_material": true,
    "material": "acrylic",
    "material_opacity": 0,
    "background": {
      "kind": "default",
      "color": "#1F4E79",
      "image": null,
      "fit": "fill",
      "opacity": 100,
      "blur": 0,
      "mask": 0,
      "card_opacity": 85
    }
  },
  "memory": {
    "min_mb": 1024,
    "max_mb": 4096
  },
  "download": {
    "max_concurrency": 16,
    "source": "auto"
  }
}
```

| 字段 | 类型 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `schema` | 整数 | `2` | 结构版本，写入时总是当前版本 |
| `language` | 字符串 | `"zh-CN"` | 界面语言，BCP 47 标签 |
| `theme` | `"system"` \| `"light"` \| `"dark"` | `"system"` | 界面主题 |
| `appearance.accent` | 字符串 \| `null` | `null` | 强调色 `#RRGGBB`；`null` 跟随系统强调色（取不到时用 `#0067C0`） |
| `appearance.window_material` | 布尔 | `true` | 平台支持时使用窗口材质（Win11 磨砂玻璃或 Mica、macOS vibrancy），否则使用 `background` |
| `appearance.material` | `"acrylic"` \| `"mica"` | `"acrylic"` | Windows 11 上的材质：磨砂玻璃（模糊窗口后方内容，需 22H2 及以上，更早的系统回退 Mica）或 Mica；macOS 始终用 vibrancy |
| `appearance.material_opacity` | 整数 0–100 | `0` | 材质生效时叠在材质上的主题底色不透明度（%），0 为纯材质，100 为纯色 |
| `appearance.background.kind` | `"default"` \| `"color"` \| `"image"` | `"default"` | 窗口材质未生效时的背景：内置亮/暗背景（跟随主题）、纯色、自定义图片；图片缺失时回退 `default` |
| `appearance.background.color` | 字符串 | `"#1F4E79"` | 纯色背景 `#RRGGBB` |
| `appearance.background.image` | 字符串 \| `null` | `null` | `ui/backgrounds/` 中的文件名，由 `appearance_import_background` 生成 |
| `appearance.background.fit` | `"fill"` \| `"fit"` \| `"center"` \| `"tile"` | `"fill"` | 图片填充方式：填充、适应、居中、平铺 |
| `appearance.background.opacity` | 整数 0–100 | `100` | 背景不透明度（%），其余透出主题底色 |
| `appearance.background.blur` | 整数 0–40 | `0` | 背景模糊半径（px） |
| `appearance.background.mask` | 整数 -100–100 | `0` | 遮罩亮度：负数变暗、正数变亮 |
| `appearance.background.card_opacity` | 整数 0–100 | `85` | 卡片与内容区底色不透明度（%），低于 100 时显示毛玻璃 |
| `memory.min_mb` | 正整数 | `1024` | 全局最小内存（MiB，`-Xms`），512–32768 且不大于 `memory.max_mb` |
| `memory.max_mb` | 正整数 | `4096` | 全局最大内存（MiB，`-Xmx`），512–32768；`instance.json` 中 `memory.follow_global` 为 `true` 的实例启动时使用这两项 |
| `download.max_concurrency` | 正整数 | `16` | 同时下载数 |
| `download.source` | `"auto"` \| `"official"` \| 镜像名（如 `"bmclapi"`） | `"auto"` | 下载源。`auto`：Forge、NeoForge、Fabric 先走 BMCLAPI（`bmclapi-loaders`），其余文件走官方源，失败都回退官方源。`bmclapi`：所有文件先走 BMCLAPI 再回退官方源。`mcim`：Modrinth 与 CurseForge 的文件（`cdn.modrinth.com`、`edge.forgecdn.net`）先走 MCIM 再回退官方源（MCIM 暂不提供文件，设置界面不提供此项）。未知名称按 `auto` 处理 |

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

## 6. 设置同步

- `sync/master.json`：`{ "schema": 1, "source_instance", "source_version", "updated_at", "options": [{ "key", "value" }] }`，原子写入。选项按 1.13 以后的格式存储（按键为 `key.keyboard.*` / `key.mouse.*`，语言代码小写），应用时按目标版本转换。实现于 `modules/sync`。
- `instances/<id>/game/options.txt.tmcl-bak-<unix 毫秒>`：应用主配置前对原 `options.txt` 的备份，每个实例保留最近 10 份；撤销时移回 `options.txt`。
