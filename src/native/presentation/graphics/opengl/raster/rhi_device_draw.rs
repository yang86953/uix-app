//! OpenGL ES 3.0 薄 RHI 的固定 draw ABI 分派。

// 引入 glow 的上下文扩展方法。
use glow::HasContext as _;
// 引入统一结果和 draw packet 语义。
use crate::core::error::Result;
use crate::native::present::rhi::{pipeline_keys, BufferUsage, DrawPacket, TextureFormat};
// 复用父资源表、shader key 和错误辅助。
use super::{rhi_invalid, OpenGlRhiDevice};

// 为字节镜像读取一个 native-endian float。
fn read_f32(data: &[u8], index: usize) -> Result<f32> {
    // 将 float 索引转换为字节偏移并检查四字节范围。
    let offset = index
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| rhi_invalid("OpenGL RHI uniform index overflows"))?;
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| rhi_invalid("OpenGL RHI uniform payload is truncated"))?;
    // 通用 renderer 使用 native-endian float ABI。
    Ok(f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

// 按 float 索引读取 vec4 常量。
fn read_vec4(data: &[u8], index: usize) -> Result<[f32; 4]> {
    // 逐通道读取，保持边界错误带有统一 typed error。
    Ok([
        read_f32(data, index)?,
        read_f32(data, index + 1)?,
        read_f32(data, index + 2)?,
        read_f32(data, index + 3)?,
    ])
}

// 给固定 program 设置 vec2 uniform。
unsafe fn set_vec2(gl: &glow::Context, program: glow::Program, name: &str, value: [f32; 2]) {
    // 未被 shader 使用的 uniform 位置为 None，GL 会安全忽略。
    let location = gl.get_uniform_location(program, name);
    gl.uniform_2_f32(location.as_ref(), value[0], value[1]);
}

// 给固定 program 设置 vec4 uniform。
unsafe fn set_vec4(gl: &glow::Context, program: glow::Program, name: &str, value: [f32; 4]) {
    // 未被 shader 使用的 uniform 位置为 None，GL 会安全忽略。
    let location = gl.get_uniform_location(program, name);
    gl.uniform_4_f32(location.as_ref(), value[0], value[1], value[2], value[3]);
}

// 给固定 program 设置单个 float uniform。
unsafe fn set_f32(gl: &glow::Context, program: glow::Program, name: &str, value: f32) {
    // 未被 shader 使用的 uniform 位置为 None，GL 会安全忽略。
    let location = gl.get_uniform_location(program, name);
    gl.uniform_1_f32(location.as_ref(), value);
}

// 绑定当前 packet 的 sampled texture、sampler 与 texture unit。
unsafe fn bind_sampled(
    gl: &glow::Context,
    device: &OpenGlRhiDevice,
    program: glow::Program,
) -> Result<TextureFormat> {
    // draw 前必须已经由 FramePlan 发出 BindTexture。
    let texture_handle = device
        .bound_texture
        .ok_or_else(|| rhi_invalid("OpenGL RHI sampled draw has no texture"))?;
    // draw 前必须已经由 FramePlan 发出 sampler 绑定。
    let sampler_handle = device
        .bound_sampler
        .ok_or_else(|| rhi_invalid("OpenGL RHI sampled draw has no sampler"))?;
    // 解析 texture 和 sampler 的原生对象。
    let texture = device.texture(texture_handle)?;
    let sampler = device.sampler(sampler_handle)?;
    // 绑定 t0/s0，并告诉 shader 从 texture unit zero 读取。
    gl.active_texture(glow::TEXTURE0);
    gl.bind_texture(glow::TEXTURE_2D, Some(texture.native));
    gl.bind_sampler(0, Some(sampler.native));
    let location = gl.get_uniform_location(program, "u_tex");
    gl.uniform_1_i32(location.as_ref(), 0);
    // 返回格式给 pipeline ABI 门禁使用。
    Ok(texture.format)
}

// 在 OpenGL ES 中执行一个已经 lowering 的 draw packet。
impl OpenGlRhiDevice {
    // 执行固定 pipeline key 对应的 non-indexed 或 uint32 indexed draw。
    pub(crate) fn draw(&mut self, gl: &glow::Context, packet: DrawPacket) -> Result<()> {
        // draw 必须位于显式 render pass 内。
        if !self.pass_open {
            return Err(rhi_invalid("OpenGL RHI draw without active render pass"));
        }
        // 复制 pipeline 的 key 和 program，结束资源表借用。
        let (key, program) = {
            let pipeline = self.pipeline(packet.pipeline)?;
            (pipeline.key, pipeline.program)
        };
        // 解析 vertex buffer 并保留其 ABI 描述。
        let (vertex, vertex_stride, vertex_usage) = {
            let buffer = self.buffer(packet.vertex_buffer)?;
            (buffer.native, buffer.stride_bytes, buffer.usage)
        };
        // 顶点资源必须声明为 Vertex。
        if vertex_usage != BufferUsage::Vertex {
            return Err(rhi_invalid("OpenGL RHI draw vertex buffer has wrong usage"));
        }
        // 所有当前固定 pipeline 都使用 uniform buffer。
        let uniform_handle = packet
            .uniform_buffer
            .ok_or_else(|| rhi_invalid("OpenGL RHI draw uniform is missing"))?;
        // 复制 uniform CPU 镜像，避免后续 GL 状态调用持有资源表借用。
        let uniform = {
            let buffer = self.buffer(uniform_handle)?;
            if buffer.usage != BufferUsage::Uniform {
                return Err(rhi_invalid("OpenGL RHI draw uniform has wrong usage"));
            }
            buffer.data.clone()
        };
        // 当前 ABI 不支持非零 base vertex 的 GLES fallback。
        if packet.base_vertex != 0 {
            return Err(rhi_invalid("OpenGL RHI nonzero base vertex is unsupported"));
        }
        // 绑定共享 VAO、顶点 buffer 和固定属性布局。
        unsafe {
            gl.use_program(Some(program));
            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex));
        }
        // 按 pipeline key 检查 stride、uniform 大小并设置 uniforms/blend。
        let sampled_format = match key {
            // solid mesh 使用 float2 vertex 和 MeshConstants。
            pipeline_keys::SOLID_MESH => {
                if vertex_stride != 8 || uniform.len() != 32 || packet.vertex_count == 0 {
                    return Err(rhi_invalid("OpenGL RHI solid ABI is invalid"));
                }
                unsafe {
                    configure_float2_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [read_f32(&uniform, 0)?, read_f32(&uniform, 1)?],
                    );
                    set_vec4(gl, program, "u_color", read_vec4(&uniform, 4)?);
                    set_premultiplied_blend(gl);
                }
                None
            }
            // BGRA sampled quad 和 additive sampled quad 共用 float8 vertex ABI。
            pipeline_keys::TEXTURED_QUAD | pipeline_keys::TEXTURED_QUAD_ADDITIVE => {
                if vertex_stride != 32 || uniform.len() != 16 || packet.vertex_count == 0 {
                    return Err(rhi_invalid("OpenGL RHI textured ABI is invalid"));
                }
                let format = unsafe {
                    configure_float8_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [read_f32(&uniform, 0)?, read_f32(&uniform, 1)?],
                    );
                    let format = bind_sampled(gl, self, program)?;
                    if format != TextureFormat::Bgra8Unorm && format != TextureFormat::Rgba8Unorm {
                        return Err(rhi_invalid("OpenGL RHI textured source format is invalid"));
                    }
                    if key == pipeline_keys::TEXTURED_QUAD_ADDITIVE {
                        set_additive_blend(gl);
                    } else {
                        set_premultiplied_blend(gl);
                    }
                    format
                };
                Some(format)
            }
            // 线性/径向渐变使用单位 float2 quad 和 24-float affine constants。
            pipeline_keys::GRADIENT_RECT => {
                if vertex_stride != 8 || uniform.len() != 96 || packet.vertex_count == 0 {
                    return Err(rhi_invalid("OpenGL RHI gradient ABI is invalid"));
                }
                unsafe {
                    configure_float2_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [read_f32(&uniform, 0)?, read_f32(&uniform, 1)?],
                    );
                    set_vec4(gl, program, "u_quad_origin_edge_x", read_vec4(&uniform, 4)?);
                    set_vec4(gl, program, "u_quad_edge_y", read_vec4(&uniform, 8)?);
                    set_vec4(gl, program, "u_color_a", read_vec4(&uniform, 12)?);
                    set_vec4(gl, program, "u_color_b", read_vec4(&uniform, 16)?);
                    set_vec4(gl, program, "u_params", read_vec4(&uniform, 20)?);
                    set_premultiplied_blend(gl);
                }
                None
            }
            // R8 coverage 使用 float8 vertex、点采样和固定量化 shader。
            pipeline_keys::GLYPH_COVERAGE_QUAD => {
                if vertex_stride != 32 || uniform.len() != 16 || packet.vertex_count == 0 {
                    return Err(rhi_invalid("OpenGL RHI coverage ABI is invalid"));
                }
                let format = unsafe {
                    configure_float8_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [read_f32(&uniform, 0)?, read_f32(&uniform, 1)?],
                    );
                    let format = bind_sampled(gl, self, program)?;
                    if format != TextureFormat::R8Unorm {
                        return Err(rhi_invalid("OpenGL RHI coverage source is not R8"));
                    }
                    set_premultiplied_blend(gl);
                    format
                };
                Some(format)
            }
            // RGBA8 MSDF 使用 float8 vertex、线性 sampler 和 32-byte constants。
            pipeline_keys::MSDF_GLYPH_QUAD => {
                if vertex_stride != 32 || uniform.len() != 32 || packet.vertex_count == 0 {
                    return Err(rhi_invalid("OpenGL RHI MSDF ABI is invalid"));
                }
                let format = unsafe {
                    configure_float8_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [read_f32(&uniform, 0)?, read_f32(&uniform, 1)?],
                    );
                    set_vec2(
                        gl,
                        program,
                        "u_tex_size",
                        [read_f32(&uniform, 2)?, read_f32(&uniform, 3)?],
                    );
                    set_f32(gl, program, "u_range", read_f32(&uniform, 4)?);
                    let format = bind_sampled(gl, self, program)?;
                    if format != TextureFormat::Rgba8Unorm {
                        return Err(rhi_invalid("OpenGL RHI MSDF source is not RGBA8"));
                    }
                    set_premultiplied_blend(gl);
                    format
                };
                Some(format)
            }
            // 普通与 Additive 圆角/描边矩形使用同一 RectConstants ABI。
            pipeline_keys::SHAPE_RECT | pipeline_keys::SHAPE_RECT_ADDITIVE => {
                if vertex_stride != 8
                    || (uniform.len() != 80 && uniform.len() != 96)
                    || packet.vertex_count == 0
                {
                    return Err(rhi_invalid("OpenGL RHI shape ABI is invalid"));
                }
                unsafe {
                    configure_float2_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [read_f32(&uniform, 0)?, read_f32(&uniform, 1)?],
                    );
                    set_vec4(gl, program, "u_rect", read_vec4(&uniform, 4)?);
                    set_vec4(gl, program, "u_color", read_vec4(&uniform, 8)?);
                    set_vec4(gl, program, "u_radius", read_vec4(&uniform, 12)?);
                    set_f32(gl, program, "u_stroke", read_f32(&uniform, 16)?);
                    // Additive pipeline 只切换目标混合，shader 与常量保持不变。
                    if key == pipeline_keys::SHAPE_RECT_ADDITIVE {
                        set_additive_blend(gl);
                    } else {
                        set_premultiplied_blend(gl);
                    }
                }
                None
            }
            // 原生扇形使用 64 字节 SectorConstants 和矩形单位 quad。
            pipeline_keys::SECTOR => {
                if vertex_stride != 8
                    || uniform.len() != crate::native::present::rhi::SECTOR_UNIFORM_BYTES
                    || packet.vertex_count == 0
                {
                    return Err(rhi_invalid("OpenGL RHI sector ABI is invalid"));
                }
                unsafe {
                    configure_float2_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [read_f32(&uniform, 0)?, read_f32(&uniform, 1)?],
                    );
                    set_vec4(gl, program, "u_rect", read_vec4(&uniform, 4)?);
                    set_vec4(gl, program, "u_color", read_vec4(&uniform, 8)?);
                    set_vec4(gl, program, "u_angles", read_vec4(&uniform, 12)?);
                    set_premultiplied_blend(gl);
                }
                None
            }
            // 仿射阴影使用 96 字节 AffineShadowConstants 和 straight-alpha blend。
            pipeline_keys::BOX_SHADOW => {
                if vertex_stride != 8 || uniform.len() != 96 || packet.vertex_count == 0 {
                    return Err(rhi_invalid("OpenGL RHI shadow ABI is invalid"));
                }
                unsafe {
                    configure_float2_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [read_f32(&uniform, 0)?, read_f32(&uniform, 1)?],
                    );
                    set_vec4(gl, program, "u_rect", read_vec4(&uniform, 4)?);
                    set_vec4(gl, program, "u_color", read_vec4(&uniform, 8)?);
                    set_vec4(gl, program, "u_radius", read_vec4(&uniform, 12)?);
                    set_vec4(gl, program, "u_params", read_vec4(&uniform, 16)?);
                    set_vec4(gl, program, "u_size", read_vec4(&uniform, 20)?);
                    set_straight_alpha_blend(gl);
                }
                None
            }
            // blur 使用 NDC float2 区域和 76-float BlurConstants。
            pipeline_keys::BLUR_PASS => {
                if vertex_stride != 8 || uniform.len() != 304 || packet.vertex_count == 0 {
                    return Err(rhi_invalid("OpenGL RHI blur ABI is invalid"));
                }
                let format = unsafe {
                    configure_float2_attributes(gl, vertex_stride);
                    set_vec4(gl, program, "u_sizes", read_vec4(&uniform, 0)?);
                    set_vec4(gl, program, "u_region", read_vec4(&uniform, 4)?);
                    set_vec4(gl, program, "u_dir_taps", read_vec4(&uniform, 8)?);
                    let mut weights = [0.0f32; 64];
                    for (index, weight) in weights.iter_mut().enumerate() {
                        *weight = read_f32(&uniform, 12 + index)?;
                    }
                    let location = gl.get_uniform_location(program, "u_weights");
                    gl.uniform_4_f32_slice(location.as_ref(), &weights);
                    let format = bind_sampled(gl, self, program)?;
                    if format == TextureFormat::R8Unorm {
                        return Err(rhi_invalid("OpenGL RHI blur source is not a color texture"));
                    }
                    gl.disable(glow::BLEND);
                    format
                };
                Some(format)
            }
            // create_pipeline 已经拒绝未知 key；此分支保持匹配穷尽。
            _ => return Err(rhi_invalid("OpenGL RHI draw pipeline key is unknown")),
        };
        // 记录 sampled_format 只用于保持每类 ABI 的显式门禁。
        let _ = sampled_format;
        // Indexed draw 需要固定 uint32 index ABI，非 indexed draw 使用顶点范围。
        unsafe {
            if let Some(index_handle) = packet.index_buffer {
                let index = self.buffer(index_handle)?;
                if index.usage != BufferUsage::Index || index.stride_bytes != 4 {
                    return Err(rhi_invalid("OpenGL RHI index buffer ABI is not uint32"));
                }
                if packet.index_count == 0 {
                    return Err(rhi_invalid("OpenGL RHI indexed draw range is empty"));
                }
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(index.native));
                gl.draw_elements(
                    glow::TRIANGLES,
                    packet.index_count as i32,
                    glow::UNSIGNED_INT,
                    packet.first_index.saturating_mul(4) as i32,
                );
            } else {
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
                gl.draw_arrays(
                    glow::TRIANGLES,
                    packet.first_vertex as i32,
                    packet.vertex_count as i32,
                );
            }
            // 清理本次 sampled draw 的绑定，避免下一 packet 继承错误资源。
            gl.bind_sampler(0, None);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }
        // 返回统一成功结果。
        Ok(())
    }
}

