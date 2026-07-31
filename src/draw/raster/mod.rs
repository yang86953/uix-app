//! 模式外基础算法 — CPU 像素光栅化（System 私有边界）。
//!
//! 按通用 SMC 定义，「普通函数、基础算法」不必强行归类为 System / Module /
//! Component。本目录是 graphics 系统内跨 Module 共享的纯像素算法与软渲染
//! 执行器：painting（FrameEncoder 的受控 CPU segment）、backend（软渲染
//! 后端）、resources（字形覆盖率）都只依赖这里的窄函数，不互相持有实现。
//!
//! 归属：graphics System 私有边界（`crate::draw` 根）唯一拥有；任何 Module
//! 都不拥有本目录的实例或生命周期。

pub(crate) mod pixel_surface;
pub(crate) mod rasterizer;
pub(crate) mod shared_rasterizer;
pub(crate) mod software_rasterizer;
pub(crate) mod software_rasterizer_fill;
pub(crate) mod software_rasterizer_stroke;
