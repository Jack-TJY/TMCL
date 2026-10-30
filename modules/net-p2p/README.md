# modules/net-p2p

tmcl-p2p：P2P 直连联机（PLAN.md M2-12）。方案与取舍见 [docs/research/p2p.md](../../docs/research/p2p.md)。

- 底层用 [EasyTier](https://github.com/EasyTier/EasyTier)（LGPL-3.0），作为独立程序在首次联机时从官方 Releases 下载，按内置 SHA-256 校验后放在 `<data_root>/p2p/easytier/<版本>/`，不与 TMCL 链接。
- 房间码与联机流程遵循 [Scaffolding 协议](https://github.com/Scaffolding-MC/Scaffolding-MC)（MIT），与 HMCL、PCL-CE、陶瓦联机互通。
- 默认公共节点与陶瓦联机相同，用户可追加自己的节点。

升级 EasyTier 时修改 `src/easytier.rs` 中的 `EASYTIER_VERSION` 和 `ASSETS`（每个平台压缩包的 SHA-256 与字节数，需实际下载核对）。

真机冒烟（下载 EasyTier，在本机起一个房主和一个房客，经公共节点互相发现）：

```
cargo run -p tmcl-p2p --example smoke
```