// 配置 position float2 顶点属性。
unsafe fn configure_float2_attributes(gl: &glow::Context, stride: u32) {
    // 只启用位置属性，避免旧 float8 attribute 泄漏到 unit quad。
    gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride as i32, 0);
    gl.enable_vertex_attrib_array(0);
    gl.disable_vertex_attrib_array(1);
    gl.disable_vertex_attrib_array(2);
}

// 配置 position/uv/color float8 顶点属性。
unsafe fn configure_float8_attributes(gl: &glow::Context, stride: u32) {
    // 配置三个固定位置属性，布局与通用 renderer 完全一致。
    gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride as i32, 0);
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride as i32, 8);
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_pointer_f32(2, 4, glow::FLOAT, false, stride as i32, 16);
    gl.enable_vertex_attrib_array(2);
}

// 设置 premultiplied SrcOver blend。
unsafe fn set_premultiplied_blend(gl: &glow::Context) {
    // 颜色和 alpha 都使用 ONE / ONE_MINUS_SRC_ALPHA。
    gl.enable(glow::BLEND);
    gl.blend_func_separate(
        glow::ONE,
        glow::ONE_MINUS_SRC_ALPHA,
        glow::ONE,
        glow::ONE_MINUS_SRC_ALPHA,
    );
}

// 设置 sampled additive blend。
unsafe fn set_additive_blend(gl: &glow::Context) {
    // Additive 只由显式 pipeline key 选择。
    gl.enable(glow::BLEND);
    gl.blend_func_separate(glow::ONE, glow::ONE, glow::ONE, glow::ONE);
}

// 设置 straight-alpha SrcOver blend。
unsafe fn set_straight_alpha_blend(gl: &glow::Context) {
    // 阴影 shader 输出 straight RGB 和 coverage alpha。
    gl.enable(glow::BLEND);
    gl.blend_func_separate(
        glow::SRC_ALPHA,
        glow::ONE_MINUS_SRC_ALPHA,
        glow::ONE,
        glow::ONE_MINUS_SRC_ALPHA,
    );
}
