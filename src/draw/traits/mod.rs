//! 渲染引擎 trait 体系。

pub mod backend;
pub mod canvas;
pub mod engine;
pub mod text;

pub use backend::{
    BackendCapabilities, BackendKind, DamageRegion, RenderBackend, RenderingBackend,
};
pub use canvas::Canvas2D;
pub use engine::{
    GraphicsCapabilities, GraphicsEngine, PresentationMode, RenderOutcome, UpdateStrategy,
};
pub use text::TextBackend;
