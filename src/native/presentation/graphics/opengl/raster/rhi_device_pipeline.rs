//! OpenGL ES 3.0 薄 RHI 的固定 shader 选择与编译辅助。

// 引入祖父 raster 模块的 OpenGL 错误辅助和 shader 源表。
use super::super::{gl_error, rhi_shaders};
// 引入 glow 的上下文扩展方法。
use glow::HasContext as _;

// 引入统一错误、结果和 pipeline key。
use crate::core::error::{Errc, Error, Result};
use crate::native::present::rhi::pipeline_keys;

// 按通用 pipeline key 选择 OpenGL ES shader 对。
pub(super) fn shader_sources(key: u64) -> Option<(&'static str, &'static str)> {
    // 把固定的通用 ABI 映射到具体 GLSL 源。
    match key {
        // 位置 mesh 使用独立的 solid shader。
        pipeline_keys::SOLID_MESH => Some((rhi_shaders::SOLID_VERTEX, rhi_shaders::SOLID_FRAGMENT)),
        // 普通颜色纹理和 Additive 纹理共用 shader，blend 在 draw 状态选择。
        pipeline_keys::TEXTURED_QUAD | pipeline_keys::TEXTURED_QUAD_ADDITIVE => {
            Some((rhi_shaders::TEXTURED_VERTEX, rhi_shaders::TEXTURED_FRAGMENT))
        }
        // 线性和径向渐变使用矩形顶点与独立 fragment。
        pipeline_keys::GRADIENT_RECT => {
            Some((rhi_shaders::GRADIENT_VERTEX, rhi_shaders::GRADIENT_FRAGMENT))
        }
        // R8 coverage 复用 float8 顶点阶段。
        pipeline_keys::GLYPH_COVERAGE_QUAD => {
            Some((rhi_shaders::TEXTURED_VERTEX, rhi_shaders::COVERAGE_FRAGMENT))
        }
        // 普通与 Additive 圆角/描边矩形共用矩形顶点和 shape fragment。
        pipeline_keys::SHAPE_RECT | pipeline_keys::SHAPE_RECT_ADDITIVE => {
            Some((rhi_shaders::RECT_VERTEX, rhi_shaders::SHAPE_FRAGMENT))
        }
        // 扇形复用单位矩形顶点阶段，并由独立 fragment 执行角度/半径裁剪。
        pipeline_keys::SECTOR => Some((rhi_shaders::RECT_VERTEX, rhi_shaders::SECTOR_FRAGMENT)),
        // 阴影使用扩张的单位 quad。
        pipeline_keys::BOX_SHADOW => {
            Some((rhi_shaders::SHADOW_VERTEX, rhi_shaders::SHADOW_FRAGMENT))
        }
        // blur 使用 NDC 区域顶点和 64 tap fragment。
        pipeline_keys::BLUR_PASS => Some((rhi_shaders::BLUR_VERTEX, rhi_shaders::BLUR_FRAGMENT)),
        // MSDF 复用 float8 顶点阶段并使用 RGBA8 距离 fragment。
        pipeline_keys::MSDF_GLYPH_QUAD => {
            Some((rhi_shaders::TEXTURED_VERTEX, rhi_shaders::MSDF_FRAGMENT))
        }
        // 未登记的 key 不进入 OpenGL shader 编译。
        _ => None,
    }
}

// 编译并链接一个 GLES 3.0 shader program。
pub(super) unsafe fn compile_program(
    gl: &glow::Context,
    vertex_source: &str,
    fragment_source: &str,
    label: &str,
) -> Result<glow::Program> {
    // 编译 vertex shader。
    let vertex = gl
        .create_shader(glow::VERTEX_SHADER)
        .map_err(|error| gl_error("create RHI vertex shader", error))?;
    gl.shader_source(vertex, vertex_source);
    gl.compile_shader(vertex);
    if !gl.get_shader_compile_status(vertex) {
        let log = gl.get_shader_info_log(vertex);
        gl.delete_shader(vertex);
        return Err(Error::new(
            Errc::PlatformError,
            format!("OpenGL {label} vertex shader compile failed: {log}"),
        ));
    }
    // 编译 fragment shader。
    let fragment = match gl.create_shader(glow::FRAGMENT_SHADER) {
        Ok(shader) => shader,
        Err(error) => {
            gl.delete_shader(vertex);
            return Err(gl_error("create RHI fragment shader", error));
        }
    };
    gl.shader_source(fragment, fragment_source);
    gl.compile_shader(fragment);
    if !gl.get_shader_compile_status(fragment) {
        let log = gl.get_shader_info_log(fragment);
        gl.delete_shader(vertex);
        gl.delete_shader(fragment);
        return Err(Error::new(
            Errc::PlatformError,
            format!("OpenGL {label} fragment shader compile failed: {log}"),
        ));
    }
    // 创建并链接 program。
    let program = match gl.create_program() {
        Ok(program) => program,
        Err(error) => {
            gl.delete_shader(vertex);
            gl.delete_shader(fragment);
            return Err(gl_error("create RHI program", error));
        }
    };
    gl.attach_shader(program, vertex);
    gl.attach_shader(program, fragment);
    gl.link_program(program);
    gl.delete_shader(vertex);
    gl.delete_shader(fragment);
    if !gl.get_program_link_status(program) {
        let log = gl.get_program_info_log(program);
        gl.delete_program(program);
        return Err(Error::new(
            Errc::PlatformError,
            format!("OpenGL {label} program link failed: {log}"),
        ));
    }
    // 返回已经链接的 program。
    Ok(program)
}
