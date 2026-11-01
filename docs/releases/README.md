# 发布说明

每个发布版本在 GitHub Releases 上的说明都保存在本目录，文件名为 `v<版本号>.md`，发布时用 `--notes-file` 传给 `gh release create`（步骤见 [beta-plan.md](../testing/beta-plan.md) §6）。

## 约定

- **Release 标题**：`TMCL v<版本号>`，例如 `TMCL v0.2.0-beta.1`。启动器首页“更新日志”组件直接显示这个标题。
- **tag**：`v<版本号>`。预发布版本（alpha、beta）勾选 Pre-release。
- **内容来源**：“新增 / 修复 / 已知问题 / 致谢”从 [CHANGELOG.md](../../CHANGELOG.md) 对应版本一节复制，其余部分按下面的模板补齐。

## 模板

```md
TMCL 的<第几个 / 什么>版本。<一两句概括本版内容>。这是测试版本，可能存在问题，欢迎在 [Issues](https://github.com/Jack-TJY/TMCL/issues) 反馈（附上“设置 → 存储 → 导出诊断信息”生成的 zip 更好）。

> TMCL 是非官方启动器，与 Mojang Studios 和 Microsoft 没有关联。

## 新增

（来自 CHANGELOG）

## 修复

（来自 CHANGELOG；首个版本可省略）

## 已测试

<平台与范围>，详见 <测试矩阵链接>。

## 已知问题

（来自 CHANGELOG）
- 暂不支持自动更新，新版本需要手动下载。

## 下载

| 平台 | 文件 |
| --- | --- |
| Windows 10 / 11（64 位） | `TMCL_<版本>_x64-setup.exe` |
| macOS（Apple Silicon） | `TMCL_<版本>_aarch64.dmg` |
| Linux | `TMCL_<版本>_amd64.AppImage`（推荐）或 `TMCL_<版本>_amd64.deb` |

数据（设置、实例、游戏文件）在安装位置旁的 `tmcl-data/` 中，卸载与升级时保留。校验和见 `SHA256SUMS.txt`。

## 致谢

（来自 CHANGELOG）

---

## English

<中文开头一段的英文版>

> TMCL is an unofficial launcher and is not affiliated with Mojang Studios or Microsoft.

**New:** …

**Known issues:** …

Checksums are in `SHA256SUMS.txt`.
```
