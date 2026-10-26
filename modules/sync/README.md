# modules/sync

tmcl-sync：设置与按键同步（PLAN.md M2-09）。

- `src/options.rs`：解析与写回 `options.txt`，未修改时逐字节不变。
- `src/keymap.rs`：1.13 以后的 `key.keyboard.*` / `key.mouse.*` 键名与 1.12.2 及以前 LWJGL2 键码互转；表在 `data/keymap-lwjgl2-glfw.json`，鼠标键按 `-100 + 按钮序号` 换算。
- `src/profile.rs`：类别（按键 / 视频 / 音频 / 语言 / 其他）与主配置 `<data_root>/sync/master.json`。
- `src/apply.rs`：按类别与目标版本应用，写入前备份为 `options.txt.tmcl-bak-<unix 毫秒>`（每个实例保留 10 份），可恢复最近一份。

功能模块只依赖 `core/*`，模块之间互不依赖，见 `docs/PLAN.md` 第 2 节。
