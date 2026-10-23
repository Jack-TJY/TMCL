# 主页组件契约

首页（`ui/src/pages/home/`）由组件（widget）组成，用户可在编辑模式下添加、删除、拖动、缩放组件（PLAN.md M1-18）。本文件规定组件的注册方式、尺寸、配置、数据来源与布局文件格式。

## 1. 目录与登记

| 路径 | 内容 |
| --- | --- |
| `ui/src/widgets/types.ts` | `WidgetDefinition`、`WidgetProps`、`defineWidget` 等类型 |
| `ui/src/widgets/registry.ts` | 组件登记表，**一行登记一个组件，按组件 ID 字母排序**（AGENTS.md §5.2 共享文件） |
| `ui/src/widgets/WidgetFrame.vue` | 组件外框：标题栏、编辑模式下的设置与删除按钮 |
| `ui/src/widgets/<id>/` | 一个组件一个目录，入口为 `index.ts`，默认导出 `defineWidget(...)` 的结果 |
| `ui/src/locales/<lang>/widget-<id>.json` | 组件自己的文案，key 以 `widget-<id>.` 开头 |

新增一个组件只需要：新建 `ui/src/widgets/<id>/` 目录、两种语言各一个 `widget-<id>.json`、在 `registry.ts` 中按字母顺序加一行。不需要改首页或其他组件。

## 2. 组件定义

```ts
export default defineWidget({
  id: "start-game",                       // 小写字母、数字、-；与目录名一致，发布后不得改名
  titleKey: "widget-start-game.title",    // 标题（外框与组件库中显示）
  descriptionKey: "widget-start-game.description", // 组件库中的一句话说明
  icon: Play24Regular,                    // @vicons/fluent 的 24 px 图标
  defaultSize: { w: 6, h: 4 },
  minSize: { w: 4, h: 3 },
  maxSize: { w: 12, h: 6 },               // 可省略，省略时不限制
  settingsSchema: [ /* 见第 4 节，可省略 */ ],
  component: () => import("./StartGame.vue"),
});
```

- `component` 必须是异步加载函数，首页只加载布局中出现的组件。
- 同一种组件可以添加多个实例，每个实例有自己的 `uid` 和设置。

## 3. 尺寸

- 栅格 12 列，行高固定；`w`、`h` 以格数计。尺寸须满足 `1 ≤ minSize ≤ defaultSize ≤ maxSize`，`w ≤ 12`。
- 非编辑模式下布局锁定；编辑模式下可拖动、拉伸，拉伸受 `minSize` / `maxSize` 约束，组件自动吸附对齐并避让。
- 窗口变窄时列宽等比缩小，组件不会重叠。组件内容须在 `minSize` 下（窗口 960×600 时约 60 px 一列）不溢出：长文字截断，列表可滚动。

## 4. 组件设置

`settingsSchema` 描述设置项，由外框统一生成设置面板，组件本身不画设置界面：

| `type` | 其他字段 | 值的类型 |
| --- | --- | --- |
| `boolean` | `default` | `boolean` |
| `number` | `default`、`min`、`max`、`step?` | `number` |
| `select` | `default`、`options: { value, labelKey }[]` | `string` |
| `instance` | `default: null` | 实例 ID `string`，`null` 表示“最近游玩的实例” |

每项都有 `key` 与 `labelKey`。组件收到的 `settings` 已按 schema 补齐默认值、丢弃未知 key、修正越界值，因此组件可以直接使用。

## 5. 组件收到的 props

```ts
interface WidgetProps {
  uid: string;                          // 该实例的 ID
  settings: Record<string, unknown>;    // 已规整的设置
  size: { w: number; h: number };       // 当前尺寸，可据此切换紧凑显示
  editing: boolean;                     // 是否处于编辑模式；编辑模式下组件不应响应点击
}
```

组件不能修改布局或自己的设置；也不能直接访问网络、文件系统或 `@tauri-apps/api`，数据只能通过 `ui/src/api/` 的封装或已有的 Pinia store（如任务中心 `stores/tasks.ts`、游戏会话 `components/game-log/sessions.ts`）获取。

## 6. 外观

- 外框是 Win11 风格的卡片：圆角 8 px、`background: var(--tmcl-card)`、`1px solid var(--tmcl-stroke)`，并带 class `tmcl-card`。
- 首页本身不画背景；背景、卡片透明度与毛玻璃由 M1-19 通过覆盖 `--tmcl-card` 或 `.tmcl-card` 样式实现，组件内部不要写死不透明的底色。
- 文字颜色使用 `--tmcl-text` / `--tmcl-text-secondary`，强调色使用 `--tmcl-accent`。

## 7. 布局文件

布局存于 `<data_root>/ui/layout.json`，经 `layout_get` / `layout_save` 读写（见 [ipc.md](ipc.md)），界面不直接写文件。

```json
{
  "schema": 1,
  "widgets": [
    { "uid": "k3j9x2", "type": "start-game", "x": 0, "y": 0, "w": 8, "h": 4, "settings": { "instance": null } }
  ]
}
```

| 字段 | 规则 |
| --- | --- |
| `uid` | 1–64 个字母、数字、`-`、`_`，布局内唯一 |
| `type` | 组件 ID，1–64 个小写字母、数字、`-` |
| `x`、`y` | 左上角的列与行，从 0 开始 |
| `w`、`h` | ≥ 1；`x + w ≤ 12`，`y + h ≤ 256` |
| `settings` | 对象，结构由组件定义；可省略 |

读取规则：

1. 文件不存在：使用默认布局（`ui/src/widgets/defaults.ts`）。
2. 整个文件无法使用（不是 JSON、`schema` 不是 1、结构不对）：后端备份为 `layout.json.corrupt-<unix 毫秒>`，界面使用默认布局并提示。
3. 单个组件字段缺失、越界或 `uid` 重复：后端跳过该项，保留其他组件。
4. `type` 未在 `registry.ts` 登记（例如由更新版本写入）：界面不显示该项，但保存时原样写回，不会丢失。
5. 尺寸超出组件当前的 `minSize` / `maxSize`：界面按约束修正。

`schema` 结构变化时提升版本并在后端写迁移。
