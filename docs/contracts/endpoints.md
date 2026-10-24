# 外部端点

> 契约文档（受保护）。登记启动器访问的第三方服务地址、用途与核实时间。新增按模块追加小节；改地址需写明原因与核实方式。

## java（M1-05）

| 用途 | 地址 | 说明 | 核实 |
| --- | --- | --- | --- |
| Java 运行时总索引 | `https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json` | Mojang 官方启动器使用的索引，按平台（`windows-x64`、`windows-x86`、`windows-arm64`、`mac-os`、`mac-os-arm64`、`linux`、`linux-i386`）与组件（`jre-legacy`、`java-runtime-alpha/beta/gamma/delta/epsilon`）列出清单地址与 SHA1。`launchermeta.mojang.com` 下同一路径返回相同内容 | 2026-10-23 实测 200 |
| 运行时清单与文件 | `https://piston-meta.mojang.com/v1/packages/<sha1>/manifest.json`、`https://piston-data.mojang.com/v1/objects/<sha1>/<name>` | 由总索引给出，不在代码中拼接；只下载 `raw`，不使用 `lzma` | 2026-10-23 下载 Java 17（403 个文件）与 Java 8 成功 |
| Adoptium 回退 | `https://api.adoptium.net/v3/assets/latest/<major>/hotspot?architecture=<x64\|x86-32\|aarch64\|arm>&image_type=<jre\|jdk>&os=<windows\|mac\|linux>&vendor=eclipse` | 官方索引没有该平台/组件或下载失败时使用；先查 `jre`，没有再查 `jdk`；压缩包按返回的 SHA256 校验 | 2026-10-23 下载 Temurin 21 JRE 成功 |

组件与主版本的对应：8 → `jre-legacy`，16 → `java-runtime-alpha`，17 → `java-runtime-gamma`，21 → `java-runtime-delta`，25 → `java-runtime-epsilon`。版本 JSON 带 `javaVersion.component` 时优先用它。

截至核实日，官方索引在 `mac-os-arm64` 与 `windows-arm64` 上没有 `jre-legacy`（Java 8），这两种平台的 Java 8 会走 Adoptium。

## app（M1-16）

| 用途 | 地址 | 说明 | 核实 |
| --- | --- | --- | --- |
| 本项目发布版本 | `https://api.github.com/repos/Jack-TJY/TMCL/releases?per_page=20` | `app_releases` 使用，主页“更新日志”组件展示。未认证接口限每 IP 每小时 60 次，结果在本地缓存 1 小时；只读取 `tag_name`、`name`、`published_at`、`prerelease`、`draft`、`html_url` | 2026-10-24 实测 200（当前没有发布版本，返回 `[]`） |
