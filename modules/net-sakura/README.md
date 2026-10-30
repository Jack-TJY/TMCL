# modules/net-sakura

tmcl-sakura：樱花穿透（SakuraFrp，PLAN.md M2-14，方案见 `docs/research/sakurafrp.md` 方案 A）。

- `api.rs`：SakuraFrp 开放 API v4（账号、节点、隧道、客户端下载信息、服务条款）。
- `frpc.rs`：运行时下载并校验官方 frpc（不随安装包分发），隐藏子进程运行，从日志解析联机地址。
- `tunnel.rs`：访问密钥（只存系统密钥库）与一次联机会话（建临时隧道、关闭时删除、清理残留）。

接口地址见 `docs/contracts/endpoints.md` 的 sakura 一节。功能模块只依赖 `core/*`，模块之间互不依赖，见 `docs/PLAN.md` 第 2 节。
