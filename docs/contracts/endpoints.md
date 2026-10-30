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

## resources（M2-01）

| 用途 | 地址 | 说明 | 核实 |
| --- | --- | --- | --- |
| Modrinth API v2 | `https://api.modrinth.com/v2` | 搜索 `/search`、项目 `/project/{id}`、`/projects`、版本 `/project/{id}/version`、`/version/{id}`、`/versions`、依赖 `/project/{id}/dependencies`、按哈希查询 `/version_file/{hash}`、`POST /version_files`、`POST /version_files/update`。`User-Agent` 必须为 `Jack-TJY/TMCL/<版本>`；速率限制 300 次/分钟，读 `X-Ratelimit-Limit/Remaining/Reset`，429 时等待后重试 | 2026-10-24 实测 200，响应头含 `X-Ratelimit-Limit: 300` |
| Modrinth 文件 CDN | `https://cdn.modrinth.com/data/<project_id>/versions/<version_id>/<filename>` | 由版本的 `files[].url` 给出，不在代码中拼接；按 `hashes.sha1` 与 `size` 校验 | 2026-10-24 下载 Sodium mc1.20.1-0.5.13-fabric 并校验 SHA1 成功 |
| Modrinth / CurseForge 文件镜像（MCIM，下载加速，默认关闭） | `https://mod.mcimirror.top/<路径>` | `core/net` 的内置镜像 `mcim`：`https://cdn.modrinth.com` 与 `https://edge.forgecdn.net` 换成 `https://mod.mcimirror.top`，路径不变（按 MCIM 文档不改写 `mediafilez.forgecdn.net`）。只在 `download.source` = `mcim` 或 `RESOURCE_MIRROR` 启用时使用，失败或校验不一致时回退官方源；不发送 `x-api-key`（该头只发给 `forgecdn.net`）。启用时须在下载界面标明来源，并提交 UA（mcmod-info-mirror/mcim-rust-api#12，原 mcim#4 已归档） | 2026-10-30 Windows 10 国内直连实测：Sodium、Fabulously Optimized `.mrpack`、Faithful 32x、JEI 的镜像地址全部 302 回 `cdn.modrinth.com` / `mediafilez.forgecdn.net`，SHA1 一致但比直连慢 1–2.5 s；MCIM README 注明“文件下载暂时不提供”，因此默认不启用 |

## resources · CurseForge（M2-02）

| 用途 | 地址 | 说明 | 核实 |
| --- | --- | --- | --- |
| CurseForge API v1 | `https://api.curseforge.com/v1` | 搜索 `/mods/search`、项目 `/mods/{id}`、`POST /mods`、文件 `/mods/{id}/files`、`/mods/{modId}/files/{fileId}`、`POST /mods/files`、按指纹查询 `POST /fingerprints/432`。请求头 `x-api-key`（构建期注入，见 AGENTS.md §11）；`User-Agent` 与 Modrinth 相同。按第三方 API 条款，响应不缓存到磁盘 | 2026-10-30 实测 200 |
| CurseForge 文件镜像（MCIM） | `https://mod.mcimirror.top/files/<id/1000>/<id%1000>/<filename>` | 只用于整合包中作者禁止第三方分发（`downloadUrl` 为空）、且 Modrinth 上按 SHA1 找不到同一文件的文件；路径与 `edge.forgecdn.net` 相同（后半段不补 0），实际 302 到 `mediafilez.forgecdn.net`。按 CurseForge 给出的 SHA1 校验；失败不影响安装，改为手动下载。不发送我们的 `x-api-key`。维护者已知悉这会绕过作者的分发设置并可能违反 CurseForge 第三方条款（2026-10-30）。MCIM 要求启动器提交 UA 到其白名单（mcmod-info-mirror/mcim#4），需维护者自行提交 | 2026-10-30 实测 BeeFix、entityculling、shotsfired 与两个资源包均 200 |
| CurseForge 文件 CDN | `https://edge.forgecdn.net/files/<a>/<b>/<filename>` | 由文件的 `downloadUrl` 给出，不在代码中拼接；对 `forgecdn.net` 的请求同样带 `x-api-key`（CurseForge 2025 年起要求）；按 `hashes`（algo 1 = SHA1）与 `fileLength` 校验。`downloadUrl` 为空表示作者禁止第三方分发，只能打开 `<websiteUrl>/files/<fileId>` 让用户手动下载 | 2026-10-30 下载 jei-1.20.1-forge-15.74.0.232.jar，SHA1 与指纹（MurmurHash2）都一致 |

