# 主题与样式系统

> 系统职责：**Theme** 提供什么颜色与排版；**Style / StyleSet** 如何描述外观。

---

## 1. 色板

- **120** chromatic：12 主色 × 10 阶；种子在第 **6** 阶（#1）  
- **13** 阶中性色，独立 scale（#2）  
- 主色命名：Ant Design 同款 12 色（#11）  
- 衍生算法：Ant Design 官方（#12）  

### 衍生阶语义

| 阶 | 用途 |
|----|------|
| 1–2 | 最浅背景 |
| 3–5 | 浅边框、次要 |
| **6** | 种子 |
| 7–10 | 悬停深 → 最深 |

---

## 2. NeutralRole（#52）

15 个语义角色（Text/Border/Fill/Bg 系列 + TextInverse）。Light / Dark **各一套** 映射到 13 阶；组件**不**直接引用阶数。

---

## 3. TypographyScale（#51）

约 **10 档**字号与行高。Style 存 **TypographyToken**；绘制时 Theme 解析为字号、行高、字体族。默认字体：**系统字体栈**（#65）。

---

## 4. ColorValue 规则

| 来源 | 组件 | 使用侧 | 随主题 |
|------|------|--------|--------|
| 色板 | ✅ | ✅ | 是 |
| 中性角色 | ✅ | ✅ | 是 |
| 自定义 RGB | ❌ | ✅ | 否（debug lint，#3） |

---

## 5. 品牌主题（#57）

| 档位 | 行为 |
|------|------|
| 默认 | 只换 Blue（8 号）种子（#28） |
| 高级 | 12 种子全量自定义 |

内置 **DefaultTheme**（#48）；切换时 **palette-only invalidate**（#9）。

Light/Dark：默认跟 OS，可覆盖，运行中监听 OS（#74、#76）。

---

## 6. Style 属性

**盒模型**：margin、padding、宽高、display、flex 方向、align、justify、gap、grid 轨道、flex_grow  

**视觉**：background、前景色、**四边 border**（#56）、border 色、圆角、透明度、可选阴影（#44）  

**排版**：TypographyToken  

Flex/Grid 对齐与轨道在 **Style** 上（#81、#84）。

---

## 7. StyleSet

五层：normal + hover、pressed、**focused**、disabled。

| 规则 | 内容 |
|------|------|
| 优先级（#54） | disabled > focused > pressed > hover > normal |
| 继承（#15） | 差异字段 + 继承 normal |
| focused（#63） | 仅 focus-visible |
| disabled（#37） | opacity + 中性色 |

---

## 8. 预设

组件外观不使用 variant enum（#16）。StyleSet 预设：button_default、button_primary、button_ghost、button_danger 等（#39、#47、#34）。button() 默认 default（#77）。

内置组件外观由 `StyleSet::button_*()` 等预设驱动，不保留组件专属 variant / size 枚举。

---

## 9. 使用侧

可改 Palette / 中性；可自定义色（不随主题）；非颜色属性任意改。详见 [view-reactive.md](view-reactive.md) 入口约束（#21）。

---

## 10. 落地要求

| 项 | 要求 |
|----|------|
| Theme | Light/Dark + 品牌种子能力 |
| 颜色解析 | 统一通过 ColorValue 延迟解析 |
| 排版解析 | 统一通过 TypographyToken 解析 |
| 组件预设 | 内置组件外观统一收敛为 StyleSet 预设 |

---

## 11. 实现顺序

Theme + Style 为 #50 第一步。见 [decisions.md](../decisions.md)。

---

## 12. 相关决策

#1–#3、#9–#17、#27–#28、#34、#37、#39、#42、#47–#48、#51–#57、#63、#65、#74、#76、#81、#84 — [decisions.md](../decisions.md)
