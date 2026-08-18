//! OpenGL ES 3.0 薄 RHI 的固定 draw ABI 分派。

// 在 Rust 2024 下禁止 unsafe 函数体隐式扩大底层操作范围。
#![deny(unsafe_op_in_unsafe_fn)]

// 引入 glow 的上下文扩展方法。
use glow::HasContext as _;
// 引入统一结果和 draw packet 语义。
use crate::core::error::Result;
use crate::native::present::rhi::{
    BLUR_DIRECTION_TAPS_FLOAT_OFFSET, BLUR_REGION_FLOAT_OFFSET, BLUR_SIZES_FLOAT_OFFSET,
    BLUR_WEIGHT_COUNT, BLUR_WEIGHTS_FLOAT_OFFSET, BufferUsage, DrawPacket,
    GRADIENT_COLOR_A_FLOAT_OFFSET, GRADIENT_COLOR_B_FLOAT_OFFSET, GRADIENT_EDGE_Y_FLOAT_OFFSET,
    GRADIENT_ORIGIN_EDGE_X_FLOAT_OFFSET, GRADIENT_PARAMS_FLOAT_OFFSET,
    GRADIENT_VIEWPORT_FLOAT_OFFSET, MESH_COLOR_FLOAT_OFFSET, MESH_VIEWPORT_FLOAT_OFFSET,
    MSDF_RANGE_FLOAT_OFFSET, MSDF_TEXTURE_SIZE_FLOAT_OFFSET, MSDF_VIEWPORT_FLOAT_OFFSET,
    PipelineBlend, PipelineBlendFactor, PipelineBlendOperation, PipelineColorWriteMask,
    PipelineCullMode, PipelineDepthClip, PipelineDepthState, PipelineDepthStencilState,
    PipelineFrontFace, PipelineKind, PipelineMultisampleState, PipelinePrimitiveTopology,
    PipelineRasterState, PipelineStencilState, SAMPLED_VIEWPORT_FLOAT_OFFSET,
    SECTOR_ANGLES_FLOAT_OFFSET, SECTOR_COLOR_FLOAT_OFFSET, SECTOR_RECT_FLOAT_OFFSET,
    SECTOR_VIEWPORT_FLOAT_OFFSET, SHADOW_BODY_SIZE_AMBIENT_FLOAT_OFFSET, SHADOW_COLOR_FLOAT_OFFSET,
    SHADOW_EDGE_Y_BLUR_FLOAT_OFFSET, SHADOW_ORIGIN_EDGE_X_FLOAT_OFFSET, SHADOW_RADIUS_FLOAT_OFFSET,
    SHADOW_VIEWPORT_FLOAT_OFFSET, SHAPE_COLOR_FLOAT_OFFSET, SHAPE_DRAW_RECT_FLOAT_OFFSET,
    SHAPE_RADIUS_FLOAT_OFFSET, SHAPE_RECT_FLOAT_OFFSET, SHAPE_STROKE_FLOAT_OFFSET,
    SHAPE_VIEWPORT_FLOAT_OFFSET, SamplerDesc, TextureFormat,
};
// 复用父资源表、目标方向、类型化 shader 语义和错误辅助。
use super::{OpenGlRhiDevice, rhi_invalid, target_y_sign};

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

/// 给固定 program 设置 vec2 uniform。
///
/// # Safety
/// 调用者必须保证当前线程绑定的是创建该 program 的同一有效 GL 上下文，且 program 在该调用期间存活。
unsafe fn set_vec2(gl: &glow::Context, program: glow::Program, name: &str, value: [f32; 2]) {
    // SAFETY：调用者保证当前线程绑定了创建该 program 的有效 GL 上下文。
    let location = unsafe { gl.get_uniform_location(program, name) };
    // SAFETY：位置来自同一 program；None 会由 GL 作为未使用 uniform 安全忽略。
    unsafe { gl.uniform_2_f32(location.as_ref(), value[0], value[1]) };
}

/// 给固定 program 设置 vec4 uniform。
///
/// # Safety
/// 调用者必须保证当前线程绑定的是创建该 program 的同一有效 GL 上下文，且 program 在该调用期间存活。
unsafe fn set_vec4(gl: &glow::Context, program: glow::Program, name: &str, value: [f32; 4]) {
    // SAFETY：调用者保证当前线程绑定了创建该 program 的有效 GL 上下文。
    let location = unsafe { gl.get_uniform_location(program, name) };
    // SAFETY：位置来自同一 program；None 会由 GL 作为未使用 uniform 安全忽略。
    unsafe { gl.uniform_4_f32(location.as_ref(), value[0], value[1], value[2], value[3]) };
}

