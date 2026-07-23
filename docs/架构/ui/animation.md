# 动画

[← 架构索引](../../架构.md)

> **接口**：声明 ui 系统中 **animation — 动画系统**模块的内部设计。所属系统：`ui`。依赖：[组件核心模块](core.md)（需了解 WidgetAnimation trait）。导出：动画公开用法 → [使用 · 动画](../../使用/动画.md)。

## 模块定位

animation 模块（`src/ui/animation/`）提供泛型逐帧动画能力。由主循环统一以 `dt` 推进，窗口不可见时自动暂停不消耗 CPU。分声明式（`Animated<T>`）和指令式（`Animation<T>`）两层。