## auth（M1-08）

| 用途 | 地址 | 说明 | 核实 |
| --- | --- | --- | --- |
| authlib-injector 发布索引 | `https://authlib-injector.yushi.moe/artifact/latest.json` | 返回 `version`、`download_url`、`checksums.sha256`；jar 下载后按 SHA-256 校验，存到 `cache/authlib-injector/` | 2026-10-24 国内直连与海外代理均下载并校验成功 |
| authlib-injector 镜像 | `https://bmclapi2.bangbang93.com/mirrors/authlib-injector/artifact/latest.json` | 官方索引失败时使用，结构相同 | 未单独实测 |
| Yggdrasil 皮肤站 | 用户输入 | 先 GET 用户输入的地址，按响应头 `X-Authlib-Injector-API-Location` 找到 API 根；登录、校验、刷新分别 POST `<api>/authserver/{authenticate,validate,refresh}` | 2026-10-24 LittleSkin 元数据（`littleskin.cn` 经 ALI 指向 `/api/yggdrasil`）实测；登录需维护者用测试账号复测 |

## auth（M0-10）

| 用途 | 地址 | 说明 | 核实 |
| --- | --- | --- | --- |
| 微软设备码 | `https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode` | 表单 POST `client_id`、`scope=XboxLive.signin offline_access`，返回 `user_code`、`device_code`、`verification_uri`、`expires_in`、`interval` | 2026-10-22、2026-10-24 真实账号实测 200 |
| 微软令牌 | `https://login.microsoftonline.com/consumers/oauth2/v2.0/token` | 表单 POST。设备码轮询 `grant_type=urn:ietf:params:oauth:grant-type:device_code`，按 `interval` 等待，`authorization_pending` 继续、`slow_down` 间隔加 5 秒；续期 `grant_type=refresh_token`，`invalid_grant` 需重新登录 | 同上 |
| Xbox Live 用户认证 | `https://user.auth.xboxlive.com/user/authenticate` | JSON POST，`RpsTicket` 为 `d=<微软访问令牌>`，返回 `Token` 与 `DisplayClaims.xui[0].uhs` | 同上 |
| XSTS 授权 | `https://xsts.auth.xboxlive.com/xsts/authorize` | JSON POST，`RelyingParty` 为 `rp://api.minecraftservices.com/`；401 响应的 `XErr` 2148916233（没有 Xbox 档案）、2148916238（未成年账号）单独映射 | 同上 |
| Minecraft 登录 | `https://api.minecraftservices.com/authentication/login_with_xbox` | JSON POST `identityToken: "XBL3.0 x=<uhs>;<XSTS 令牌>"`，返回 `access_token`、`expires_in`；Azure 应用未获 Mojang 批准时返回 403 | 2026-10-24 审核通过后实测 200 |
| 权益与档案 | `https://api.minecraftservices.com/entitlements/mcstore`、`https://api.minecraftservices.com/minecraft/profile` | `Authorization: Bearer <Minecraft 访问令牌>`；档案 404 表示没有档案，此时权益为空视为未购买。Xbox Game Pass 账号权益可能为空但有档案 | 2026-10-24 实测 200 |

## skin（M2-07）

