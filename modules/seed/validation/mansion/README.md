# 林地府邸原版对照（M3-01d2）

只用来固定 `mansion` 在默认主世界的有效起点，防止把放置候选误报成建筑。产物不超过区块坐标，不包含 JAR、存档、结构模板或截图。

| 版本 | 种子 | 原版有效起点（区块） | 原版拒绝的候选（区块） | 来源 |
| --- | ---: | --- | --- | --- |
| 1.18 | 0 | (−221,−52) | (36,24) | 生成器与 `isViableStructurePos`/`isViableStructureTerrain` |
| 1.18 | −1 | (259,−208) | (9,21) | 同上 |
| 26.3 | 0 | (−221,−52) | 未单独取得 | 官方服务端 `/locate structure minecraft:mansion` |
| 26.3 | −1 | (259,−208) | 未单独取得 | 同上 |

1.18 的**反例**是同一放置规则下的候选区块，原版生成器判定起点无效或没有有效拼块；Rust 查询必须返回空。这样能覆盖“只有生物群系/地形合格才出现”的过滤，而不仅是正例。26.3 的服务端命令只给最近的有效起点，没有公开的反例枚举，故不虚构反例；对应回归只断言正例与明显无关区块为空。

1.12.2 未列入：1.12.2 的府邸起点构造需要完整的区块提供器，当前离线探针无法在不生成世界的情况下调用。产品查询对 1.12.2 保持返回 `SEED_INVALID_INPUT`，界面也不显示该图层；补齐需要改用真实客户端世界，留作后续子项。

## 复跑

输入为开发者本地提供的官方服务端 JAR；脚本先校验 SHA-1。

~~~sh
python modules/seed/validation/mansion/check.py <server-1.18.jar> <JDK目录>
python modules/seed/validation/mansion/probe26.py <server-26.3.jar> <JDK目录>
cargo test --locked -p tmcl-seed --test backend mansion
~~~

1.18 脚本在临时目录解包官方内嵌 JAR，用签名隔离的子类加载器运行探针；26.3 脚本在临时目录启动一次无玩家、离线模式的本地服务端，只发送 `locate` 与 `stop`，不向外部联网也不需要账号。两者都不会把游戏文件写入仓库。

Rust 回归把这些坐标硬编码在 `modules/seed/tests/backend.rs`，不需要 Java 或游戏文件即可在 CI 运行。
