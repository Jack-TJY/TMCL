# modules/skin

tmcl-skin：皮肤与披风（M2-07 起）。

- `src/mojang.rs`：正版账号读取档案皮肤与披风、上传皮肤、切换或隐藏披风（`api.minecraftservices.com`）。
- `src/yggdrasil.rs`：外置登录账号的皮肤站页面地址（取自元数据 `meta.links.homepage`，否则为站点根）。
- `src/library.rs`：本地皮肤库 `<data_root>/skins/`（`<sha256>.png` + `index.json`）与 PNG 校验（64×64 或 64×32）。

功能模块只依赖 `core/*`，模块之间互不依赖，见 `docs/PLAN.md` 第 2 节。
