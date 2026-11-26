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

### 扩大原版样本与精简构建验证（2026-11-25）

沿用上述默认主世界、(0,64,0) 参考点和版本。表内结构位置均为 **区块坐标**。1.12.2 用原版村庄布局生成器检查候选；1.18 从原版结构生成器得到有效起点；26.3 在新建的原版单人世界中读取有效结构起点及指定点的群系。

| 版本 | 种子 | 原点群系 | 村庄区块 | 废弃营地区块 | 斑驳森林样点 (X,Y,Z) |
| --- | ---: | --- | --- | --- | --- |
| 1.12.2 | 0 | forest | (−25,21) | — | — |
| 1.12.2 | −1 | deep_ocean | (10,−56) | — | — |
| 1.18 | 0 | river | (38,45) | — | — |
| 1.18 | −1 | deep_ocean | (58,−48) | — | — |
| 26.3 | 0 | river | (38,45) | (11,−11) | (2784,64,−1280) |
| 26.3 | −1 | deep_ocean | (58,−48) | (53,0) | (672,64,1984) |

[回归探针](validation/vanilla_samples.c) 固定了三个种子的 12 个群系样点和 12 个有效结构起点。它检查已知位置，不对最近搜索排序、其他结构、维度或自定义世界作保证。可在仓库根目录用 PowerShell 复跑（将路径改为固定版本的引擎源码目录，使用 Zig 0.14.1）：

~~~powershell
$engine = '外部 Seed Atlas 源码目录/seedatlas-engine'
$units = 'finders','generator','layers','biomenoise','biomes','noise','util','quadbase','strongholds','eyesearch' |
    ForEach-Object { Join-Path $engine "$_.c" }
$exe = Join-Path $env:TEMP 'tmcl-seed-samples.exe'
zig cc -O2 -fwrapv -ffp-contract=off -I $engine modules/seed/validation/vanilla_samples.c @units -o $exe
if ($LASTEXITCODE -eq 0) { & $exe }
~~~

**参数表核查。** 固定引擎用到的五张 `btree*.h` 与 [cubiomes 固定提交的 MIT 版本](https://github.com/Cubitect/cubiomes/tree/e61f90580cbdd883214a8054670dacae655e59c0/tables) 逐文件字节一致。应保留 Cubitect 的 MIT 版权声明，并区分 Seed Atlas 根项目的 GPLv3 条款与原有 MIT 文件。`terrain_splines.h` 明确标注来自原版数据，目前未取得单独的再分发依据。

**精简试验。** 临时源码树只包含 C 源码、头文件和上述五张群系表，剔除 `terrain_splines.h` 及其解释器。实验中将 `getTerrainFactor` 和 `getTerrainJaggedness` 替换为立即报错退出的哨兵，主动调用验证了退出码 86。完整与精简构建各通过全部 24 项回归。默认主世界、固定高度和上述结构的样本没有触发被裁剪的路径。建议原型先限定在这个范围内；在接入启动器前，应在调用边界拒绝不支持的世界类型和高度查询，避免测试哨兵进入发布包。大型生物群系、放大化世界、任意种子及其他结构的可用性仍需各自验证。

### 旧版结构取样（M3-01c1）

以种子 0 和 −1 的默认主世界为基准，调用本地原版 1.12.2 和 1.18 的结构生成器，核实要塞、沙漠神殿、丛林神庙和海底遗迹。每个世界核验近、远两个要塞及其余三类各一个起点（共 20 个）；要求起点有效、拼块非空且起始区块吻合。1.12.2 的两类神殿共享放置规则，按实际拼块区分。各世界另记录 128 个要塞的**放置**坐标，不能将其当作 512 次完整建筑生成验证。

在仓库根目录运行以下命令；使用 Python 3、JDK 17/18 和开发者自行提供的对应官方服务端 JAR。脚本只读取本地 JAR、核对指纹、在临时目录编译和运行探针，不启动服务器或下载文件。坐标单位为区块，不是方块。

~~~sh
python modules/seed/validation/legacy/check.py 1.12.2 <server-1.12.2.jar> <JDK目录>
python modules/seed/validation/legacy/check.py 1.18 <server-1.18.jar> <JDK目录>
~~~

[验证器](validation/legacy/check.py) 输出实际起点及拼块数，并将两组完整要塞环逐项与[坐标基准](validation/legacy/rings.json)比较。这个步骤只提供独立原版基准，不代表启动器已经接入地图后端。
