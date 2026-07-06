# 主题与样式系统

> Theme 提供颜色与排版；Style / StyleSet 描述外观。

## 色板

120 chromatic（12×10，种子第 6 阶，#1）+ 13 阶中性 scale（#2）。Ant Design 同款命名与衍生算法（#11–#12）。

**NeutralRole**（#52）：15 语义角色，Light/Dark 各映射 13 阶；组件不直接引用阶数。

**TypographyScale**（#51）：~10 档；Style 存 TypographyToken，Theme 解析字号/行高/字体（#65 系统栈）。

## ColorValue

| 来源 | 组件可用 | 随主题 |
|------|----------|--------|
| 色板 / 中性角色 | ✅ | 是 |
| 自定义 RGB | ❌（使用侧可，#3 lint） | 否 |

## 品牌与 Light/Dark

默认只换 Blue 种子（#28）；高级 12 种子自定义（#57）。DefaultTheme（#48）；切换 palette-only invalidate（#9）。Light/Dark 默认跟 OS，可覆盖并运行中监听（#74、#76）。

## Style · StyleSet

盒模型 + flex/grid 对齐轨道（#81、#84）+ 四边 border（#56）+ 可选 shadow（#44）+ TypographyToken。

StyleSet 五态：normal / hover / pressed / focused / disabled。优先级 disabled > focused > pressed > hover > normal（#14）；差异继承 normal（#15）；focused 仅 focus-visible（#63）；disabled 用 opacity + 中性色（#37）。

预设 `StyleSet::button_*()`，无 variant enum（#16、#39、#47、#34、#77）。使用侧可改 Palette/中性/非色属性；入口约束见 view-reactive（#21）。
