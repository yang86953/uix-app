# 主题与样式系统

← [Main](../Main.md) · 系统 **#4** · 功能域：`ui`（Theme）· `draw`（token 解析）

> Theme 提供颜色与排版；Style / StyleSet 描述外观。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| 两层色板 | [色板架构](#色板架构) | #1–#3 #11 #51 #52 |
| 运行时 Theme | [Theme 与 TokenProvider](#theme-与-tokenprovider) | #48 #57 |
| 颜色引用 | [ColorValue](#colorvalue) | #3 |
| 品牌 / 明暗 | [品牌与 Light/Dark](#品牌与-lightdark) | #9 #28 #74 #125 |
| 组件外观 | [Style · StyleSet](#style--styleset) | #14–#17 #37 #44 #56 #81 |

**关联**：[component](component.md) · [rendering](rendering.md) · [view-reactive](view-reactive.md) · [data](data.md) · [application](application.md) · [demand-driven](demand-driven.md)

---

## 色板架构

设计两层，运行时合一：

### 算法层（#1–#2、#11–#12）

| 概念 | 规格 |
|------|------|
| Chromatic | 12 主色 × 10 阶 = **120**；种子在第 **6** 阶 |
| 命名 | Ant Design 同款（Red … Magenta） |
| 衍生 | 移植 Ant Design 官方色板算法 |
| Neutral scale | **13** 阶白→黑，独立 scale |
| NeutralRole | **15** 语义角色 → 映射到 13 阶；Light/Dark 各一套（#52） |

API：`DesignTokens::from_primaries([Color; 12], is_dark)`、`with_brand_primary(Color)`（#57）。

### 语义层（运行时）

**DesignTokens** 暴露 Ant Design 5 语义字段（`color_primary`、`color_bg_container`、`color_text`…），供组件与 Style 直接引用。

```text
ThemePrimitives (种子 + 语义基色)
    → into_design_tokens(is_dark)     // RGB 插值公式
    → DesignTokens (~80 字段)
    → Theme(Arc<dyn TokenProvider>)
```

组件通过 `ColorValue::palette(PaletteColor::Primary)` 解析，**不直接引用 chromatic 阶数**。

### TypographyScale（#51）

~10 档字号/行高/字重；Style 存 **TypographyToken**，Theme 解析具体数值（#65 系统字体栈）。

---

## Theme 与 TokenProvider

### 类型层次

```text
draw::ThemeTokens          // IColor + ITypography + ISpacing + IBoxShadow + is_dark
    ↑
ui::TokenProvider          // + style_container / style_button_* 默认 Style
    ↑
DesignTokens / DynTokens
    ↑
Theme { Arc<dyn TokenProvider> }
```

### Theme API

| 方法 | 作用 |
|------|------|
| `Theme::antd_light()` / `antd_dark()` | 内置 DefaultTheme（#48） |
| `Theme::new(provider)` | 包装自定义 TokenProvider |
| `tokens() -> &dyn TokenProvider` | 绘制时注入 PaintContext |

### DynTokens（#74、#125）

- `RwLock<DesignTokens>` + 运行时切换 `set_light` / `set_dark` / `set_custom`
- **跟 OS**：App builder `.follow_system_theme(true)`（**opt-in**，默认 false）
  - 框架收 `ThemeChanged` UiEvent → 读 `IDisplay::is_dark_mode()` → 切换 tokens → 全窗 palette invalidate（#128）
  - **不**后台 poll；无 ThemeChanged 不 wake
- `.follow_system_theme(false)` 时：运行中仅 `.theme(...)` / App 显式切换生效

---

## ColorValue

| 变体 | 组件可用 | 随主题 | 说明 |
|------|----------|--------|------|
| `Palette(PaletteColor)` | ✅ | 是 | 语义色：Primary、Success、Error… |
| `Neutral(NeutralRole)` | ✅ | 是 | 15 角色 |
| `Custom(Color)` | 使用侧可 | 否 | #3 debug lint / clippy 警告 |

解析：`ColorValue::resolve(&dyn ThemeTokens)` → `draw::Color`。

---

## 品牌与 Light/Dark

| 场景 | API |
|------|-----|
| 默认品牌 | Primary::BLUE 种子（#28） |
| 换主色 | `with_brand_primary` |
| 全定制 | `from_primaries([12])` |
| 切换 invalidate | 仅 palette 引用组件（#9） |
| 默认模式 | 启动时可读 OS（#74）；运行中 **opt-in** `.follow_system_theme(true)` 跟 OS（#125），否则 App `.theme()` |
| Settings | `theme_mode` key → App 解析（见 [data](data.md)） |

Settings 持久化 `theme_mode` / `brand_primary` key → App 解析（见 [data](data.md)）。

---

## Style · StyleSet

### Style（纯数据，类 CSS）

**盒模型**：`margin`、`padding`、`border_width: EdgeInsets`（#56）、`border_color`、`border_radius`。

**尺寸**：`width`、`height`（optional）。

**Flex / Grid**（#81、#84）：

| 字段 | 用途 |
|------|------|
| `display`, `flex_direction`, `flex_wrap` | 容器模式 |
| `justify_content`, `align_items`, `gap` | Flex 对齐 |
| `grid_template_columns/rows`, `grid_gap` | Grid 轨道 |
| `flex_grow`, `flex_shrink`, `align_self` | 子项 |

**视觉**：`background`（ColorValue）、`color`、`font_size`（TypographyToken）、`opacity`、`box_shadow`（#44）、`visible`。

应用：`apply_style(ctx, style, state)` 写入 PaintContext；布局引擎读 `margin` 参与盒模型。

### StyleSet 五态（#14–#15）

```rust
pub struct StyleSet {
    pub normal: Style,
    pub hover: Option<Style>,
    pub pressed: Option<Style>,
    pub focused: Option<Style>,
    pub disabled: Option<Style>,
}
```

**resolve(state)** 优先级（#14）：

```text
disabled > focused > pressed > hover > normal
```

| 态 | 规则 |
|----|------|
| focused | 仅 `:focus-visible` 时机（#63） |
| disabled | opacity + 中性色（#37） |
| 差异继承 | 非 normal 态只存差异字段，合并 normal（#15） |

### 预设（#16、#39、#47、#77）

| 预设 | 特征 |
|------|------|
| `button_default()` | 默认按钮 |
| `button_primary()` | 主色填充 |
| `button_ghost()` | 透明底 + Primary 边框/文字（#47） |
| `button_danger()` | Primary::RED（#34） |

View DSL：`button("...").primary()` 等价于切换 StyleSet 预设。无 variant enum。

---

## 主题作用域（#33、#94）

- App **全局**一份 Theme 快照
- 多窗 **共享** Theme
- 浮层子树继承同一 Theme

切换 Theme 时组件 `on_theme_changed` → palette-only paint invalidate，不触发 layout。

---

## 源码模块

```text
ui/theme/              Theme, DesignTokens, TokenProvider, DynTokens
ui/foundation/style/   Style, StyleSet, ColorValue, apply_style
draw/painting/theme/   ThemeTokens（IColor / ITypography / ISpacing）
```

详见 [Main · 源码目录详表](../Main.md#源码目录详表)。
