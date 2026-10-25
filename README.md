# TMCL

**简体中文** | [English](#english)

TMCL 是一个开源、跨平台的我的世界（Java 版）启动器。下载游戏、安装模组和整合包、换皮肤、管理多版本、和朋友联机，都在一个客户端里完成。

> ⚠️ 当前为 Alpha 预览版，已能下载并启动游戏（含 Forge、NeoForge、Fabric、Quilt）和使用三种账号，其余功能仍在开发中，可能存在问题。设计细节见 [TMCL 项目大纲](TMCL%20项目大纲.md)，测试结果与已知问题见 [Alpha 测试矩阵](docs/testing/alpha-matrix.md)。
>
> TMCL 是非官方启动器，与 Mojang Studios 和 Microsoft 没有关联。

## 功能规划

| 优先级 | 模块 | 内容 |
| --- | --- | --- |
| P0 首发 | 游戏下载 | 正式版、快照、远古版一键下载，官方源与国内镜像可切换 |
| P0 首发 | Java 管理 | 按游戏版本自动匹配并下载 Java 8 / 17 / 21 |
| P0 首发 | 账号 | 微软正版登录、离线账号（仅限本地测试）、authlib-injector 外置登录 |
| P0 首发 | 多版本管理 | 实例隔离，独立的模组、存档和配置 |
| P0 首发 | 模组加载器 | Forge、NeoForge、Fabric、Quilt 一键安装 |
| P1 | 整合包与资源 | Modrinth / CurseForge 整合包安装与导出，模组、资源包、光影搜索 |
| P1 | 皮肤 | 更换皮肤与披风，3D 实时预览 |
| P1 | 设置同步 | 按键、视频、音频设置在实例间同步，兼容 1.13 前后的键位格式 |
| P1 | 联机 | P2P 直连（打洞 + 中继）、内置樱花穿透（SakuraFrp） |
| P2 | 其他 | 地图种子预览、崩溃日志分析、自动更新、主题切换 |

界面支持简体中文和英文，目标平台为 Windows、macOS 和 Linux。

## 技术栈

- 桌面框架：[Tauri 2](https://tauri.app/)
- 核心：Rust
- 前端：Vue 3 + TypeScript
- 存储：实例 JSON 配置 + SQLite

## 安装

从 [GitHub Releases](https://github.com/Jack-TJY/TMCL/releases) 下载对应平台的安装包。TMCL 把设置、实例和游戏文件都放在安装目录旁的 `tmcl-data/` 文件夹里，所以请装在自己有写权限的位置。

| 平台 | 安装包 | 数据位置 |
| --- | --- | --- |
| Windows 10 / 11 | `TMCL_<版本>_x64-setup.exe`，安装给当前用户，不需要管理员权限，默认装到 `%LOCALAPPDATA%\TMCL` | 安装目录下的 `tmcl-data\` |
| macOS 10.15+ | `TMCL_<版本>_<架构>.dmg`，把 `TMCL.app` 拖到“应用程序”或任意文件夹 | `TMCL.app` 所在文件夹下的 `tmcl-data/` |
| Linux | `TMCL_<版本>_amd64.AppImage`（推荐）或 `.deb` | AppImage：`.AppImage` 文件旁的 `tmcl-data/`；deb：`~/.local/share/tmcl-data/` |

卸载时默认保留 `tmcl-data`；Windows 卸载界面勾选“删除应用数据”才会一并删除。

**macOS 未签名版本的打开方法**：Alpha 版没有经过 Apple 签名与公证，第一次双击会提示“无法验证开发者”或“已损坏”。可以任选一种方法打开：

1. 在 Finder 中按住 Control 点按 `TMCL.app`，选择“打开”，再在弹窗中点“打开”。macOS 15 及以后需要先双击一次，再到“系统设置 → 隐私与安全性”底部点“仍要打开”。
2. 或在终端执行 `xattr -dr com.apple.quarantine /Applications/TMCL.app`（路径换成实际位置）。

**Linux AppImage**：下载后执行 `chmod +x TMCL_*.AppImage` 再运行。部分发行版需要先安装 `libfuse2`。

## 开发

开发流程与分工见 [AGENTS.md](AGENTS.md) 与 [开发计划](docs/PLAN.md)。

### 微软登录 Client ID

微软正版登录需要一个在 Azure 注册的应用 Client ID。**Client ID 不会提交到仓库**，而是在构建时注入：

1. 在 [Azure 门户](https://portal.azure.com/) 注册自己的应用，并申请 Minecraft API 使用权限。
2. 在仓库根目录创建 `.env.local`（已被 `.gitignore` 忽略），写入：

   ```
   TMCL_MS_CLIENT_ID=你的-client-id
   ```

3. 构建时由 `core/auth/build.rs` 读取：环境变量 `TMCL_MS_CLIENT_ID` 优先，其次是 `.env.local`。没有配置或格式不是 GUID 时，微软登录不可用，其余功能不受影响；应用的 `app_info` 会返回 `ms_login_enabled: false`。
4. 第一次新建 `.env.local` 后执行一次 `cargo clean -p tmcl-auth`，之后修改或删除该文件会自动触发重新构建。

自行构建和分发 TMCL 的开发者需要使用自己的 Client ID。

## 参与贡献

欢迎通过 Issue 提交功能建议和 Bug，或 Fork 后提交 Pull Request。

## 许可证

本项目以 [GPL-3.0](LICENSE) 许可证开源。

---

## English

TMCL is an open-source, cross-platform launcher for Minecraft: Java Edition. Downloading the game, installing mods and modpacks, changing skins, managing multiple instances and playing with friends all happen in one client.

> ⚠️ TMCL is in alpha. It can download and launch the game (including Forge, NeoForge, Fabric and Quilt) with all three account types; other features are still in development and may have issues. See the [project outline](TMCL%20项目大纲.md) (Chinese) for the design and the [alpha test matrix](docs/testing/alpha-matrix.md) (Chinese) for test results and known issues.
>
> TMCL is an unofficial launcher and is not affiliated with Mojang Studios or Microsoft.

### Planned features

- **First release (P0):** one-click game download (official source or mirrors), automatic Java management, Microsoft / offline / authlib-injector accounts, isolated instances, Forge / NeoForge / Fabric / Quilt installation.
- **Next (P1):** Modrinth and CurseForge modpacks, mod and resource browsing, skin changer with 3D preview, settings and keybind sync across instances, P2P multiplayer and built-in SakuraFrp tunneling.
- **Later (P2):** seed map preview, crash log analysis, auto-update, themes.

The UI supports Simplified Chinese and English, targeting Windows, macOS and Linux.

### Tech stack

Tauri 2, a Rust core, and a Vue 3 + TypeScript frontend.

### Install

Download the package for your platform from [GitHub Releases](https://github.com/Jack-TJY/TMCL/releases). TMCL keeps settings, instances and game files in a `tmcl-data/` folder next to where it is installed, so install it somewhere you can write to.

- **Windows 10 / 11:** `TMCL_<version>_x64-setup.exe` installs for the current user without admin rights, by default to `%LOCALAPPDATA%\TMCL`. Data lives in `tmcl-data\` inside the install folder.
- **macOS 10.15+:** open the `.dmg` and drag `TMCL.app` to Applications or any folder. Data lives in `tmcl-data/` next to `TMCL.app`.
- **Linux:** the `.AppImage` (recommended) keeps data in `tmcl-data/` next to the `.AppImage` file; the `.deb` uses `~/.local/share/tmcl-data/`. Run `chmod +x TMCL_*.AppImage` first; some distributions need `libfuse2`.

Uninstalling keeps `tmcl-data` unless you tick "Delete the application data" in the Windows uninstaller.

**Opening the unsigned macOS build:** alpha builds are not signed or notarized by Apple, so the first launch says the developer cannot be verified or the app is damaged. Either Control-click `TMCL.app` in Finder, choose Open and confirm (on macOS 15 and later, double-click once, then click "Open Anyway" at the bottom of System Settings → Privacy & Security), or run `xattr -dr com.apple.quarantine /Applications/TMCL.app` with the actual path.

### Microsoft login Client ID

Microsoft login needs an Azure application Client ID, which is **never committed**. It is injected at build time from `.env.local` (ignored by git):

```
TMCL_MS_CLIENT_ID=your-client-id
```

The `TMCL_MS_CLIENT_ID` environment variable takes precedence over `.env.local`. After creating `.env.local` for the first time, run `cargo clean -p tmcl-auth` once; later edits are picked up automatically.

Anyone building and distributing TMCL should register and use their own Client ID. Without it, Microsoft login is disabled and everything else still works.

### License

TMCL is licensed under [GPL-3.0](LICENSE).