/// 给固定 program 设置单个 float uniform。
///
/// # Safety
/// 调用者必须保证当前线程绑定的是创建该 program 的同一有效 GL 上下文，且 program 在该调用期间存活。
unsafe fn set_f32(gl: &glow::Context, program: glow::Program, name: &str, value: f32) {
    // SAFETY：调用者保证当前线程绑定了创建该 program 的有效 GL 上下文。
    let location = unsafe { gl.get_uniform_location(program, name) };
    // SAFETY：位置来自同一 program；None 会由 GL 作为未使用 uniform 安全忽略。
    unsafe { gl.uniform_1_f32(location.as_ref(), value) };
}

/// 绑定当前 packet 的 sampled texture、sampler 与 texture unit。
///
/// # Safety
/// 调用者必须保证当前 owner thread 的 GL context current，且 program、texture、sampler 均由该设备在同一上下文创建并保持存活。
unsafe fn bind_sampled(
    gl: &glow::Context,
    device: &OpenGlRhiDevice,
    program: glow::Program,
) -> Result<(TextureFormat, SamplerDesc)> {
    // draw 前必须已经由 FramePlan 发出 BindTexture。
    let texture_handle = device
        .pass
        // 从共享 pass 状态读取 sampled texture 身份。
        .bound_texture()
        .ok_or_else(|| rhi_invalid("OpenGL RHI sampled draw has no texture"))?;
    // draw 前必须已经由 FramePlan 发出 sampler 绑定。
    let sampler_handle = device
        .pass
        // 从共享 pass 状态读取 sampler 身份。
        .bound_sampler()
        .ok_or_else(|| rhi_invalid("OpenGL RHI sampled draw has no sampler"))?;
    // 解析 texture 和 sampler 的原生对象。
    let texture = device.texture(texture_handle)?;
    let sampler = device.sampler(sampler_handle)?;
    // SAFETY：调用者保证当前 owner thread 已绑定有效 GL 上下文。
    unsafe { gl.active_texture(glow::TEXTURE0) };
    // SAFETY：纹理句柄来自当前设备资源表，且仍由该设备拥有。
    unsafe { gl.bind_texture(glow::TEXTURE_2D, Some(texture.native)) };
    // SAFETY：sampler 句柄来自当前设备资源表，且仍由该设备拥有。
    unsafe { gl.bind_sampler(0, Some(sampler.native)) };
    // SAFETY：program 由当前设备在同一 GL 上下文中创建并保持存活。
    let location = unsafe { gl.get_uniform_location(program, "u_tex") };
    // SAFETY：位置来自同一 program；None 会由 GL 安全忽略。
    unsafe { gl.uniform_1_i32(location.as_ref(), 0) };
    // 返回格式与创建时的 sampler 描述给共享 pipeline 门禁共同验证。
    Ok((texture.format, sampler.desc))
}

