# 平台系统

> OS 差异隔离；traits 对外；Fail Fast。

聚合：窗口、事件循环、剪贴板、光标、呈现、定时器、文件/通知（非热路径，差异用 Result）。

**呈现**：CPU 像素缓冲 + damage present；GPU context + damage swap；PresentDamage 全屏或矩形列表。

**UiEvent** → app 边界转 SystemEvent。Windows / Linux ✅；macOS 未实现。FakePlatform 用于测试（#40）。

唯一允许 `#[cfg(windows/unix)]` 的源码区：`native/backends`、`native/factory.rs`（详见 AGENTS.md）。
