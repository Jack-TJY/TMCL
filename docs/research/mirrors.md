# 下载镜像实测（M1-02）

测试日期：2026-10-23 · 测试人：claude-mirror · 程序：`core/net/examples/bench_sources.rs`

本报告供维护者决定 `auto` 策略的默认镜像。决定前 `auto` 等同于只用官方源（`core/net/src/source.rs` 的 `DEFAULT_MIRROR = None`）。

## 1. 方法

- 文件组（共 125 个，期望 SHA1 全部取自官方元数据）：
  - 版本清单 `version_manifest_v2.json`
  - 1.20.1 版本 JSON
  - 1.20.1 `client.jar`（约 22 MiB）
  - 1.20.1 的前 20 个依赖库
  - 1.20.1 资源索引中按哈希排序的前 100 个资源文件
  - Forge 1.20.1-47.2.0 安装器与 Fabric Loader 0.15.11
- 每个源跑 3 轮。资源文件 8 并发，其他文件组也用同一个并发上限。下载结果只在内存中算 SHA1，不写盘。
- 单个请求的时限：等响应头 60 s，读响应体 300 s。重定向自动跟随，表中“实际服务主机”是最终提供内容的主机。
- 吞吐 = 成功字节数 ÷ 该文件组的总耗时，所以小文件组的数值主要反映延迟。
- 网络：同一台 Windows 10 机器。国内直连时出口在中国大陆；海外测试通过本机代理，出口在美国。只测了一条国内线路（单一运营商），结论仅供参考。
- 复现：

  ```
  cargo run -p tmcl-net --example bench_sources --release -- 3 国内直连
  ```

  代理由 `HTTP(S)_PROXY` 环境变量决定。去掉这些变量即为直连。

## 2. 候选源

| 名称 | 地址 | 说明 |
| --- | --- | --- |
| `official` | Mojang / Forge / Fabric 官方 | 基准 |
| `bmclapi` | `bmclapi2.bangbang93.com` | 已内置。主站 302 跳转到 OpenBMCLAPI 众包节点、高校镜像或官方源 |
| `ustc` | `mirrors.ustc.edu.cn/bmclapi` | 中科大对 BMCLAPI maven 部分的缓存，只覆盖 maven 路径。未内置，仅在测速程序中定义 |
| MCBBS | `download.mcbbs.net` | 已失效（连接失败），未纳入测试 |

## 3. 结果

### 3.1 国内直连

| 源 | 文件组 | 成功率 | SHA1 一致 | 吞吐 (MiB/s) | 首字节中位数 (ms) | 失败原因 |
| --- | --- | --- | --- | --- | --- | --- |
| official | 版本清单 | 3/3 | 3/3 | 0.44 | 134 | - |
| official | 版本 JSON | 3/3 | 3/3 | 0.26 | 122 | - |
| official | client.jar | 3/3 | 3/3 | 7.79 | 513 | - |
| official | 依赖库 | 60/60 | 60/60 | 9.79 | 134 | - |
| official | 资源文件 | 300/300 | 300/300 | 2.64 | 113 | - |
| official | Forge/Fabric | **0/6** | 0/6 | - | - | 超时 ×1，读取中断 ×5 |
| bmclapi | 版本清单 | **1/3** | 1/3 | 0.00 | 1570 | 超时 ×1，读取中断 ×1 |
| bmclapi | 版本 JSON | 2/3 | 2/3 | 0.00 | 3722 | 读取中断 ×1 |
| bmclapi | client.jar | 3/3 | 3/3 | 6.47 | 629 | -（被跳转回官方源） |
| bmclapi | 依赖库 | 45/60 | 45/60 | 0.23 | 344 | 超时 ×6，读取中断 ×9 |
| bmclapi | 资源文件 | **215/300** | 215/300 | 0.02 | 815 | 超时 ×68，读取中断 ×17 |
| bmclapi | Forge/Fabric | 6/6 | 6/6 | 2.64 | 648 | -（跳转到中科大镜像） |
| ustc | 依赖库 | 39/60 | 39/60 | 26.06 | 166 | HTTP 404 ×21 |
| ustc | Forge/Fabric | 6/6 | 6/6 | **28.14** | 69 | - |

### 3.2 海外（美国出口）

