//! platform `presentation` Module：原生 surface、图形 recipe、presenter 与提交。
//!
//! SMC 边界：本 Module 是 platform System 的私有实现，由 System 私有边界
//! （`crate::native` 根）拥有。职责：device/surface、resize、begin/end
//! frame、CPU/GPU 结果到原生窗口的最终 present；只拥有原生 surface 与
//! 提交能力，不拥有 UI 绘制命令、字体缓存或 WidgetTree。
//!
//! 契约与实现：`platform::presentation` 拥有中立窄合同，`graphics` 是具体
//! GPU/软渲染 Adapter，`presenter` 是提交 Adapter。本 Module 经
//! `windowing::window::PlatformWindow` 窄契约取窗口 surface，不依赖兄弟
//! Module 的实现。`factory` 与 `platform::composition_root` 负责内部登记与组装。

pub(crate) mod graphics;
pub(crate) mod presenter;
