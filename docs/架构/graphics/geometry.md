# 设计令牌与色彩

[← 架构索引](../../架构.md)

> **接口**：声明 graphics 系统中 **geometry — 绘制几何**模块（Color、Path）和 ui 系统中 **theme — 主题系统**模块（DesignTokens）之间的令牌推导链。所属系统：`graphics` + `ui` 交界。依赖：[系统列表](../系统列表.md)。导出：设计令牌用法 → [使用 · 样式与主题](../../使用/样式与主题.md)。

## 模块定位

色彩体系分两层：**选色色板**（graphics 系统，按需取用）和**运行时令牌**（ui 系统，主题驱动）。两者共享种子约定但不互相复制。

## 组件清单

### 模块：绘制几何中的色彩组件

| 组件 | 类型 | 所属系统 | 职责 |
|------|------|----------|------|
| `Color` | struct | `graphics` | RGBA 颜色；hex、rgba、from_rgb 构造 |
| `ColorScale` | struct | `graphics` | 单色 10 阶色板；0 最浅、9 最深、5 为主色 |
| `PrimaryHue` | enum | `graphics` | 12 个主色相（Red 到 Magenta），Blue 为品牌主色 |
| `NEUTRAL_PALETTE` | const | `graphics` | 白到黑 13 阶中性色 |
| `FunctionalColorRole` | enum | `graphics` | 功能色角色：Success / Warning / Error / Info |
| `DATA_VISUALIZATION_PALETTE` | const | `graphics` | AntV 10 色分类数据色板 |

### 模块：主题系统（ui 侧）

| 组件 | 类型 | 所属系统 | 职责 |
|------|------|----------|------|
| `ThemePrimitives` | struct | `ui` | 8 个基色种子（primary, success, warning, error, info, bg, text, border） |
| `DesignTokens` | struct | `ui` | ~100 个运行时令牌；桥接 ThemePrimitives 到组件 TokenPatch |
| `TokenPatch` | struct | `ui` | 组件级令牌覆写；用于组件自定义覆盖 |
| `ShadowToken` | struct | `ui` | 阴影令牌 |
| `Theme` | struct | `ui` | 完整主题（含亮/暗模式） |

### 模块：绘制几何（`geometry/`）

| 组件 | 类型 | 职责 |
|------|------|------|
| `Color` | struct | 已在上方组件清单列出 |
| `Path` | struct | 矢量路径；支持线段、贝塞尔曲线、弧线；与 Color 共同构成绘制几何基础 |

## 组件：ColorScale → ThemePrimitives → DesignTokens 推导链

```
ColorScale（graphics 层）
  → PrimaryHue.NEUTRAL_PALETTE.FunctionalColorRole
  → ThemePrimitives（ui 层，每一色相插值出 10 阶）
  → DesignTokens（ui 层，~100 个语义令牌）
  → TokenPatch（ui 层，组件级覆写）
```

**分层不变量**：`graphics` 层只消费已解析的 `Color`，不染指 `ThemePrimitives` / `DesignTokens` 的推导逻辑。`ColorScale` 和色板在 `graphics` 层，主题推导在 `ui` 层。
