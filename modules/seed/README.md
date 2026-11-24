# tmcl-seed · 种子地图

M3-01 为 Java 版原版世界生成生物群系瓦片和结构位置。选型顺序与验收版本见[开发计划](../../docs/PLAN.md#m3-01-种子地图后端)。

## Chunkbase 集成核查

2026-11-24 核查时，Seed Map 提供可分享的地图链接，适合用系统浏览器打开。地图服务返回 `X-Frame-Options: deny`。在官方公开说明中未找到适用于启动器的离线 SDK、公开查询 API 或引擎再分发许可。使用条款第 2–3 节限制复制、修改、反编译和镜像。TMCL 的离线后端不采用 Chunkbase；用户可自行通过网站交叉核对结果。[Seed Map](https://www.chunkbase.com/apps/seed-map) · [使用条款](https://www.chunkbase.com/terms-of-use)

## Seed Atlas 验证范围

固定候选提交为 [43e46eb](https://github.com/DUzzL/Seed-Atlas/tree/43e46eb90244a7cac4654b4fdb8d4f27b3fb6346/seedatlas-engine)。比较默认主世界、数字种子 12345，参考点 (0, 64, 0)。结构起始区块、游戏命令返回的方块坐标、群系采样点应分别记录。
