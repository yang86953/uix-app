//! NoopCanvas3D — Canvas3D 的空实现（返回 NotSupported）。
//!
//! 用于 SoftwareEngine，因为 CPU 渲染器不提供 3D 能力。

use uix_platform::Error;

use crate::traits::{
    BufferHandle, Camera, Canvas3D, Light, ShaderHandle, ShaderSource, TexFormat,
    TextureHandle, VertexLayout,
};

/// 返回 "3D 渲染未实现" 错误。
fn not_impl() -> Error {
    Error::not_implemented("Canvas3D")
}

/// Canvas3D 的空实现。所有方法返回 NotSupported。
pub struct NoopCanvas3D;

impl Canvas3D for NoopCanvas3D {
    fn create_vertex_buffer(
        &mut self, _data: &[u8], _layout: &VertexLayout,
    ) -> Result<BufferHandle, Error> {
        Err(not_impl())
    }

    fn update_vertex_buffer(&mut self, _handle: BufferHandle, _data: &[u8]) {}

    fn destroy_buffer(&mut self, _handle: BufferHandle) {}

    fn create_shader(&mut self, _src: &ShaderSource) -> Result<ShaderHandle, Error> {
        Err(not_impl())
    }

    fn destroy_shader(&mut self, _handle: ShaderHandle) {}

    fn create_texture_2d(
        &mut self, _w: u32, _h: u32, _data: &[u8], _fmt: TexFormat,
    ) -> Result<TextureHandle, Error> {
        Err(not_impl())
    }

    fn destroy_texture(&mut self, _handle: TextureHandle) {}

    fn set_camera(&mut self, _camera: &Camera) {}
    fn set_light(&mut self, _index: u32, _light: &Light) {}

    fn draw_mesh(
        &mut self, _vb: BufferHandle, _ib: Option<BufferHandle>,
        _shader: ShaderHandle, _textures: &[TextureHandle],
    ) {}

    fn clear_depth(&mut self) {}
}
