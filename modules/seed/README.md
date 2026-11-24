# tmcl-seed · 种子地图

M3-01 为 Java 版原版世界生成生物群系瓦片和结构位置。选型顺序与验收版本见[开发计划](../../docs/PLAN.md#m3-01-种子地图后端)。

## Chunkbase 集成核查

2026-11-24 核查时，Seed Map 提供可分享的地图链接，适合用系统浏览器打开。地图服务返回 `X-Frame-Options: deny`。在官方公开说明中未找到适用于启动器的离线 SDK、公开查询 API 或引擎再分发许可。使用条款第 2–3 节限制复制、修改、反编译和镜像。TMCL 的离线后端不采用 Chunkbase；用户可自行通过网站交叉核对结果。[Seed Map](https://www.chunkbase.com/apps/seed-map) · [使用条款](https://www.chunkbase.com/terms-of-use)

## Seed Atlas 验证范围

固定候选提交为 [43e46eb](https://github.com/DUzzL/Seed-Atlas/tree/43e46eb90244a7cac4654b4fdb8d4f27b3fb6346/seedatlas-engine)。比较默认主世界、数字种子 12345，参考点 (0, 64, 0)。结构起始区块、游戏命令返回的方块坐标、群系采样点应分别记录。

### 固定版本的本地编译与预测

Windows x64 使用 Zig 0.14.1 的 C 编译器，按上游构建清单编译 10 个 C 单元，保留 `-fwrapv -ffp-contract=off`。生成器可在不加载 Qt 的情况下运行。使用 `setupGenerator`、`applySeed` 初始化默认主世界；通过 `getBiomeAt` 采样方块坐标。结构候选在放置区域 X/Z 各 −12 至 +12 范围内枚举，经 `isViableStructurePos` 和 `isViableStructureTerrain` 筛选后按距原点的水平距离排序。

| 引擎版本 | 原点群系 | 最近村庄的起始区块西北角 (X, Z) |
| --- | --- | --- |
| 1.12.2 | taiga_mountains | (−688, 80) |
| 1.18 系列 | ocean | (−144, −832) |
| 26.3 | ocean | (−144, −832) |

26.3 预测最近废弃营地起始区块为 (39, −28)，西北角为 (624, −448)。在 Y=64 以 32 方块间隔从内向外扫描方环，斑驳森林首个样点为 (608, 64, −736)。验证时应查询该点的实际群系；原版定位命令的采样顺序可能不同。引擎将 `MC_1_18` 定义为 `MC_1_18_2`，仍需用原版 1.18 独立对照。

### 组件范围

桌面项目采用 GPL-3.0，独立 C 引擎目录声明 MIT。编译需要五张群系查找表及 `terrain_splines.h`；后者标注由原版世界生成数据生成，随包分发的依据仍需核实。图标中含上游明确排除在代码许可之外的游戏纹理。此次仅在隔离目录测试，仓库没有引入第三方引擎、游戏文件或图标。[引擎许可](https://github.com/DUzzL/Seed-Atlas/blob/43e46eb90244a7cac4654b4fdb8d4f27b3fb6346/seedatlas-engine/LICENSE) · [参数表](https://github.com/DUzzL/Seed-Atlas/blob/43e46eb90244a7cac4654b4fdb8d4f27b3fb6346/seedatlas-engine/tables/terrain_splines.h) · [第三方声明](https://github.com/DUzzL/Seed-Atlas/blob/43e46eb90244a7cac4654b4fdb8d4f27b3fb6346/THIRD_PARTY_NOTICES.md)

固定提交的上游 tests_versions.c 在启用断言时通过。此结果属于引擎自测，原版对照另行记录。

### 与 Java 原版独立对照

以下结果使用同一数字种子 12345、默认主世界、参考点 (0,64,0)。区块起点按区块坐标乘以 16 换算，结构坐标列均为 (X,Z)。

| 原版 | 原版数据来源 | 群系与村庄对照 |
| --- | --- | --- |
| 1.12.2 | 用官方 JAR 的 BiomeProvider、MapGenVillage 及结构起点生成代码查询 | 原点为 minecraft:mutated_taiga（ID 133），对应引擎的 taiga_mountains。引擎村庄区块 (−43,5) 生成有效布局，共 26 个组件。原版定位另选 (760,−456)，所在区块 (47,−29) 也有有效村庄，共 25 个组件。 |
| 1.18 | 用官方 **1.18** JAR 的默认世界生成设置、生物群系管理器与结构生成器查询 | 原点为 minecraft:ocean。按区块起点距原点的水平距离筛选，区块 (−9,−52) 实际生成有效村庄起点，共 175 个组件，西北角 (−144,−832) 与引擎相同。 |
| 26.3 | 原版客户端新建默认单人世界，从内建服务器查询群系、执行 locate 并检查有效结构起点 | 原点为海洋，(608,64,−736) 为斑驳森林。废弃营地定位为 (624,−448)，区块 (39,−28) 有有效的 abandoned_camp_taiga。引擎村庄区块 (−9,−52) 与原版从原点定位选出的 (10,55) 均有有效的 village_plains。 |

**选型结论**：固定引擎通过以上单种子原版样本验证，适合作为离线后端候选。地图可以标注已验证的结构起始区块；若界面提供“最近结构”，须说明距离及位置口径，不能据此承诺复现游戏定位命令的结果。扩大种子、结构和平台覆盖，以及澄清生成参数表的分发依据，分别属于后续验证与引入工作。
