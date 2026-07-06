# 组件系统

> 单个 UI 组件的结构、能力与生命周期。

## 原则

改样式 = 改属性；颜色来自主题；业务回调在 HandlerTable 不在 struct；能力拆分为 Layout / Render / Event / Lifecycle。

struct 放：配置、交互态、StyleSet。不放：业务闭包、variant 枚举、on_click 字段。

## 能力

| Trait | 职责 |
|-------|------|
| Layout | measure → Size（#29） |
| Render | StyleSet + Theme 绘制 |
| Event | dispatch、命中测试、焦点 |
| Lifecycle | attach → mount → active/inactive → unmount…；ThemeChanged 时 palette-only invalidate |

Active/Inactive = 焦点 + 视口求交（#8、#19）。动画由 **AnimationRegistry** 驱动（#83）。

ComponentHandle：只读配置；可 invalidate/emit；不可改 style 或读交互态（#61、#72）。

Authoring：`component!` 宏（#20）；prelude 精选符号（#69）。

## Button v1

纯文本；StyleSet 预设，默认 `button_default`（#77）；无 icon/loading/Ripple（#23、#30）。Enter/Space → Click。

基础设施就绪后 Big Bang 全量重写内置组件与 demo（#58、#80）。

标脏：交互态由 dispatch；State 由框架；Theme 为 palette-only；动画由 Registry。