// 在 OpenGL ES 中执行一个已经 lowering 的 draw packet。
impl OpenGlRhiDevice {
    // 执行固定 pipeline 语义对应的 non-indexed 或 uint32 indexed draw。
    pub(crate) fn draw(&mut self, gl: &glow::Context, packet: DrawPacket) -> Result<()> {
        // draw 必须位于显式 render pass 内。
        self.pass.require_open()?;
        // draw 的 Y 方向只能来自当前 pass 已冻结的 render target 身份。
        let active_target = self.pass.target()?;
        // texture target 保持 top-left 存储，原生 surface 映射到窗口顶部。
        let y_sign = target_y_sign(active_target);
        // 读取 FramePlan 绑定的共享 pipeline 语义。
        let bound_kind = packet.pipeline.kind();
        // 复制 Adapter 资源表中的实际语义和 program，结束资源表借用。
        let (kind, program) = {
            // 原生资源表只按不透明句柄定位 pipeline。
            let pipeline = self.pipeline(packet.pipeline.handle())?;
            (pipeline.kind, pipeline.program)
        };
        // 句柄资源与 FramePlan 语义必须仍是创建时的同一绑定。
        if kind != bound_kind {
            // 拒绝可能让不同 Adapter 解释不同 shader ABI 的错配。
            return Err(rhi_invalid("OpenGL RHI pipeline binding kind is stale"));
        }
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
        // 从唯一共享契约读取当前 pipeline 的顶点、uniform、采样与混合语义。
        let contract = packet.pipeline.contract();
        // Drawing 生产端与 OpenGL 消费端必须严格使用同一个 ABI。
        if vertex_stride != contract.vertex.stride_bytes()
            || uniform.len() != contract.uniform.size_bytes()
            || packet.vertex_count == 0
        {
            // 使用统一门禁拒绝任何 pipeline 的漂移载荷。
            return Err(rhi_invalid("OpenGL RHI pipeline ABI is invalid"));
        }
        // 绑定共享 VAO、顶点 buffer 和固定属性布局。
        // SAFETY: program、self.vao 与 vertex 均由本设备在当前上下文创建且存活；context 保持 current。
        unsafe {
            gl.use_program(Some(program));
            // SAFETY：program 已绑定；缺少该 uniform 的 blur shader 会按 GL 规范安全忽略。
            set_f32(gl, program, "u_target_y_sign", y_sign);
            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex));
        }
        // 按封闭 pipeline 语义检查 stride、uniform 大小并设置 uniforms/blend。
        let sampled_format = match kind {
            // solid mesh 使用 float2 vertex 和 MeshConstants。
            PipelineKind::SolidMesh => {
                // SAFETY: 该分支已校验 stride=8、uniform=32 字节且 vertex_count>0；program/vao/vertex 存活；uniform 解码由 read_f32 的边界检查兜底；context 保持 current。
                unsafe {
                    configure_float2_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [
                            // 读取共享 Mesh viewport.width 字段。
                            read_f32(&uniform, MESH_VIEWPORT_FLOAT_OFFSET)?,
                            // 读取共享 Mesh viewport.height 字段。
                            read_f32(&uniform, MESH_VIEWPORT_FLOAT_OFFSET + 1)?,
                        ],
                    );
                    // 按共享字段索引映射 straight-alpha Mesh 颜色。
                    set_vec4(
                        gl,
                        program,
                        "u_color",
                        read_vec4(&uniform, MESH_COLOR_FLOAT_OFFSET)?,
                    );
                }
                None
            }
            // BGRA sampled quad 和 additive sampled quad 共用 float8 vertex ABI。
            PipelineKind::TexturedQuad | PipelineKind::TexturedQuadAdditive => {
                // SAFETY: 该分支已校验 stride=32、uniform=16 字节且 vertex_count>0；program/vao/vertex/texture/sampler 均存活；源格式由随后的门禁检查兜底；context 保持 current。
                let format = unsafe {
                    configure_float8_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [
                            // 读取 sampled viewport.width 字段。
                            read_f32(&uniform, SAMPLED_VIEWPORT_FLOAT_OFFSET)?,
                            // 读取 sampled viewport.height 字段。
                            read_f32(&uniform, SAMPLED_VIEWPORT_FLOAT_OFFSET + 1)?,
                        ],
                    );
                    // 同时取得纹理格式与创建时冻结的 sampler 描述。
                    let (format, sampler) = bind_sampled(gl, self, program)?;
                    // 共享采样契约必须同时接受颜色格式与线性过滤。
                    if !contract.sampling.accepts(format, sampler) {
                        return Err(rhi_invalid("OpenGL RHI textured source format is invalid"));
                    }
                    format
                };
                Some(format)
            }
            // 线性/径向渐变使用单位 float2 quad 和 24-float affine constants。
            PipelineKind::GradientRect => {
                // SAFETY: 该分支已校验 stride=8、uniform=96 字节且 vertex_count>0；program/vao/vertex 存活；uniform 解码有边界检查；context 保持 current。
                unsafe {
                    configure_float2_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [
                            // 读取共享 viewport.width 字段。
                            read_f32(&uniform, GRADIENT_VIEWPORT_FLOAT_OFFSET)?,
                            // 读取共享 viewport.height 字段。
                            read_f32(&uniform, GRADIENT_VIEWPORT_FLOAT_OFFSET + 1)?,
                        ],
                    );
                    // 按共享字段索引映射仿射原点与 X 边。
                    set_vec4(
                        gl,
                        program,
                        "u_quad_origin_edge_x",
                        read_vec4(&uniform, GRADIENT_ORIGIN_EDGE_X_FLOAT_OFFSET)?,
                    );
                    // 按共享字段索引映射仿射 Y 边。
                    set_vec4(
                        gl,
                        program,
                        "u_quad_edge_y",
                        read_vec4(&uniform, GRADIENT_EDGE_Y_FLOAT_OFFSET)?,
                    );
                    // 按共享字段索引映射起始 straight-alpha 颜色。
                    set_vec4(
                        gl,
                        program,
                        "u_color_a",
                        read_vec4(&uniform, GRADIENT_COLOR_A_FLOAT_OFFSET)?,
                    );
                    // 按共享字段索引映射结束 straight-alpha 颜色。
                    set_vec4(
                        gl,
                        program,
                        "u_color_b",
                        read_vec4(&uniform, GRADIENT_COLOR_B_FLOAT_OFFSET)?,
                    );
                    // 按共享字段索引映射模式、方向或半径参数。
                    set_vec4(
                        gl,
                        program,
                        "u_params",
                        read_vec4(&uniform, GRADIENT_PARAMS_FLOAT_OFFSET)?,
                    );
                }
                None
            }
            // R8 coverage 使用 float8 vertex、点采样和固定量化 shader。
            PipelineKind::GlyphCoverageQuad => {
                // SAFETY: 该分支已校验 stride=32、uniform=16 字节且 vertex_count>0；program/vao/vertex/texture/sampler 均存活；R8 源格式由随后的门禁检查兜底；context 保持 current。
                let format = unsafe {
                    configure_float8_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [
                            // 读取 coverage 共用的 viewport.width 字段。
                            read_f32(&uniform, SAMPLED_VIEWPORT_FLOAT_OFFSET)?,
                            // 读取 coverage 共用的 viewport.height 字段。
                            read_f32(&uniform, SAMPLED_VIEWPORT_FLOAT_OFFSET + 1)?,
                        ],
                    );
                    // 同时取得 coverage 格式与创建时冻结的 sampler 描述。
                    let (format, sampler) = bind_sampled(gl, self, program)?;
                    // 共享采样契约必须同时接受 R8 与最近点过滤。
                    if !contract.sampling.accepts(format, sampler) {
                        return Err(rhi_invalid("OpenGL RHI coverage source is not R8"));
                    }
                    format
                };
                Some(format)
            }
            // RGBA8 MSDF 使用 float8 vertex、线性 sampler 和 32-byte constants。
            PipelineKind::MsdfGlyphQuad => {
                // SAFETY: 该分支已校验 stride=32、uniform=32 字节且 vertex_count>0；program/vao/vertex/texture/sampler 均存活；RGBA8 源格式由随后的门禁检查兜底；context 保持 current。
                let format = unsafe {
                    configure_float8_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [
                            read_f32(&uniform, MSDF_VIEWPORT_FLOAT_OFFSET)?,
                            read_f32(&uniform, MSDF_VIEWPORT_FLOAT_OFFSET + 1)?,
                        ],
                    );
                    set_vec2(
                        gl,
                        program,
                        "u_tex_size",
                        [
                            read_f32(&uniform, MSDF_TEXTURE_SIZE_FLOAT_OFFSET)?,
                            read_f32(&uniform, MSDF_TEXTURE_SIZE_FLOAT_OFFSET + 1)?,
                        ],
                    );
                    set_f32(
                        gl,
                        program,
                        "u_range",
                        read_f32(&uniform, MSDF_RANGE_FLOAT_OFFSET)?,
                    );
                    // 同时取得 MSDF 格式与创建时冻结的 sampler 描述。
                    let (format, sampler) = bind_sampled(gl, self, program)?;
                    // 共享采样契约必须同时接受 RGBA8 与线性过滤。
                    if !contract.sampling.accepts(format, sampler) {
                        return Err(rhi_invalid("OpenGL RHI MSDF source is not RGBA8"));
                    }
                    format
                };
                Some(format)
            }
            // 普通与 Additive 圆角/描边矩形使用同一 RectConstants ABI。
            PipelineKind::ShapeRect | PipelineKind::ShapeRectAdditive => {
                // SAFETY: 该分支已校验 stride=8、uniform 为固定 Shape ABI 且 vertex_count>0；program/vao/vertex 存活；uniform 解码有边界检查；context 保持 current。
                unsafe {
                    configure_float2_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [
                            // 读取共享 viewport.width 字段。
                            read_f32(&uniform, SHAPE_VIEWPORT_FLOAT_OFFSET)?,
                            // 读取共享 viewport.height 字段。
                            read_f32(&uniform, SHAPE_VIEWPORT_FLOAT_OFFSET + 1)?,
                        ],
                    );
                    // 按共享字段索引编码原始 Shape 矩形。
                    set_vec4(
                        gl,
                        program,
                        "u_rect",
                        read_vec4(&uniform, SHAPE_RECT_FLOAT_OFFSET)?,
                    );
                    // 按共享字段索引编码直通颜色。
                    set_vec4(
                        gl,
                        program,
                        "u_color",
                        read_vec4(&uniform, SHAPE_COLOR_FLOAT_OFFSET)?,
                    );
                    // 按共享字段索引编码四角半径。
                    set_vec4(
                        gl,
                        program,
                        "u_radius",
                        read_vec4(&uniform, SHAPE_RADIUS_FLOAT_OFFSET)?,
                    );
                    // 一次交付半描边宽度与共享分析抗锯齿 fringe。
                    set_vec4(
                        gl,
                        program,
                        "u_stroke",
                        read_vec4(&uniform, SHAPE_STROKE_FLOAT_OFFSET)?,
                    );
                    // 直接交付共享 GPU Raster Module 计算的实际绘制边界。
                    set_vec4(
                        gl,
                        program,
                        "u_draw_rect",
                        read_vec4(&uniform, SHAPE_DRAW_RECT_FLOAT_OFFSET)?,
                    );
                }
                None
            }
            // 原生扇形使用 64 字节 SectorConstants 和矩形单位 quad。
            PipelineKind::Sector => {
                // SAFETY: 该分支已校验 stride=8、uniform 为固定字节数且 vertex_count>0；program/vao/vertex 存活；uniform 解码有边界检查；context 保持 current。
                unsafe {
                    configure_float2_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [
                            // 读取共享 Sector viewport.width 字段。
                            read_f32(&uniform, SECTOR_VIEWPORT_FLOAT_OFFSET)?,
                            // 读取共享 Sector viewport.height 字段。
                            read_f32(&uniform, SECTOR_VIEWPORT_FLOAT_OFFSET + 1)?,
                        ],
                    );
                    // 按共享字段索引映射扇形外接矩形。
                    set_vec4(
                        gl,
                        program,
                        "u_rect",
                        read_vec4(&uniform, SECTOR_RECT_FLOAT_OFFSET)?,
                    );
                    // 按共享字段索引映射 straight-alpha 扇形颜色。
                    set_vec4(
                        gl,
                        program,
                        "u_color",
                        read_vec4(&uniform, SECTOR_COLOR_FLOAT_OFFSET)?,
                    );
                    // 按共享字段索引映射起始角与扫过角。
                    set_vec4(
                        gl,
                        program,
                        "u_angles",
                        read_vec4(&uniform, SECTOR_ANGLES_FLOAT_OFFSET)?,
                    );
                }
                None
            }
            // 仿射阴影使用 96 字节 AffineShadowConstants 和 straight-alpha blend。
            PipelineKind::BoxShadow => {
                // SAFETY: 该分支已校验 stride=8、uniform=96 字节且 vertex_count>0；program/vao/vertex 存活；uniform 解码有边界检查；context 保持 current。
                unsafe {
                    configure_float2_attributes(gl, vertex_stride);
                    set_vec2(
                        gl,
                        program,
                        "u_viewport",
                        [
                            // 读取共享 viewport.width 字段。
                            read_f32(&uniform, SHADOW_VIEWPORT_FLOAT_OFFSET)?,
                            // 读取共享 viewport.height 字段。
                            read_f32(&uniform, SHADOW_VIEWPORT_FLOAT_OFFSET + 1)?,
                        ],
                    );
                    // 按共享字段索引映射仿射原点与 X 边。
                    set_vec4(
                        gl,
                        program,
                        "u_rect",
                        read_vec4(&uniform, SHADOW_ORIGIN_EDGE_X_FLOAT_OFFSET)?,
                    );
                    // 按共享字段索引映射 straight-alpha 阴影颜色。
                    set_vec4(
                        gl,
                        program,
                        "u_color",
                        read_vec4(&uniform, SHADOW_COLOR_FLOAT_OFFSET)?,
                    );
                    // 按共享字段索引映射四角半径。
                    set_vec4(
                        gl,
                        program,
                        "u_radius",
                        read_vec4(&uniform, SHADOW_RADIUS_FLOAT_OFFSET)?,
                    );
                    // 按共享字段索引映射仿射 Y 边与两轴模糊量。
                    set_vec4(
                        gl,
                        program,
                        "u_params",
                        read_vec4(&uniform, SHADOW_EDGE_Y_BLUR_FLOAT_OFFSET)?,
                    );
                    // 按共享字段索引映射本体尺寸与环境曲线标记。
                    set_vec4(
                        gl,
                        program,
                        "u_size",
                        read_vec4(&uniform, SHADOW_BODY_SIZE_AMBIENT_FLOAT_OFFSET)?,
                    );
                }
                None
            }
            // blur 使用 NDC float2 区域和共享 BlurConstants。
            PipelineKind::BlurPass => {
                // SAFETY: 该分支已校验共享 Blur ABI、stride=8 且 vertex_count>0；program/vao/vertex/texture/sampler 均存活；weights 使用共享固定容量；颜色源格式由随后的门禁检查兜底；context 保持 current。
                let format = unsafe {
                    configure_float2_attributes(gl, vertex_stride);
                    // 按共享字段索引映射目标与 source texture 尺寸。
                    set_vec4(
                        gl,
                        program,
                        "u_sizes",
                        read_vec4(&uniform, BLUR_SIZES_FLOAT_OFFSET)?,
                    );
                    // 按共享字段索引映射采样区域原点与尺寸。
                    set_vec4(
                        gl,
                        program,
                        "u_region",
                        read_vec4(&uniform, BLUR_REGION_FLOAT_OFFSET)?,
                    );
                    // 按共享字段索引映射像素方向与 tap 半径。
                    set_vec4(
                        gl,
                        program,
                        "u_dir_taps",
                        read_vec4(&uniform, BLUR_DIRECTION_TAPS_FLOAT_OFFSET)?,
                    );
                    // 使用共享权重数量创建精确的 Adapter 临时映射。
                    let mut weights = [0.0f32; BLUR_WEIGHT_COUNT];
                    // 逐项读取共享权重区间，不在 Adapter 重新定义起始槽。
                    for (index, weight) in weights.iter_mut().enumerate() {
                        // 保持 Drawing 已经归一化的权重顺序。
                        *weight = read_f32(&uniform, BLUR_WEIGHTS_FLOAT_OFFSET + index)?;
                    }
                    // 查找固定十六个 float4 权重数组的位置。
                    let location = gl.get_uniform_location(program, "u_weights");
                    // 一次上传完整共享权重区间。
                    gl.uniform_4_f32_slice(location.as_ref(), &weights);
                    // 同时取得 Blur 源格式与创建时冻结的 sampler 描述。
                    let (format, sampler) = bind_sampled(gl, self, program)?;
                    // 共享采样契约必须同时接受颜色格式与线性过滤。
                    if !contract.sampling.accepts(format, sampler) {
                        return Err(rhi_invalid("OpenGL RHI blur source is not a color texture"));
                    }
                    format
                };
                Some(format)
            }
        };
        // 记录 sampled_format 只用于保持每类 ABI 的显式门禁。
        let _ = sampled_format;
        // SAFETY: contract 只包含固定 GL 采样覆盖映射，当前 owner thread 的 context 保持 current。
        unsafe { apply_pipeline_multisample(gl, contract.multisample) };
        // SAFETY: contract 只包含固定 GL 光栅与深度模板映射，当前 owner thread 的 context 保持 current。
        unsafe { apply_pipeline_fixed_state(gl, contract.raster, contract.depth_stencil) };
        // SAFETY: contract 只包含固定 GL blend 映射，当前 owner thread 的 context 保持 current。
        unsafe { apply_pipeline_blend(gl, contract.blend) };
        // 把共享原语拓扑翻译一次，供 indexed 与 non-indexed draw 共用。
        let primitive_topology = gl_primitive_topology(contract.topology);
        // Indexed draw 需要固定 uint32 index ABI，非 indexed draw 使用顶点范围。
        // SAFETY: index buffer 已校验为 Index usage 且 stride=4；first_index.saturating_mul(4) 防止偏移溢出；index_count/vertex_count 已校验非零；program/vao/vertex 存活；context 保持 current。
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
                    primitive_topology,
                    packet.index_count as i32,
                    glow::UNSIGNED_INT,
                    packet.first_index.saturating_mul(4) as i32,
                );
            } else {
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, None);
                gl.draw_arrays(
                    primitive_topology,
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

/// 配置 position float2 顶点属性。
///
/// # Safety
/// 调用者必须保证当前绑定的是与 float2 ABI 匹配的 VAO 和顶点 buffer，且 context current。
unsafe fn configure_float2_attributes(gl: &glow::Context, stride: u32) {
    // SAFETY：调用者已绑定与 float2 ABI 匹配的 VAO 和顶点缓冲。
    unsafe { gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride as i32, 0) };
    // SAFETY：属性零属于当前绑定 VAO 的 position 槽位。
    unsafe { gl.enable_vertex_attrib_array(0) };
    // SAFETY：关闭旧属性只修改当前绑定 VAO 的状态。
    unsafe { gl.disable_vertex_attrib_array(1) };
    // SAFETY：关闭旧属性只修改当前绑定 VAO 的状态。
    unsafe { gl.disable_vertex_attrib_array(2) };
}

