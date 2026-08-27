//! 渲染系统 — 2D 引擎、光栅化、字体与合成管线。
//!
//! # SMC 边界（SMC-03）
//!
//! 本模块是 graphics System 的公开边界与私有实现。目标 Module 与依赖
//! （按通用 SMC 定义：窄契约单向无环、能力端口由 System 组装期注入）：
//!
//! | Module | 职责 | 依赖 |
//! |--------|------|------|
//! | [`geometry`] | 绘制值（Color/Path/Transform）与空间类型 | 无（只依赖 core） |
//! | [`painting`] | PaintContext / DisplayList / FrameEncoder 录制管线 | geometry；resources 服务经能力端口注入 |
//! | [`scene`] | LayerTree / Picture / NodeId 场景与合成 | painting；resources 服务经能力端口注入 |
//! | [`renderer`] | Renderer / 帧编排 / 恢复 | scene、backend、resources、painting |
//! | [`backend`] | CPU/GPU 执行与能力协商 | painting（消费命令）、geometry、resources |
//! | [`resources`] | 字体、文本布局、图像与代际句柄 | geometry |
//!
//! System 私有边界（跨 Module 契约与模式外实现，任何 Module 不拥有）：
//!
//! | 归属 | 内容 |
//! |------|------|
//! | [`outcome`] | `RenderOutcome` / `GraphicsFailure`（painting 产出、renderer 消费） |
//! | [`target`] | `RenderTarget` / `UpdateStrategy` / `GraphicsCapabilities`（scene 栅格化目标协议与 renderer 帧策略） |
//! | [`raster`] | 模式外基础算法：CPU 像素光栅化器与软渲染执行器（painting/backend/resources 共用） |
//! | [`debug`] | 跨 painting/renderer 的调试绘制服务 |
//!
//! # 旧路径 compile-fail 测试
//!
//! ```compile_fail
//! // 旧平铺路径不得复活：Canvas2D 归 painting。
//! use uix::draw::api::Canvas2D;
//! ```
//!
//! ```compile_fail
//! // 旧平铺路径不得复活：FrameEncoder 归 painting。
//! use uix::draw::command::FrameEncoder;
//! ```
//!
//! ```compile_fail
//! // CPU 光栅化器已上移 System 边界，不得经 backend 路径访问。
//! use uix::draw::backend::cpu::rasterizer::fill::fill_scanline;
//! ```
//!
//! ```compile_fail
//! // NodeId 归 scene，renderer 路径不得复活。
//! use uix::draw::renderer::NodeId;
//! ```
//!
//! ```compile_fail
//! // 自动探测与 CPU fallback 属于框架内部策略，不是第二套公开图形选择入口。
//! use uix::draw::renderer::bootstrap::bootstrap_renderer;
//! ```

pub mod backend;
pub mod debug;
pub mod geometry;
pub mod outcome;
pub mod painting;
pub mod raster;
pub mod renderer;
pub mod resources;
pub mod scene;
pub mod target;

pub use crate::core::{DamageRegion, DirtyRegion};
pub use backend::{BackendCapabilities, BackendKind, RenderBackend};
// test-harness 与 Agent 截屏公开统一像素快照，不导出原生 RHI 类型。
#[cfg(any(feature = "test-harness", feature = "agent-control"))]
pub use backend::SurfaceReadback;
pub use geometry::color::{Color, colors};
pub use geometry::path::{FillRule, LineCap, LineJoin, Path, PathBuilder, PathSegment};
pub use geometry::spatial::unit::AngleExt;
pub use geometry::spatial::{
    AABB3D, IntoAABB3D, Mat4, Orientation, PhysicalBox, PhysicalUnit, PhysicalUnitExt, Quad2D,
    Ray3D, SpatialContext, Vec2, Vec3, Vec4,
};
pub use geometry::stroker::StrokeOptions;
pub use geometry::types::{
    BlendMode, FontHandle, GradientDirection, HAlign, ImageHandle, Radius, TextLayoutOptions,
    Transform, VAlign,
};
pub use geometry::{color, flattener, path, stroker, tessellator, types};
pub use painting::Canvas2D;
pub use renderer::Renderer;
// test-harness 与 Agent 截屏公开有界等待票据，与规范像素快照组成完整端口。
#[cfg(any(feature = "test-harness", feature = "agent-control"))]
pub use renderer::SurfaceReadbackTicket;
pub use renderer::{
    AnimationRegistry, Invalidation, InvalidationQueue, InvalidationSource, RenderMetrics,
    RenderOutcome, ScrollDelta,
};
pub use renderer::{
    GraphicsCapabilities, PresentationMode, RasterPipeline, RenderTarget, ScrollCopy,
    UpdateStrategy,
};
pub use resources::font::TextBackend;
pub use resources::{
    BitmapFont, BitmapHandle, FontBundle, FontService, GlyphRaster, ImageService, ImageSlot,
    LineInfo, LineMetrics, PositionedGlyph, TextLayout,
};
// 导出公开的场景节点身份与已解析 overlay effect 值。
pub use scene::{NodeId, OverlayBackdropEffect};
