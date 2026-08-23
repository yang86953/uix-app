//! OpenGL ES 3.0 薄 RHI 的固定 shader 选择与编译辅助。

// 在 Rust 2024 下禁止 unsafe 函数体隐式扩大底层操作范围。
#![deny(unsafe_op_in_unsafe_fn)]

// 引入祖父 raster 模块的 OpenGL 错误辅助和 shader 源表。
use super::super::{gl_error, rhi_shaders};
// 引入 glow 的上下文扩展方法。
use glow::HasContext as _;

// 引入统一错误、结果和封闭 pipeline 语义。
use crate::core::error::{Errc, Error, Result};
use crate::platform::presentation::rhi::PipelineKind;

// 按封闭 pipeline 语义选择 OpenGL ES shader 对。
pub(super) fn shader_sources(kind: PipelineKind) -> (&'static str, &'static str) {
    // 把固定的通用 ABI 映射到具体 GLSL 源。
    match kind {
        // 位置 mesh 使用独立的 solid shader。
        PipelineKind::SolidMesh => (rhi_shaders::SOLID_VERTEX, rhi_shaders::SOLID_FRAGMENT),
        // 普通颜色纹理和 Additive 纹理共用 shader，blend 在 draw 状态选择。
        PipelineKind::TexturedQuad | PipelineKind::TexturedQuadAdditive => {
            (rhi_shaders::TEXTURED_VERTEX, rhi_shaders::TEXTURED_FRAGMENT)
        }
        // 线性和径向渐变使用矩形顶点与独立 fragment。
        PipelineKind::GradientRect => {
            (rhi_shaders::GRADIENT_VERTEX, rhi_shaders::GRADIENT_FRAGMENT)
        }
        // R8 coverage 复用 float8 顶点阶段。
        PipelineKind::GlyphCoverageQuad => {
            (rhi_shaders::TEXTURED_VERTEX, rhi_shaders::COVERAGE_FRAGMENT)
        }
        // 普通与 Additive 圆角/描边矩形共用矩形顶点和 shape fragment。
        PipelineKind::ShapeRect | PipelineKind::ShapeRectAdditive => {
            // Shape 顶点阶段只消费共享层提供的实际绘制边界。
            (rhi_shaders::SHAPE_VERTEX, rhi_shaders::SHAPE_FRAGMENT)
        }
        // 扇形使用独立单位矩形顶点阶段，并由 fragment 执行角度/半径裁剪。
        PipelineKind::Sector => (rhi_shaders::SECTOR_VERTEX, rhi_shaders::SECTOR_FRAGMENT),
        // 任意方向线段使用解析胶囊距离场，所有平台保持相同覆盖率语义。
        PipelineKind::LineSegment => (rhi_shaders::LINE_VERTEX, rhi_shaders::LINE_FRAGMENT),
        // 阴影使用扩张的单位 quad。
        PipelineKind::BoxShadow => (rhi_shaders::SHADOW_VERTEX, rhi_shaders::SHADOW_FRAGMENT),
        // blur 使用 NDC 区域顶点和 64 tap fragment。
        PipelineKind::BlurPass => (rhi_shaders::BLUR_VERTEX, rhi_shaders::BLUR_FRAGMENT),
        // MSDF 复用 float8 顶点阶段并使用 RGBA8 距离 fragment。
        PipelineKind::MsdfGlyphQuad => (rhi_shaders::TEXTURED_VERTEX, rhi_shaders::MSDF_FRAGMENT),
    }
}