/// 配置 position/uv/color float8 顶点属性。
///
/// # Safety
/// 调用者必须保证当前绑定的是与 float8 ABI 匹配的 VAO 和顶点 buffer，且 context current。
unsafe fn configure_float8_attributes(gl: &glow::Context, stride: u32) {
    // SAFETY：调用者已绑定与 float8 ABI 匹配的 VAO 和顶点缓冲。
    unsafe { gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride as i32, 0) };
    // SAFETY：属性零属于当前绑定 VAO 的 position 槽位。
    unsafe { gl.enable_vertex_attrib_array(0) };
    // SAFETY：八字节偏移与通用 float8 顶点 ABI 的 uv 槽位一致。
    unsafe { gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride as i32, 8) };
    // SAFETY：属性一属于当前绑定 VAO 的 uv 槽位。
    unsafe { gl.enable_vertex_attrib_array(1) };
    // SAFETY：十六字节偏移与通用 float8 顶点 ABI 的 color 槽位一致。
    unsafe { gl.vertex_attrib_pointer_f32(2, 4, glow::FLOAT, false, stride as i32, 16) };
    // SAFETY：属性二属于当前绑定 VAO 的 color 槽位。
    unsafe { gl.enable_vertex_attrib_array(2) };
}

// 把 API 无关混合因子翻译为 OpenGL ES 枚举。
fn gl_blend_factor(factor: PipelineBlendFactor) -> u32 {
    // 只映射共享层允许的封闭因子集合。
    match factor {
        // Zero 对应 GL_ZERO。
        PipelineBlendFactor::Zero => glow::ZERO,
        // One 对应 GL_ONE。
        PipelineBlendFactor::One => glow::ONE,
        // SourceAlpha 对应 GL_SRC_ALPHA。
        PipelineBlendFactor::SourceAlpha => glow::SRC_ALPHA,
        // OneMinusSourceAlpha 对应 GL_ONE_MINUS_SRC_ALPHA。
        PipelineBlendFactor::OneMinusSourceAlpha => glow::ONE_MINUS_SRC_ALPHA,
    }
}

