//! platform `presentation` Module：原生 surface、图形 recipe、presenter 与提交。
//!
//! SMC 边界：本 Module 是 platform System 的私有实现，由 System 私有边界
//! （`crate::native` 根）拥有。职责：device/surface、resize、begin/end
//! frame、CPU/GPU 结果到原生窗口的最终 present；只拥有原生 surface 与
//! 提交能力，不拥有 UI 绘制命令、字体缓存或 WidgetTree。
//!
//! 契约与实现：`present` 是本 Module 的窄能力契约（OS 提供者在 `backends`
//! 实现），`graphics` 是 GPU/软渲染后端，`presenter` 是提交适配。本 Module
//! 经 `windowing::window::PlatformWindow` 窄契约取窗口 surface，不依赖
//! 兄弟 Module 的实现。`factory`（组合根）与 `platform`（System 共享契约）
//! 位于 System 私有边界，不属于本 Module。

pub(crate) mod graphics;
pub(crate) mod presenter;
