//! painting Module — 组件绘制入口、canonical 操作、DisplayList 与帧编码。
//!
//! SMC 边界：本 Module 是 graphics System 的私有实现，由 System 私有边界
//! （`crate::draw` 根）拥有。职责：显式录制 save/restore、clip、transform、
//! opacity、blend 与已解析主题值；选择 native op、Picture 或受控 CPU
//! segment，不能语义等价执行时返回 typed error。
//!
//! 能力端口：`PaintContext` 通过构造注入接收 `resources` 的字体/图片服务
//! 句柄（System 组装期注入，只持有、不创建/销毁/管理生命周期），录制管线
//! 不直接接触 backend 或 native graphics。
//!
//! 窄契约：本 Module 依赖 `geometry` 的绘制值；跨 Module 共享的
//! `RenderOutcome` / `RenderTarget` 归 System 私有边界，本 Module 不拥有。

pub mod canvas;
pub mod display_list;
pub mod encoder;
pub mod paint_context;
pub(crate) mod recorder;

pub use canvas::Canvas2D;
pub use display_list::{DisplayList, PaintOp, PaintPass};
pub use encoder::{
    EncodedFrameExecution, EncodedPictureExecution, FrameCommand, FrameEncoder, FrameEncoderError,
    FrameGlyphBlit, FrameGlyphOutline, FrameImage, FrameOpacity, FramePresenter, FrameRadius,
    FrameRasterOp, FrameRect, FrameSampledRect, FrameStrokeRect, FrameStrokeWidth, GpuFrameAudit,
    GpuFrameViolationKind, PresentOutcome, ReferenceFrame,
};
pub use paint_context::{resolve_font_size, PaintContext, PaintSurfaceConfig};