// 把 API 无关混合运算翻译为 OpenGL ES 枚举。
fn gl_blend_operation(operation: PipelineBlendOperation) -> u32 {
    // 只映射共享层允许的封闭运算集合。
    match operation {
        // Add 对应 GL_FUNC_ADD。
        PipelineBlendOperation::Add => glow::FUNC_ADD,
    }
}

// 把 API 无关颜色写掩码翻译为 OpenGL ES 的四通道开关。
fn gl_color_write_mask(mask: PipelineColorWriteMask) -> [bool; 4] {
    // 只映射共享层允许的封闭写掩码集合。
    match mask {
        // All 对应完整 RGBA 写入。
        PipelineColorWriteMask::All => [true, true, true, true],
    }
}

// 把 API 无关正面绕序翻译为 OpenGL ES 枚举。
fn gl_front_face(front_face: PipelineFrontFace) -> u32 {
    // 只映射共享层允许的封闭绕序集合。
    match front_face {
        // CounterClockwise 对应 GL_CCW。
        PipelineFrontFace::CounterClockwise => glow::CCW,
    }
}

// 把 API 无关原语拓扑翻译为 OpenGL ES 枚举。
fn gl_primitive_topology(topology: PipelinePrimitiveTopology) -> u32 {
    // 只映射共享层允许的封闭拓扑集合。
    match topology {
        // TriangleList 对应每三个顶点形成独立三角形的 GL_TRIANGLES。
        PipelinePrimitiveTopology::TriangleList => glow::TRIANGLES,
    }
}