| 用途 | 地址 | 说明 | 核实 |
| --- | --- | --- | --- |
| 档案皮肤与披风 | `GET https://api.minecraftservices.com/minecraft/profile` | `Authorization: Bearer <Minecraft 访问令牌>`，返回 `skins[]`（`id`、`state`、`url`、`variant: CLASSIC\|SLIM`）与 `capes[]`（`id`、`state`、`url`、`alias`）；401 视为令牌失效，404 为没有档案 | 未实测，需维护者用正版账号验证 |
| 上传皮肤 | `POST https://api.minecraftservices.com/minecraft/profile/skins` | multipart：`variant`（`classic`\|`slim`）与 `file`（PNG）；成功返回新档案；400 视为图片不合法 | 同上 |
| 切换 / 隐藏披风 | `PUT` / `DELETE https://api.minecraftservices.com/minecraft/profile/capes/active` | `PUT` 的 JSON 体为 `{"capeId": "<id>"}`；成功返回新档案 | 同上 |
| 皮肤与披风图片 | `GET https://textures.minecraft.net/texture/<哈希>` | 档案 `skins[].url` / `capes[].url` 给出的 PNG（地址可能是 `http://`，启动器一律改用 HTTPS）；只用于界面预览（`skin_texture`） | 未实测，需维护者用正版账号验证 |
| 外置登录皮肤站页面 | 账号保存的 Yggdrasil API 地址 | GET API 根读取元数据的 `meta.links.homepage`，没有时用 API 地址的站点根；只在系统浏览器中打开 | 离线测试 |

## sakura（M2-14）

| 用途 | 地址 | 说明 | 核实 |
| --- | --- | --- | --- |
| SakuraFrp 开放 API v4 | `https://api.natfrp.com/v4` | 账号 `GET /user/info`（校验访问密钥；`realname` > 0 为已实名，有 `ban` 字段为已冻结）、节点 `GET /nodes`（键为节点 ID；`flag` 位：`1<<2` 可建隧道、`1<<9` 离线、`1<<11` 允许公开游戏）、隧道 `GET /tunnels`、`POST /tunnels`（JSON：`name`、`type=tcp`、`node`、`local_ip`、`local_port`、`note`，返回 201 与 `id`）、`POST /tunnel/delete`（JSON `ids` 逗号分隔，每次最多 10 条）。请求头 `Authorization: Bearer <访问密钥>`，密钥只存系统密钥库（键 `tmcl:sakura:access-key`），不写日志。401 视为密钥无效；其余 4xx 的 `{code,msg}` 中 `msg` 原样交给界面 | 2026-10-30 实测下方两个公开接口；需要密钥的接口离线测试，待维护者用自己的账号真机验证 |
| SakuraFrp 客户端下载信息 | `GET https://api.natfrp.com/v4/system/clients` | 不需要登录。顶层键为分类（`windows`、`macos`、`android`、`unix`、`frpc`、`other`），只用 `frpc`：`ver`（如 `0.51.0-sakura-15.1`）与 `archs.<os>_<arch>`（`windows_amd64`、`linux_arm64`、`darwin_arm64` 等）的 `url`、`hash`、`size`；没有 `hash` 的条目（Docker 链接）跳过 | 2026-10-30 Windows 10 国内直连实测 200 |
| frpc 下载 | `https://nya.globalslb.net/natfrp/client/frpc/<ver>/frpc_<os>_<arch>[.exe]` | 由上一行的 `url` 给出，不在代码中拼接；未压缩的可执行文件，按 `size` 与 `hash`（MD5）校验，存到 `<data_root>/cache/sakura-frpc/<ver>/`，新版本可用后删除旧版本。运行参数 `-f <密钥>:<隧道 ID> -n --disable_log_color`，Windows 上不弹出控制台窗口 | 2026-10-30 下载 `frpc_windows_amd64.exe`，大小 14209024 与 MD5 均与接口一致 |
| SakuraFrp 服务条款 | `GET https://api.natfrp.com/v4/system/policy?type=tos` | 不需要登录，返回 `updated`（如 `2024/02/13`）、`alert`、`content`（Markdown，UTF-8）；首次使用时在界面展示 | 2026-10-30 实测 200 |
