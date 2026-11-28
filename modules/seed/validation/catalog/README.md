# 原版可定位结构目录（M3-01d1）

这是后续原版对照的**分母**，不是启动器能力表。范围只含默认普通原版世界的结构，不含群系、POI、模组或自定义数据包。三份 JSON 中，`dimensions[维度][定位目标]` 的值是该维度下的变体/对应名；`tags` 单独保存标签到最终成员的关系。维度之间相同的命令名只计一个去重目标，不能把所有维度列数或标签数直接相加。

| 版本 | 主世界 | 下界 | 末地 | 去重定位目标 | 当前有同粒度原版起点回归的目标¹ |
| --- | ---: | ---: | ---: | ---: | --- |
| [1.12.2](1.12.2.json) | 6 | 1 | 1 | 8 | 3/8：Village、Stronghold、Monument；Temple 仅验证两种子类，不计为完整目标 |
| [1.18](1.18.json) | 14 | 4 | 1 | 18 | 6/18：village、stronghold、desert_pyramid、jungle_pyramid、monument、mansion |
| [26.3](26.3.json) | 47 | 4 | 1 | 52 | 5/52：stronghold、desert_pyramid、jungle_pyramid、monument、mansion；另有两种**标签级**图层 |

¹ 这里统计与现有[有效起点回归](../../tests/backend.rs)可对应的命令目标，不保证任意种子或全部配置变体正确。林地府邸只在 1.18 与 26.3 开放：1.12.2 的府邸起点需要完整区块提供器，当前离线探针无法构造，保留为缺口而不是按未验证结果开放。三版下界、末地的产品查询能力仍为零。1.18 按“目标 × 维度”计是 **6/19** 而非 6/18，因为 ruined_portal 在两维度出现。

### 如何解释一行

- **1.12.2**：键保留原版大小写（例如 `/locate Temple`、`EndCity`）。值是本目录手工对照的语义名，**不是**该版注册 ID 或可单独输入的命令。Temple 包含沙漠神殿、丛林神庙、雪屋、沼泽小屋；现有图层只能选前两者。Mineshaft 的普通/恶地变体也不计作两条命令。
- **1.18**：键来自实际 Brigadier `locate` 子节点，值来自内置 configured structure feature 注册表，经默认维度生成器配置和可生成群系过滤；要塞单独检查原版环。例：`/locate endcity` 对应末地的 `minecraft:end_city`；下界的 `ruined_portal` 仅对应 `minecraft:ruined_portal_nether`。不要把配置名作为该版命令参数。
- **26.3**：`/locate structure` 的参数是 worldgen/structure 的 ID 或标签。5 种村庄、18 种废弃营地各有独立 ID；旧图层验证的是整组，不能据此宣称已能筛选 23 种变体。33 个标签（含用途标签）都不是额外建筑。结构集如 nether_complexes 是放置规则，不是新的命令目标。JSON 保留规范命名空间；默认 minecraft 命名空间的简写不另计。

### 离线复跑

在仓库根目录运行，输入是开发者自行提供的**官方服务端**，脚本先校验该版本指纹。Python 3.9+；旧版用 JDK 17/18，26.3 本次用 JDK 27。发布仓库不含 JAR、结构模板、群系参数、完整游戏报告或存档。

~~~sh
python modules/seed/validation/catalog/check.py 1.12.2 <server.jar> <JDK目录>
python modules/seed/validation/catalog/check.py 1.18 <server.jar> <JDK目录>
python modules/seed/validation/catalog/check.py 26.3 <server.jar> <JDK目录>
python -m unittest discover -s modules/seed/validation/catalog
cargo test --locked -p tmcl-seed --test locate_catalog
~~~

默认比较整个 JSON，任一成员、维度或目标变化即失败。传 `--output <新文件>` 可导出候选基准再审 diff。旧版探针运行在临时目录，不启动游戏服务：1.12.2 取命令补全，再以种子 0 调用三个默认维度生成器的定位入口；每个命令必须至少在一个维度实际返回位置。1.18 读取种子 1 普通预设的命令树和生成设置，不把配置存在误当成已生成某个起点。

26.3 用官方数据生成器的命令报告确认参数指向的注册表；从原版数据包联结结构注册、正权重放置条目和群系选择器。按普通预设使用主世界/下界的原版群系参数报告、末地的原版群系标签。另在真实普通单人世界的集成服务器线程运行 [Runtime26.sample](Runtime26.java)：解析定位命令、枚举活注册表和生成器的 possibleBiomes；核对全部 **52** 条维度归属及 **33** 个标签的 **66** 条成员关系。可在 26.3 复跑命令末尾追加 `--live-log <该次探针日志>`，逐项比较两种来源；仅在本地读取探针记录，不提交整份日志。该核查不宣称已逐座生成建筑。

### 缺口与实施顺序

剩余直接目标分别为 **5、13、48** 个（1.12.2 的 5 个含部分覆盖的 Temple）。下一批建议从三版主世界都存在、尚无产品图层的 **Mansion / mansion** 开始：先取原版有效起点和反例，再核实引擎的地形筛选误差。此后按放置算法拆批；下界/末地需单独验证生成器和坐标语义。未核验的项保留在分母中，不下发给 UI。