| 源 | 文件组 | 成功率 | SHA1 一致 | 吞吐 (MiB/s) | 首字节中位数 (ms) | 失败原因 |
| --- | --- | --- | --- | --- | --- | --- |
| official | 版本清单 | 3/3 | 3/3 | 0.38 | 345 | - |
| official | 版本 JSON | 3/3 | 3/3 | 0.17 | 190 | - |
| official | client.jar | 3/3 | 3/3 | 4.01 | 347 | - |
| official | 依赖库 | 60/60 | 60/60 | 4.02 | 697 | - |
| official | 资源文件 | 300/300 | 300/300 | 1.39 | 198 | - |
| official | Forge/Fabric | 6/6 | 6/6 | 3.41 | 376 | - |
| bmclapi | 版本清单 | 3/3 | 3/3 | 0.07 | 1927 | - |
| bmclapi | 版本 JSON | 3/3 | 3/3 | 0.02 | 1359 | - |
| bmclapi | client.jar | 3/3 | 3/3 | 3.99 | 1377 | -（被跳转回官方源） |
| bmclapi | 依赖库 | 52/60 | 52/60 | 0.16 | 2785 | 连接失败 ×5，读取中断 ×3 |
| bmclapi | 资源文件 | 276/300 | 276/300 | 0.28 | 838 | 连接失败 ×24 |
| bmclapi | Forge/Fabric | 4/6 | 4/6 | 0.11 | 3539 | 请求错误 ×1，读取中断 ×1 |
| ustc | 依赖库 | 39/60 | 39/60 | 4.96 | 360 | HTTP 404 ×21 |
| ustc | Forge/Fabric | 6/6 | 6/6 | 4.89 | 333 | - |

### 3.3 观察

1. **SHA1 一致率为 100%**：所有源只要下载成功，内容都与官方一致。失败全是超时、断流、连接失败或 404，没有出现内容错误。
2. **国内直连官方源**：Mojang 的内容（清单、版本 JSON、client.jar、依赖库、资源文件）全部成功，速度最快。原因是 `piston-data.mojang.com`、`libraries.minecraft.net` 等主机走 Azure 前端，在国内有节点。但 **Forge 与 Fabric 的官方 maven 在国内直连全部失败**。
3. **BMCLAPI**：在两种网络下都慢于官方源，且对小文件不稳定。国内资源文件成功率约 72%，海外约 92%。根源是它把请求跳转到质量参差的 OpenBMCLAPI 众包节点。client.jar 会被跳转回官方源。它的 maven 请求在国内会跳到中科大镜像，全部成功。
4. **中科大缓存**：Forge/Fabric 极快（国内 28 MiB/s），但它是按需缓存，Mojang 依赖库有 35% 返回 404，不能单独作为完整镜像。地址 `mirrors.ustc.edu.cn/bmclapi` 也不是 BMCLAPI 文档公开的入口。
5. 测试中发现，下载引擎没有单个请求的总时限。一次请求在 TLS 握手或读取阶段挂起了 10 分钟以上。测速程序已自行加了时限，引擎本身的问题记在 PR 的“已知问题”中。

## 4. 条款与 User-Agent

- **BMCLAPI**（摘自 <https://bmclapidoc.bangbang93.com> 的“协议”一节）：
  - 文件版权归源站点所有。
  - 使用 BMCLAPI 的程序必须在下载界面或其他可见位置标明来源。
  - 禁止在 BMCLAPI 上二次封装其他协议。
  - 文档没有对 User-Agent 和速率提出具体要求。
  - 文档列出的映射与 `PrefixMirror::bmclapi()` 一致。NeoForge 只镜像 `net/neoforged/forge` 与 `net/neoforged/neoforge`。Quilt 被标为“上游 API 存在 bug，暂时不可用”，未收录。
- **中科大镜像站**：本次未核对其针对 BMCLAPI 缓存的使用说明。若采用，需先确认。
- TMCL 所有请求都带 `User-Agent: TMCL/<版本>`（`core/net/src/client.rs`），可被各源识别。

## 5. 供维护者选择

| 选项 | 内容 | 后果 |
| --- | --- | --- |
| A. 维持现状 | `auto` = 只用官方源 | 原版下载在国内外都最快最稳；国内直连无法安装 Forge/Fabric |
| **B. 加载器走 BMCLAPI（推荐）** | `auto`：Mojang 内容走官方源；Forge、NeoForge、Fabric 的 maven 与 meta 先走 BMCLAPI，失败再回退官方源 | 国内能装加载器（实测 6/6）；海外 BMCLAPI 失败时自动回退官方源，只损失时间；需要在下载界面标明 BMCLAPI 来源 |
| C. 整体走 BMCLAPI | `auto` = `bmclapi` 后回退官方 | 不推荐：两种网络下原版内容都更慢，大量小文件要等超时后才回退 |

选 B 的实现量很小：内置一个只含加载器规则的镜像（例如名为 `bmclapi-loaders`），把 `DEFAULT_MIRROR` 设为它即可。用户仍可在设置里把 `download.source` 设为 `bmclapi` 或 `official`。
