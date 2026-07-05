//! # uix-graphics 稳定公开 API
//!
//! 按功能域划分的渲染契约。
//!
//! | 模块 | 职责 |
//! |------|------|
//! | [`primitives`] | 颜色、路径、描边、混合模式 |
//! | [`spatial`] | 物理单位、变换矩阵、脏区域 |
//! | [`canvas`] | Canvas2D 绘制上下文 |
//! | [`text`] | TextBackend 与字体文本类型 |
//! | [`backend`] | RenderingBackend、RenderBackend |
//! | [`engine`] | GraphicsEngine、UpdateStrategy |
//! | [`pipeline`] | RenderSession、无效化队列 |

pub mod backend;
pub mod canvas;
pub mod engine;
pub mod pipeline;
pub mod primitives;
pub mod spatial;
pub mod text;

pub use backend::*;
pub use canvas::Canvas2D;
pub use engine::*;
pub use pipeline::*;
pub use primitives::*;
pub use spatial::*;
pub use text::*;
