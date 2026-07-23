# 主题与配置

[← 架构索引](../../架构.md)

> **接口**：声明 ui 系统中 **theme — 主题系统**及 **foundation/** 中 Locale/Config 提供者的内部设计。所属系统：`ui`。依赖：[设计令牌与色彩](../graphics/geometry.md)（令牌推导链）、[组件核心模块](core.md)。导出：主题配置用法 → [使用 · 框架能力](../../使用/框架能力.md)。

## 组件清单

### 模块：主题系统（`theme/`）

| 组件 | 类型 | 职责 |
|------|------|------|
| `DesignTokens` | struct | ~100 个运行时令牌（color_text, color_bg_container, border_radius 等） |
| `ThemePrimitives` | struct | 8 个基色种子（primary, success, warning, error, info, bg, text, border） |
| `TokenPatch` | struct | 组件级令牌覆写 |
| `Theme` | struct | 完整主题（含亮/暗模式） |
| `ConfigProvider` | struct | 组件配置上下文提供者 |
| `LocaleProvider` | struct | 本地化上下文提供者 |
| `Config` | struct | 组件配置结构 |
| `Locale` | struct | 本地化字符串集合 |

详细设计 → [`graphics/geometry.md`](../graphics/geometry.md)

## 组件：TokenPatch

TokenPatch 携带组件粒度的令牌覆写。在 reconciliation 阶段，UI 层自顶向下收集当前 subtree 的所有 TokenPatch，合并后应用到该组件的渲染中。

> `graphics` 层只消费已解析的 Color，`ui` 层的 TokenPatch 和 DesignTokens 推导逻辑不进入 `graphics` 层。

## 组件：Locale

Locale 是框架内置国际化方案。使用 `zh_cn` / `en_us` 两个内置 locale。
