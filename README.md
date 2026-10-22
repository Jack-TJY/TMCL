# TMCL

**简体中文** | [English](#english)

TMCL 是一个开源、跨平台的我的世界（Java 版）启动器。下载游戏、安装模组和整合包、换皮肤、管理多版本、和朋友联机，都在一个客户端里完成。

> ⚠️ 项目处于早期规划阶段，还没有可用的版本。设计细节见 [TMCL 项目大纲](TMCL%20项目大纲.md)。
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

## 开发

项目代码尚未建立，以下为计划中的约定。

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

> ⚠️ The project is in early planning and has no usable release yet. See the [project outline](TMCL%20项目大纲.md) (Chinese) for the design.
>
> TMCL is an unofficial launcher and is not affiliated with Mojang Studios or Microsoft.

### Planned features

- **First release (P0):** one-click game download (official source or mirrors), automatic Java management, Microsoft / offline / authlib-injector accounts, isolated instances, Forge / NeoForge / Fabric / Quilt installation.
- **Next (P1):** Modrinth and CurseForge modpacks, mod and resource browsing, skin changer with 3D preview, settings and keybind sync across instances, P2P multiplayer and built-in SakuraFrp tunneling.
- **Later (P2):** seed map preview, crash log analysis, auto-update, themes.

The UI supports Simplified Chinese and English, targeting Windows, macOS and Linux.

### Tech stack

Tauri 2, a Rust core, and a Vue 3 + TypeScript frontend.

### Microsoft login Client ID

Microsoft login needs an Azure application Client ID, which is **never committed**. It is injected at build time from `.env.local` (ignored by git):

```
TMCL_MS_CLIENT_ID=your-client-id
```

The `TMCL_MS_CLIENT_ID` environment variable takes precedence over `.env.local`. After creating `.env.local` for the first time, run `cargo clean -p tmcl-auth` once; later edits are picked up automatically.

Anyone building and distributing TMCL should register and use their own Client ID. Without it, Microsoft login is disabled and everything else still works.

### License

TMCL is licensed under [GPL-3.0](LICENSE).
