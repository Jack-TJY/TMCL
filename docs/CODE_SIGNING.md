# 代码签名政策 / Code signing policy

**简体中文** | [English](#english)

## 签名方

Windows 版由 [SignPath.io](https://signpath.io) 提供免费代码签名，证书由 [SignPath Foundation](https://signpath.org) 颁发。

> 状态：已向 SignPath Foundation 提交申请，**尚未生效**。在签名生效之前发布的 Windows 安装包没有签名，见 [README 的安装说明](../README.md#安装)。

## 签名范围

- 只签名 TMCL 自己的源码在 GitHub Actions 上构建出的 Windows 安装包（`TMCL_<版本>_x64-setup.exe`），发布在 [GitHub Releases](https://github.com/Jack-TJY/TMCL/releases)。
- 不签名第三方程序。例如 P2P 联机用到的 [EasyTier](https://github.com/EasyTier/EasyTier) 不打进安装包，用户第一次使用联机功能时才从 EasyTier 官方 Release 下载。
- macOS 与 Linux 安装包目前没有签名，请只从上面的 GitHub Releases 下载。

## 构建与签名流程

1. 维护者推送 `v*` 版本 tag，触发公开的 [build.yml](../.github/workflows/build.yml) 工作流，在 GitHub 托管的 Runner 上从该 tag 的源码构建。
2. 工作流把未签名的安装包上传为 workflow artifact，并通过 SignPath 的 GitHub Action 提交签名请求。SignPath 会核对它来自本仓库的这次构建。
3. 每个签名请求都要由下方的审批人在 SignPath 上手动批准。签名私钥保存在 SignPath 的 HSM 中，项目成员拿不到。
4. 签名后的安装包由同一工作流下载并上传，维护者再把它发布到 GitHub Releases。

签名配置（产品名必须是 `TMCL`、只签名 PE 文件等）记录在 [.signpath/artifact-configuration.xml](../.signpath/artifact-configuration.xml)。

## 团队角色

| 角色 | 成员 |
| --- | --- |
| 提交者与评审者（Committers and reviewers） | [Jack-TJY](https://github.com/Jack-TJY) |
| 审批人（Approvers） | [Jack-TJY](https://github.com/Jack-TJY) |

本项目只有一位维护者。所有外部 Pull Request（包括 AI 编程助手起草的改动）都由维护者评审后才合并进 `main`，只有维护者能合并和发布。维护者的 GitHub 与 SignPath 账号都开启了多因素认证。

## 隐私政策

除非用户本人或安装、操作 TMCL 的人明确要求，TMCL 不会把任何信息传给其他联网系统。TMCL 没有遥测、统计或崩溃上报。

只有在用户使用对应功能时，TMCL 才会访问这些服务：

| 功能 | 访问的服务 |
| --- | --- |
| 下载游戏、Java、模组加载器 | Mojang / Microsoft 官方源、Forge、NeoForge、Fabric、Quilt 官方源，以及 [BMCLAPI](https://bmclapi2.bangbang93.com) 镜像 |
| 微软账号登录 | Microsoft、Xbox Live、Minecraft 服务（[Microsoft 隐私声明](https://privacy.microsoft.com/privacystatement)） |
| 外置登录（authlib-injector） | 用户自己填写的验证服务器 |
| 浏览、安装模组和整合包 | [Modrinth](https://modrinth.com/legal/privacy)、[CurseForge](https://www.curseforge.com)、MCIM 镜像 |
| P2P 联机 | EasyTier 公共节点（交换联机所需的网络地址） |
| 樱花穿透 | [SakuraFrp](https://www.natfrp.com)（用户自己的账号） |

各服务如何处理数据，以它们自己的隐私政策为准。

---

## English

### Signer

Free code signing for the Windows build is provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org).

> Status: an application to SignPath Foundation has been submitted and is **not active yet**. Windows installers released before then are unsigned; see the [install notes in the README](../README.md#install).

### What is signed

- Only the Windows installer (`TMCL_<version>_x64-setup.exe`) built on GitHub Actions from TMCL's own source and published on [GitHub Releases](https://github.com/Jack-TJY/TMCL/releases).
- No third-party programs are signed. For example, [EasyTier](https://github.com/EasyTier/EasyTier), used for P2P multiplayer, is not bundled in the installer; it is downloaded from EasyTier's official releases the first time the user opens the multiplayer feature.
- macOS and Linux packages are currently unsigned. Download them only from the GitHub Releases page above.

### Build and signing process

1. The maintainer pushes a `v*` tag, which runs the public [build.yml](../.github/workflows/build.yml) workflow on GitHub-hosted runners, building from the tagged source.
2. The workflow uploads the unsigned installer as a workflow artifact and submits a signing request through SignPath's GitHub Action. SignPath verifies that it comes from this repository's build.
3. Every signing request must be approved manually on SignPath by an approver listed below. The private key is kept in SignPath's HSM and is never available to project members.
4. The same workflow downloads and uploads the signed installer, and the maintainer publishes it on GitHub Releases.

The signing configuration (product name must be `TMCL`, only PE files are signed, and so on) is kept in [.signpath/artifact-configuration.xml](../.signpath/artifact-configuration.xml).

### Team roles

| Role | Members |
| --- | --- |
| Committers and reviewers | [Jack-TJY](https://github.com/Jack-TJY) |
| Approvers | [Jack-TJY](https://github.com/Jack-TJY) |

TMCL has a single maintainer. Every pull request from outside, including changes drafted by AI coding assistants, is reviewed by the maintainer before it is merged into `main`, and only the maintainer can merge and release. The maintainer uses multi-factor authentication for both GitHub and SignPath.

### Privacy policy

This program will not transfer any information to other networked systems unless specifically requested by the user or the person installing or operating it. TMCL has no telemetry, analytics or crash reporting.

TMCL contacts these services only when the user uses the matching feature:

| Feature | Services contacted |
| --- | --- |
| Downloading the game, Java and mod loaders | Official Mojang / Microsoft, Forge, NeoForge, Fabric and Quilt sources, and the [BMCLAPI](https://bmclapi2.bangbang93.com) mirror |
| Microsoft account login | Microsoft, Xbox Live and Minecraft services ([Microsoft Privacy Statement](https://privacy.microsoft.com/privacystatement)) |
| External login (authlib-injector) | The authentication server the user enters |
| Browsing and installing mods and modpacks | [Modrinth](https://modrinth.com/legal/privacy), [CurseForge](https://www.curseforge.com), the MCIM mirror |
| P2P multiplayer | EasyTier public nodes (to exchange the network addresses needed to connect) |
| SakuraFrp tunneling | [SakuraFrp](https://www.natfrp.com) (the user's own account) |

How each service handles data is governed by its own privacy policy.
