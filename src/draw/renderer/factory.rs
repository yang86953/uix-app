//! Maps a native [`IGraphicsContext`] to the unique draw [`Renderer`].

use crate::core::{Error, Result};
use crate::draw::renderer::Renderer;
use crate::native::present::IGraphicsContext;

/// Creates the unique renderer for the context's orthogonal raster/present recipe.
///
/// Legal native recipes remain `GpuNative × Swapchain` and `Cpu × PixelUpload`;
/// both now select presentation state inside the same concrete [`Renderer`]. On
/// failure the context is shut down before the error is returned.
pub(crate) fn create_renderer(context: Box<dyn IGraphicsContext>) -> Result<Renderer, Error> {
    Renderer::from_context(context)
}
