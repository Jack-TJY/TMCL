# modules/resources

tmcl-resources：Modrinth / CurseForge 客户端、资源浏览、整合包（M2-01 起）。

- `src/modrinth/`：Modrinth API v2（M2-01）。
- `tests/fixtures/modrinth/`：录制的 Modrinth 真实响应，长文本与列表已截短。

功能模块只依赖 `core/*`，模块之间互不依赖，见 `docs/PLAN.md` 第 2 节。
