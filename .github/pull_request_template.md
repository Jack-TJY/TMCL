<!--
标题格式：[步骤ID] 简短描述，例如 [M1-01] 下载引擎
契约 PR 标题以 contract: 开头并加 contract-change 标签（AGENTS.md §6.2）
各节都必须填写；不适用的写“无”。
-->

## 步骤

<!-- 步骤 ID 与 PLAN.md 链接，例如：M1-01 · docs/PLAN.md#m1-01-下载引擎 -->

- 步骤：
- 关联 Issue：<!-- Closes #… -->
- Agent：<!-- 例如 claude-a；人类贡献者可留空 -->

## 改动

<!-- 做了什么、为什么；列出改动的路径，确认都在该步骤的“文件”范围内 -->

## 契约变更

<!-- 无 / 列出新增或修改的命令、事件、错误码、配置字段。改名、删除、改类型、改语义必须走单独的契约 PR -->

无

## 验证

<!-- 跑了哪些命令；在哪些平台、哪些游戏版本上手测，结果如何。对照该步骤的“验收”逐条说明 -->

- [ ] `pnpm check`
- 平台 / 游戏版本：
- 验收逐条：

## 未完成与已知问题

无

## 交接

<!-- 按 AGENTS.md §12 填写，更新时覆盖旧内容并保留时间戳 -->

```
交接 · YYYY-MM-DD HH:MM · <agent 名>
状态：进行中 / 阻塞 / 待评审
已完成：
未完成：
下一步：
验证方法：
坑点：
阻塞原因：
```

## 自查清单（PLAN.md 第 4 节）

- [ ] `cargo fmt --check`、`cargo clippy --workspace -- -D warnings`、`cargo test --workspace` 通过
- [ ] 涉及前端时 `pnpm lint`、`pnpm typecheck`、`pnpm test` 通过
- [ ] 新增公开函数有文档注释；新增错误码已登记 `docs/contracts/errors.md` 并有中英文案
- [ ] 新增界面文字全部走 i18n，`pnpm i18n:check` 通过
- [ ] 联网测试标记为 `#[ignore]` 或放在 `online-tests` feature 下，联网逻辑至少有一个离线 fixture 测试
- [ ] 新增第三方依赖许可证与 GPL-3.0 兼容（`cargo deny check licenses`、`pnpm licenses:check`）
- [ ] 只改了步骤范围内的文件，没有密钥、调试输出、注释掉的大段代码