/// 按共享 pipeline 状态恢复采样覆盖状态。
///
/// # Safety
/// 调用者必须保证当前 owner thread 的 GL context current。
unsafe fn apply_pipeline_multisample(gl: &glow::Context, state: PipelineMultisampleState) {
    // SAFETY：调用者保证当前 owner thread 的 GL context current。
    unsafe {
        // 穷尽映射共享采样覆盖集合。
        match state {
            // 单样本路径显式关闭两种可能改变覆盖率的兼容 context 状态。
            PipelineMultisampleState::SingleSample => {
                // 关闭按 alpha 生成样本覆盖率。
                gl.disable(glow::SAMPLE_ALPHA_TO_COVERAGE);
                // 关闭按 sample coverage 值裁剪覆盖率。
                gl.disable(glow::SAMPLE_COVERAGE);
            }
        }
    }
}

/// 按共享 pipeline 状态恢复二维光栅与深度模板状态。
///
/// # Safety
/// 调用者必须保证当前 owner thread 的 GL context current。
unsafe fn apply_pipeline_fixed_state(
    // 借用当前 OpenGL ES 上下文。
    gl: &glow::Context,
    // 接收共享二维光栅状态。
    raster: PipelineRasterState,
    // 接收共享深度模板状态。
    depth_stencil: PipelineDepthStencilState,
) {
    // SAFETY：调用者保证当前 owner thread 的 GL context current。
    unsafe {
        // 穷尽映射共享面剔除语义。
        match raster.cull_mode {
            // UIX 二维图元显式关闭面剔除。
            PipelineCullMode::None => gl.disable(glow::CULL_FACE),
        }
        // 显式恢复共享正面绕序，禁止继承兼容 context 状态。
        gl.front_face(gl_front_face(raster.front_face));
        // OpenGL ES 固定启用裁剪体，只接受与该事实相同的共享语义。
        match raster.depth_clip {
            // Enabled 由 OpenGL ES 固定裁剪体直接满足。
            PipelineDepthClip::Enabled => {}
        }
        // 穷尽映射共享深度状态。
        match depth_stencil.depth {
            // 禁用深度测试与写入，禁止继承兼容 context 状态。
            PipelineDepthState::Disabled => {
                // 关闭深度测试。
                gl.disable(glow::DEPTH_TEST);
                // 关闭深度写入。
                gl.depth_mask(false);
            }
        }
        // 穷尽映射共享模板状态。
        match depth_stencil.stencil {
            // 禁用模板测试与写入，禁止继承兼容 context 状态。
            PipelineStencilState::Disabled => {
                // 关闭模板测试。
                gl.disable(glow::STENCIL_TEST);
                // 关闭模板写入。
                gl.stencil_mask(0);
            }
        }
    }
}