// 编译并链接一个 GLES 3.0 shader program。
///
/// # Safety
/// 调用线程必须持有并 current `gl` 所属的有效 GLES 3 上下文，且在本函数返回前不得切换或销毁该上下文。
pub(super) unsafe fn compile_program(
    gl: &glow::Context,
    vertex_source: &str,
    fragment_source: &str,
    label: &str,
) -> Result<glow::Program> {
    // SAFETY：调用者保证当前线程绑定了用于创建资源的有效 GL 上下文。
    let vertex = unsafe { gl.create_shader(glow::VERTEX_SHADER) }
        .map_err(|error| gl_error("create RHI vertex shader", error))?;
    // SAFETY：vertex 由当前上下文创建且在本函数返回前保持存活。
    unsafe { gl.shader_source(vertex, vertex_source) };
    // SAFETY：vertex 仍由当前上下文拥有，源码已经设置。
    unsafe { gl.compile_shader(vertex) };
    // SAFETY：查询对象是当前上下文创建且尚未释放的 vertex shader。
    if !unsafe { gl.get_shader_compile_status(vertex) } {
        // SAFETY：读取日志不会改变仍存活的 vertex shader 所有权。
        let log = unsafe { gl.get_shader_info_log(vertex) };
        // SAFETY：失败分支只释放一次由当前上下文创建的 vertex shader。
        unsafe { gl.delete_shader(vertex) };
        return Err(Error::new(
            Errc::PlatformError,
            format!("OpenGL {label} vertex shader compile failed: {log}"),
        ));
    }
    // 编译 fragment shader。
    // SAFETY：当前 GL 上下文仍有效并拥有已经编译的 vertex shader。
    let fragment = match unsafe { gl.create_shader(glow::FRAGMENT_SHADER) } {
        Ok(shader) => shader,
        Err(error) => {
            // SAFETY：fragment 创建失败时只释放一次仍由当前上下文拥有的 vertex shader。
            unsafe { gl.delete_shader(vertex) };
            return Err(gl_error("create RHI fragment shader", error));
        }
    };
    // SAFETY：fragment 由当前上下文创建且在本函数返回前保持存活。
    unsafe { gl.shader_source(fragment, fragment_source) };
    // SAFETY：fragment 仍由当前上下文拥有，源码已经设置。
    unsafe { gl.compile_shader(fragment) };
    // SAFETY：查询对象是当前上下文创建且尚未释放的 fragment shader。
    if !unsafe { gl.get_shader_compile_status(fragment) } {
        // SAFETY：读取日志不会改变仍存活的 fragment shader 所有权。
        let log = unsafe { gl.get_shader_info_log(fragment) };
        // SAFETY：失败分支只释放一次仍由当前上下文拥有的 vertex shader。
        unsafe { gl.delete_shader(vertex) };
        // SAFETY：失败分支只释放一次仍由当前上下文拥有的 fragment shader。
        unsafe { gl.delete_shader(fragment) };
        return Err(Error::new(
            Errc::PlatformError,
            format!("OpenGL {label} fragment shader compile failed: {log}"),
        ));
    }
    // 创建并链接 program。
    // SAFETY：当前 GL 上下文有效，两个 shader 均已成功编译并保持存活。
    let program = match unsafe { gl.create_program() } {
        Ok(program) => program,
        Err(error) => {
            // SAFETY：program 创建失败时只释放一次仍存活的 vertex shader。
            unsafe { gl.delete_shader(vertex) };
            // SAFETY：program 创建失败时只释放一次仍存活的 fragment shader。
            unsafe { gl.delete_shader(fragment) };
            return Err(gl_error("create RHI program", error));
        }
    };
    // SAFETY：program 与 vertex 都由当前上下文创建且保持存活。
    unsafe { gl.attach_shader(program, vertex) };
    // SAFETY：program 与 fragment 都由当前上下文创建且保持存活。
    unsafe { gl.attach_shader(program, fragment) };
    // SAFETY：program 已绑定两份由当前上下文拥有的已编译 shader。
    unsafe { gl.link_program(program) };
    // SAFETY：link 已消费 shader 内容，vertex 原生对象只释放一次。
    unsafe { gl.delete_shader(vertex) };
    // SAFETY：link 已消费 shader 内容，fragment 原生对象只释放一次。
    unsafe { gl.delete_shader(fragment) };
    // SAFETY：查询对象是当前上下文创建且尚未释放的 program。
    if !unsafe { gl.get_program_link_status(program) } {
        // SAFETY：读取日志不会改变仍存活的 program 所有权。
        let log = unsafe { gl.get_program_info_log(program) };
        // SAFETY：失败分支只释放一次由当前上下文创建的 program。
        unsafe { gl.delete_program(program) };
        return Err(Error::new(
            Errc::PlatformError,
            format!("OpenGL {label} program link failed: {log}"),
        ));
    }
    // 返回已经链接的 program。
    Ok(program)
}
