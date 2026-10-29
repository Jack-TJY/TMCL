# P2P 联机方案调研（M2-11）

调研日期：2026-10-29 · 调研人：claude-p2p · 前提：中继服务器问题挂起（PLAN.md 第 9 节）

本报告供维护者为 M2-12 选定直连方案。**结论（2026-10-29 维护者按推荐选定）**：以 EasyTier 为底层，在 `modules/net-p2p` 里实现 Scaffolding 联机协议，与 HMCL、PCL-CE、陶瓦联机的房间互通。理由和取舍见第 5 节。

## 1. 先看别人怎么做

国内主流启动器在 2025 到 2026 年基本都收敛到了同一条路线：**EasyTier + 陶瓦联机（Terracotta）+ Scaffolding 协议**。

| 启动器 | 联机方式 | 来源 |
| --- | --- | --- |
| HMCL | 3.7.1 起内置陶瓦联机，入口在主界面左下角 | [HMCL 3.7.1 更新日志](https://www.bilibili.com/opus/1124733750249259010)（“GP-4215: 在启动器中内置 Terracotta”） |
| PCL-CE | 接入陶瓦联机 | [陶瓦文档：使用陶瓦联机的程序列表](https://github.com/burningtnt/Terracotta/blob/master/sites/docs/integrations.md) |
| FCL、ZalithLauncher2、FMCL、MoLaunch、Axolotl | 内置陶瓦，或实现 Scaffolding 协议与陶瓦互通 | 同上 |
| VoxLink（模组） | 在游戏内集成陶瓦的 P2P 能力 | 同上 |

几个关键事实（读自陶瓦仓库 2026-10-24 的源码与文档）：

- **陶瓦联机**：Rust 编写，许可证 AGPL-3.0-or-later，附两条例外：①打包**未修改**的二进制、且不静态或动态链接；②通过它的进程间接口（HTTP API）与未修改的程序交互，**并在界面明显处标出陶瓦的版权信息**。满足任一条，宿主程序不受 AGPL 约束。
- 陶瓦在桌面端把 `easytier-core` 当作子进程拉起（`src/easytier/executable_impl.rs`），在 Android 上把 EasyTier 当库链接。当前捆绑的 EasyTier 版本是 `v2.5.0-terracotta.2`（作者的分支）。
- 陶瓦对外提供一个只监听 `127.0.0.1` 的 HTTP API（无鉴权），启动器用 `terracotta --hmcl <file>` 拉起它，从 `<file>` 读端口，然后轮询 `GET /state`、调用创建或加入房间的接口。文档：[http-api.md](https://github.com/burningtnt/Terracotta/blob/master/sites/docs/http-api.md)。
- **Scaffolding 协议**（MIT 许可，[Scaffolding-MC/Scaffolding-MC](https://github.com/Scaffolding-MC/Scaffolding-MC)）规定了互通所需的全部约定：
  - 房间码格式 `U/NNNN-NNNN-SSSS-SSSS`，由密码学安全随机数生成，按 34 进制小端读出的整数要能被 7 整除（用作校验）。
  - EasyTier 网络名为 `scaffolding-mc-NNNN-NNNN`，网络密钥为 `SSSS-SSSS`。也就是说，**房间码本身就是密钥**，不需要任何服务器来分发。
  - 房主的 EasyTier 主机名为 `scaffolding-mc-server-<端口>`，在虚拟 IP 的这个端口上开一个简单的 TCP 请求/响应服务，提供 `c:ping`、`c:protocols`、`c:server_port`、玩家列表等标准请求。
  - 房客查到 MC 端口后，用 EasyTier 的端口转发在本机开一个端口，再用一个假服务器（`src/mc/fakeserver`）让它出现在多人游戏的局域网列表里。
- 陶瓦默认使用的 EasyTier 公共节点（`src/easytier/publics.rs`）：`tcp://public.easytier.top:11010`、`tcp://public2.easytier.cn:54321`、`https://etnode.zkitefly.eu.org/node1`、`/node2`。后两个由社区成员 zkitefly 提供，[说明页](https://etnode.zkitefly.eu.org/about)写明“两节点均禁中转”，node1 只对中国大陆开放。

结论：只要实现了 Scaffolding 协议，TMCL 的用户就能和 HMCL、PCL-CE 的用户互相加入房间。这对一个新启动器来说价值很大，因为联机的另一方往往不用 TMCL。

## 2. 候选方案

| 编号 | 方案 | 一句话 |
| --- | --- | --- |
| P1 | 内置陶瓦联机（子进程 + HTTP API） | 照抄 HMCL 的做法，工作量最小 |
| P2 | EasyTier + 自己实现 Scaffolding 协议 | 与 HMCL、PCL-CE 互通，界面和流程完全自己掌控（**推荐**） |
| P3 | EasyTier + 自定义房间格式 | 和 P2 一样用 EasyTier，但不与其他启动器互通 |
| P4 | 自研：STUN + UDP 打洞 + QUIC（quinn） | PLAN.md 原先列的方案，完全自主，但要自建信令服务器 |
| P5 | libp2p 或 iroh | Rust 生态的 P2P 框架，自带打洞，但同样依赖引导或中继节点 |
| P6 | n2n | C 语言的老牌 P2P VPN |
| P7 | Tailscale、Headscale、ZeroTier、NetBird 一类 | 成熟的组网工具，但要账号或控制服务器 |

## 3. 逐项比较

### 3.1 总表

| 维度 | P1 陶瓦 | P2 EasyTier+Scaffolding | P3 EasyTier 自定义 | P4 自研 | P5 libp2p / iroh | P6 n2n | P7 Tailscale 类 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| NAT 穿透能力 | 同 EasyTier | UDP 打洞、IPv6，官方称能在 NAT4-NAT4（双方对称 NAT）下工作 | 同 P2 | 取决于实现；对称 NAT 两端通常打不通 | libp2p DCUtR 和 iroh 都支持打洞，对称 NAT 回落中继 | 依赖 supernode，打洞能力一般 | 很强，但靠 DERP 等中继兜底 |
| 必须自建的服务器 | 无（用社区公共节点） | 无（用社区公共节点，可选自建） | 同 P2 | **信令服务器必需**，STUN 可用公共的 | 引导节点或中继（iroh 有 n0 公共中继） | **supernode 必需** | 控制服务器（官方或自建 Headscale） |
| 与其他启动器互通 | 是 | 是 | 否 | 否 | 否 | 否 | 否 |
| 许可证（与 GPL-3.0） | AGPL + 例外，按例外使用可行，但有界面署名义务 | EasyTier LGPL-3.0，兼容；Scaffolding MIT，兼容 | 同 P2 | 自己的代码 | MIT / Apache-2.0，兼容 | GPL-3.0，兼容 | 客户端 BSD-3 等，控制面另论 |
| Rust 集成方式 | 子进程 + 本地 HTTP | `easytier-core` 子进程（或 git 依赖链接） | 同 P2 | 纯 Rust（quinn、stun 库） | 纯 Rust crate | C 程序子进程 | Go 程序子进程或 tsnet |
| 需要虚拟网卡 / 管理员权限 | 否（no-tun 模式） | 否（no-tun + 端口转发） | 否 | 否 | 否 | **是**（TAP） | 通常是 |
| 加密 | EasyTier（AES-GCM 或 WireGuard） | 同左 | 同左 | 自己做（QUIC 自带 TLS 1.3） | Noise / QUIC | 可选 | WireGuard |
| 工作量（粗估） | S～M | M～L | M | L～XL | L | M | M，且体验不合适 |
| 主要风险 | 依赖第三方二进制与其 API 稳定性；署名义务 | 协议仍在演进；依赖社区公共节点 | 用户少、无互通 | 自建信令要运维；打洞成功率要自己调 | 同样要节点；与 MC 生态无互通 | 需驱动，已少有人用 | 要账号，不适合临时联机 |

数据来源：EasyTier 仓库 README（2026-10-29 主分支，版本 2.7.0，LICENSE 文件为 LGPL-3.0）、陶瓦仓库源码与文档。P5 到 P7 的许可证与特性来自各项目的公开资料，本次没有逐一下载核对，实施前需复核。

### 3.2 NAT 穿透成功率

没有找到公开、可信的成功率统计，本次也没有条件做两地实测，因此只能给定性判断：

- EasyTier 支持 UDP 打洞、IPv6 直连和 TCP 等多种连接方式，官方称在双方都是对称 NAT（NAT4）的网络里也能工作。国内家庭宽带 IPv6 普及率较高，IPv6 直连往往比 IPv4 打洞更容易成功。
- 打洞失败时，EasyTier 会通过公共节点中转。**但陶瓦默认的 zkitefly 节点禁止中转**，`public.easytier.top` 的中转策略本次没查到明确说明。所以“两端都打不通”的组合在不建中继的前提下仍会失败，这正是 M2-12 里 `P2P_DIRECT_FAILED` 加“建议使用樱花穿透”的用武之地。
- P4 自研方案在对称 NAT 对对称 NAT 时基本打不通，要达到 EasyTier 现有水平需要大量调优，性价比低。
- 建议 M2-12 验收时，在至少三类网络组合（两端都是普通家宽、一端校园网或移动热点、一端有 IPv6）下记录成功率，写进测试报告。

### 3.3 服务器需求与费用

| 角色 | P1～P3 | P4 | 说明 |
| --- | --- | --- | --- |
| 信令 / 发现 | EasyTier 公共节点兼任，免费 | **必须自建**，最低一台 1 核 1 GB 的 VPS，粗估国内 50～100 元/月（未询价，备案等成本另计） | P1～P3 的房间码就是网络名和密钥，不需要单独的信令服务 |
| STUN | EasyTier 内置 | 可用公共 STUN（如各大厂商的公共 STUN 服务器），免费 | |
| 中继 | 挂起；公共节点是否中转由节点决定 | 挂起 | 中继流量大，费用按带宽算，是维护者需要决定的问题（第 6 节） |
| 可选：自建“只协助打洞”节点 | `easytier-core --relay-network-whitelist --relay-all-peer-rpc` 只转发 RPC 不转发游戏流量，带宽需求很小，一台最低配 VPS 即可 | 不适用 | 用来降低对社区公共节点的依赖，见 [EasyTier 文档](https://easytier.cn/en/guide/network/host-public-server.html) |

### 3.4 Rust 集成方式（P2）

- **推荐子进程**：把官方发布的 `easytier-core` 作为 Tauri 的 `externalBin` 随安装包分发，在构建时从 EasyTier 的 GitHub Releases 下载并按固定 SHA-256 校验。陶瓦桌面端也是这么做的。
- **不推荐作为库链接**：crates.io 上的 `easytier` 停在 2.0.3（2024-10），当时的元数据许可证还是 Apache-2.0；新版只能用 git 依赖，依赖树大、编译慢，tokio 等版本要和我们对齐。陶瓦在 Android 上链接时还特意注释“DO NOT upgrade their version”。
- 控制方式：用命令行参数启动（`--no-tun`、`--network-name`、`--network-secret`、`-p <公共节点>`、`--hostname`），用 `easytier-cli` 或其 RPC 接口查询对端列表和 NAT 类型、动态添加端口转发。具体接口在 M2-12 开工时按当时的 EasyTier 版本确认。
- LGPL-3.0 义务：以独立程序分发时，附上 EasyTier 的许可证文本和源码获取方式即可（放进“关于”页的第三方许可证列表）。

### 3.5 安全性

- **加密**：EasyTier 网络流量用 AES-GCM 或 WireGuard 加密；Scaffolding 房间码里的 `SSSS-SSSS` 就是网络密钥。房间码有 16 位 34 进制字符，约 81 位熵，其中一半是网络名、一半是密钥，密钥部分约 40 位，足以防止临时房间被猜中，但不适合长期公开。界面上应提醒“房间码等于钥匙，只发给朋友”。
- **只转发游戏端口**：两侧都用 `--no-tun`，不创建虚拟网卡，房客只在本机开一个转发到房主游戏端口的监听。房主一侧在 no-tun 模式下还能被网络内的成员访问哪些端口，**需要在 M2-12 实测确认**（例如用 EasyTier 的端口白名单或 ACL 只放行 MC 端口和 Scaffolding 端口）。这条是 M2-12 的安全验收项。
- **本地接口**：P1 的陶瓦 HTTP API 无鉴权，本机任何进程都能调用；P2 由我们自己控制，没有这个问题。
- **公共节点**：节点能看到网络名和连接元数据，看不到加密后的游戏内容。

## 4. 不选的方案为什么不选

- **P3**：和 P2 成本差不多，却放弃了互通，没有理由这样选。
- **P4**：要自建并长期运维信令服务器，打洞成功率要从零调，还和中继挂起的前提冲突最大。
- **P5**：libp2p 和 iroh 都是好库，但 Minecraft 联机生态已经站在 EasyTier 上，用它们得不到互通，也省不掉节点。
- **P6**：需要 TAP 驱动和管理员权限，supernode 必须自建。
- **P7**：要求双方登录账号或自建控制服务器，不适合“发个码就进来”的临时联机。

## 5. 推荐

**首选 P2：EasyTier 子进程 + 自研 Scaffolding 实现。**

- 与 HMCL、PCL-CE、陶瓦的用户互通，这是对新启动器最有价值的一点。
- 不需要自建任何服务器，符合“中继挂起”的前提；以后若决定自建节点，只是多加一个 `-p` 参数。
- 许可证干净：EasyTier LGPL-3.0 作为独立程序分发，Scaffolding 协议 MIT。
- 和 PLAN.md 中 M2-12 的文件划分（`room.rs`、`signal.rs`、`punch.rs`、`tunnel.rs`、`relay.rs`）能对上：`room.rs` 写房间码生成与解析，`signal.rs` 写 Scaffolding 的 TCP 请求响应，`tunnel.rs` 管 EasyTier 子进程和端口转发，`punch.rs` 在本方案里变成“读取 EasyTier 的连接与 NAT 状态”，`relay.rs` 保留 `trait Relay` 空实现。M2-12 开工时需要按此修订文件说明。

**备选 P1：直接内置陶瓦。** 如果维护者希望最快上线，可以先走 P1，以后再换 P2，房间码格式不变，用户无感。代价是界面上必须明显标出陶瓦的版权信息、依赖陶瓦的 HTTP API 稳定，并多带一个约十几 MB 的二进制。

## 6. 需要维护者决定的具体问题

1. 选 P2 还是 P1（或其他）？
2. 默认使用哪些 EasyTier 公共节点？建议与陶瓦相同的四个，并允许用户在设置里添加自己的节点。社区节点由个人维护，随时可能变化，要接受这个风险。
3. 是否联系公共节点的维护者（EasyTier 官方、zkitefly）告知 TMCL 将使用其节点？建议礼貌告知，由维护者本人发送。
4. 中继仍然挂起。若将来要解决“两端都打不通”，有三条路，需要维护者决定是否以及何时考虑：
   - 自建一个只协助打洞的 EasyTier 节点（带宽小，粗估 30～60 元/月的最低配 VPS，未询价），能提升成功率但不能解决全部失败；
   - 自建允许中转的节点，费用随联机人数和流量增长，需要限速、限时等滥用控制；
   - 不建，失败时引导用户用樱花穿透（见 [sakurafrp.md](sakurafrp.md)）。
5. 房主侧端口暴露范围在 M2-12 实测前，是否同意将“只暴露 MC 端口”列为 M2-12 的硬性验收项？

## 7. 决策表

请在选中的项前把 `[ ]` 改成 `[x]`，有补充写在“备注”里。

**2026-10-29 维护者决定：全部按推荐执行。**

| # | 问题 | 选项 | 备注 |
| --- | --- | --- | --- |
| 1 | 直连方案 | [x] P2 EasyTier + 自研 Scaffolding（推荐）<br>[ ] P1 内置陶瓦联机<br>[ ] 先 P1 上线，后续换 P2<br>[ ] 其他 | |
| 2 | 默认公共节点 | [x] 与陶瓦相同的四个，并允许用户自定义（推荐）<br>[ ] 只用 EasyTier 官方节点<br>[ ] 其他 | |
| 3 | 告知节点维护者 | [x] 由我本人发送，请 Agent 起草<br>[ ] 不需要 | |
| 4 | 中继 | [x] 继续挂起，失败时引导樱花穿透（推荐）<br>[ ] 评估自建“只协助打洞”节点<br>[ ] 评估自建中转节点 | |
| 5 | 安全验收 | [x] 同意把“房主只暴露 MC 与 Scaffolding 端口”列为 M2-12 硬性验收（推荐）<br>[ ] 不需要 | |

## 参考

- EasyTier：<https://github.com/EasyTier/EasyTier>（LGPL-3.0，2.7.0）· 文档 <https://easytier.cn/>
- 陶瓦联机：<https://github.com/burningtnt/Terracotta>（AGPL-3.0-or-later + 例外）
- Scaffolding 协议：<https://github.com/Scaffolding-MC/Scaffolding-MC>（MIT）
- HMCL 3.7.1 更新日志：<https://www.bilibili.com/opus/1124733750249259010>
- zkitefly 公共节点说明：<https://etnode.zkitefly.eu.org/about>
- 自建 EasyTier 公共节点：<https://easytier.cn/en/guide/network/host-public-server.html>
