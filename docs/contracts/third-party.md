# 第三方二进制组件

> 契约文档（受保护）。按 [AGENTS.md §14](../../AGENTS.md) 登记启动器使用的第三方二进制组件。

| 组件 | 来源 | 版本 | 许可证 | 校验 | 随安装包分发 |
| --- | --- | --- | --- | --- | --- |
| SakuraFrp 官方 frpc（M2-14） | `GET https://api.natfrp.com/v4/system/clients` 的 `frpc` 分类，见 [endpoints.md](endpoints.md#sakuram2-14) | 运行时取最新版（2026-10-30 为 `0.51.0-sakura-15.1`） | 闭源，官方未说明再分发条款 | 下载时按接口给出的大小与 MD5 校验 | 否，首次使用樱花穿透时下载到 `<data_root>/cache/sakura-frpc/`，作为独立进程运行 |
| EasyTier `easytier-core`、`easytier-cli`（M2-12） | 官方 GitHub Releases `https://github.com/EasyTier/EasyTier/releases/download/<版本>/easytier-<平台>-<版本>.zip`，见 `modules/net-p2p/src/easytier.rs` | 固定版本 `v2.6.4` | LGPL-3.0，未修改，作为独立程序使用；关于页列出许可证与源码地址 | 按代码内置的 SHA-256 与字节数校验 | 否，首次联机时下载到 `<data_root>/p2p/easytier/<版本>/`，作为独立进程运行 |