/// 按共享 pipeline 状态设置唯一颜色混合公式。
///
/// # Safety
/// 调用者必须保证当前 owner thread 的 GL context current。
unsafe fn apply_pipeline_blend(gl: &glow::Context, blend: PipelineBlend) {
    // 从共享层读取完整颜色与 alpha 因子。
    let state = blend.state();
    // 把共享写掩码翻译为 OpenGL 的四个通道开关。
    let [write_red, write_green, write_blue, write_alpha] = gl_color_write_mask(state.write_mask);
    // SAFETY：调用者保证当前 owner thread 的 GL context current。
    unsafe {
        // 每次 draw 前显式恢复写掩码，禁止继承兼容 context 的遗留状态。
        gl.color_mask(write_red, write_green, write_blue, write_alpha);
    }
    // Replace 等语义关闭硬件混合并直接覆盖目标。
    if !state.enabled {
        // SAFETY：调用者保证当前 owner thread 的 GL context current。
        unsafe { gl.disable(glow::BLEND) };
        // 禁用后无需设置不会生效的因子。
        return;
    }
    // SAFETY：调用者保证当前 owner thread 的 GL context current。
    unsafe { gl.enable(glow::BLEND) };
    // SAFETY：共享因子和运算全部映射为当前 OpenGL ES 支持的固定枚举。
    unsafe {
        // 显式恢复 RGB 与 alpha 运算，禁止继承兼容 context 的遗留 equation。
        gl.blend_equation_separate(
            // 翻译 RGB 混合运算。
            gl_blend_operation(state.color_operation),
            // 翻译 alpha 混合运算。
            gl_blend_operation(state.alpha_operation),
        );
        // 一次设置 RGB 与 alpha 的四个共享因子。
        gl.blend_func_separate(
            // 翻译源 RGB 因子。
            gl_blend_factor(state.source_color),
            // 翻译目标 RGB 因子。
            gl_blend_factor(state.destination_color),
            // 翻译源 alpha 因子。
            gl_blend_factor(state.source_alpha),
            // 翻译目标 alpha 因子。
            gl_blend_factor(state.destination_alpha),
        );
    }
}

