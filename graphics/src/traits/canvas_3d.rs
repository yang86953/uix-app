//! 3D 渲染接口——保留模式，先留接口。
//!
//! 当前 SoftwareEngine 返回 NotSupported 错误。
//! 未来 GpuEngine 在 wgpu/OpenGL 后端正经实现。

use uix_platform::Error;

// ═══════════════════════════════════════════
// 占位类型（后续实现时填充）
// ═══════════════════════════════════════════

/// 缓冲句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferHandle(pub u32);

/// 着色器句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShaderHandle(pub u32);

/// 纹理句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureHandle(pub u32);

/// 顶点布局描述。
#[derive(Debug, Clone)]
pub struct VertexLayout;

/// 着色器源码。
#[derive(Debug, Clone)]
pub struct ShaderSource;

/// 纹理格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TexFormat {
    /// RGBA 每通道 8 位无符号整数。
    Rgba8Unorm,
}

/// 摄像机。
#[derive(Debug, Clone, Copy)]
pub struct Camera;

/// 光源。
#[derive(Debug, Clone, Copy)]
pub struct Light;

// ═══════════════════════════════════════════
// Canvas3D trait
// ═══════════════════════════════════════════

/// 3D 渲染上下文——保留模式 API。
pub trait Canvas3D {
    // 顶点/索引缓冲
    fn create_vertex_buffer(&mut self, data: &[u8], layout: &VertexLayout)
        -> Result<BufferHandle, Error>;
    fn update_vertex_buffer(&mut self, handle: BufferHandle, data: &[u8]);
    fn destroy_buffer(&mut self, handle: BufferHandle);

    // 着色器
    fn create_shader(&mut self, src: &ShaderSource) -> Result<ShaderHandle, Error>;
    fn destroy_shader(&mut self, handle: ShaderHandle);

    // 纹理
    fn create_texture_2d(
        &mut self,
        w: u32,
        h: u32,
        data: &[u8],
        fmt: TexFormat,
    ) -> Result<TextureHandle, Error>;
    fn destroy_texture(&mut self, handle: TextureHandle);

    // 渲染状态
    fn set_camera(&mut self, camera: &Camera);
    fn set_light(&mut self, index: u32, light: &Light);

    // 提交绘制
    fn draw_mesh(
        &mut self,
        vb: BufferHandle,
        ib: Option<BufferHandle>,
        shader: ShaderHandle,
        textures: &[TextureHandle],
    );
    fn clear_depth(&mut self);
}
