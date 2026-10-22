// 重新生成 ui/src/bindings/（ts-rs）：以更新模式运行 src-tauri 的绑定测试。
// 不带环境变量时同一测试只做比对，因此 `cargo test` / `pnpm check` 会在生成物过期时失败。
import { spawnSync } from "node:child_process";

const result = spawnSync(
  "cargo",
  ["test", "--package", "tmcl-app", "--lib", "--", "bindings::", "--quiet"],
  { stdio: "inherit", env: { ...process.env, TMCL_UPDATE_BINDINGS: "1" } },
);
process.exit(result.status ?? 1);