// 验证 render target 身份到 NDC Y 方向的纯映射。
#[cfg(test)]
mod target_direction_tests {
    // 引入父模块私有目标方向 helper 与 surface sentinel。
    use super::super::{RHI_SURFACE_TARGET_RAW, target_y_sign};
    // 引入 opaque render target 句柄。
    use crate::native::present::rhi::RenderTargetHandle;

    // 锁定原生 surface 把左上逻辑坐标映射到 GL 高 Y。
    #[test]
    fn surface_target_uses_window_top_direction() {
        // 构造 adapter 保留的原生 surface 句柄。
        let surface = RenderTargetHandle::from_raw(RHI_SURFACE_TARGET_RAW);
        // surface 顶部必须通过负符号从逻辑 Y-down 映射到 NDC Y-up。
        assert_eq!(target_y_sign(surface), -1.0);
    }

    // 锁定 texture target 把逻辑顶部保存为可直接采样的 v=0 行。
    #[test]
    fn texture_target_uses_top_left_storage_direction() {
        // 构造普通非零 texture render target 句柄。
        let texture = RenderTargetHandle::from_raw(1);
        // texture 目标必须保留正符号，让逻辑顶部写入 GL 第零行。
        assert_eq!(target_y_sign(texture), 1.0);
    }
}
