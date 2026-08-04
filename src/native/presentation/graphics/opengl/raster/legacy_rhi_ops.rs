// 复用父模块的 OpenGL pipeline 类型、固定顶点结构和 GL 辅助函数。
use super::*;

// 引入 legacy native queue 的绘制 DTO。
use crate::native::present::{
    GpuImageBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSector, GpuSolidMesh,
};

// 保存 legacy queue 共用的薄 RHI shader 与动态图片纹理资源。
pub(super) struct LegacyNativeOps {
    // 保存线性和径向渐变共用的 shader program。
    gradient_program: glow::Program,
    // 保存渐变 viewport uniform。
    gradient_viewport: Option<glow::UniformLocation>,
    // 保存渐变 origin/edge_x uniform。
    gradient_origin_edge_x: Option<glow::UniformLocation>,
    // 保存渐变 edge_y uniform。
    gradient_edge_y: Option<glow::UniformLocation>,
    // 保存渐变起始颜色 uniform。
    gradient_color_a: Option<glow::UniformLocation>,
    // 保存渐变结束颜色 uniform。
    gradient_color_b: Option<glow::UniformLocation>,
    // 保存渐变 mode、方向或半径参数 uniform。
    gradient_params: Option<glow::UniformLocation>,
    // 保存 sector shader program。
    sector_program: glow::Program,
    // 保存 sector viewport uniform。
    sector_viewport: Option<glow::UniformLocation>,
    // 保存 sector 外接矩形 uniform。
    sector_rect: Option<glow::UniformLocation>,
    // 保存 sector 颜色 uniform。
    sector_color: Option<glow::UniformLocation>,
    // 保存 sector 角度 uniform。
    sector_angles: Option<glow::UniformLocation>,
    // 保存 solid mesh shader program。
    mesh_program: glow::Program,
    // 保存 solid mesh 专用 vertex array。
    mesh_vao: glow::VertexArray,
    // 保存 solid mesh 动态 vertex buffer。
    mesh_vbo: glow::Buffer,
    // 保存 mesh viewport uniform。
    mesh_viewport: Option<glow::UniformLocation>,
    // 保存 mesh premultiplied 颜色 uniform。
    mesh_color: Option<glow::UniformLocation>,
    // 保存任意仿射图片共用的 textured shader program。
    image_program: glow::Program,
    // 保存图片 viewport uniform。
    image_viewport: Option<glow::UniformLocation>,
    // 保存图片采样器 uniform。
    image_texture_uniform: Option<glow::UniformLocation>,
    // 保存可跨 blit 复用的 BGRA 图片纹理。
    image_texture: Option<glow::Texture>,
    // 保存当前图片纹理尺寸。
    image_width: u32,
    // 保存当前图片纹理高度。
    image_height: u32,
}

impl LegacyNativeOps {
    // 编译 legacy queue 复用的固定 GLSL ABI 并创建 mesh 缓冲。
    pub(super) fn new(gl: &glow::Context) -> Result<Self> {
        // 编译渐变 shader，保持与通用 RHI 的 key ABI 一致。
        let gradient_program = unsafe {
            super::compile_program(
                gl,
                super::rhi_shaders::GRADIENT_VERTEX,
                super::rhi_shaders::GRADIENT_FRAGMENT,
                "legacy gradient",
            )?
        };
        // 编译 sector shader，复用矩形单位 quad 顶点阶段。
        let sector_program = unsafe {
            super::compile_program(
                gl,
                super::rhi_shaders::RECT_VERTEX,
                super::rhi_shaders::SECTOR_FRAGMENT,
                "legacy sector",
            )?
        };
        // 编译 solid mesh shader，颜色由上层按 premultiplied 语义规整。
        let mesh_program = unsafe {
            super::compile_program(
                gl,
                super::rhi_shaders::SOLID_VERTEX,
                super::rhi_shaders::SOLID_FRAGMENT,
                "legacy solid mesh",
            )?
        };
        // 编译任意四角图片的 textured shader，采样保持 BGRA ABI。
        let image_program = unsafe {
            super::compile_program(
                gl,
                super::rhi_shaders::TEXTURED_VERTEX,
                super::rhi_shaders::TEXTURED_FRAGMENT,
                "legacy image blit",
            )?
        };
        // 创建 solid mesh 的动态 float2 vertex buffer。
        let (mesh_vao, mesh_vbo) = unsafe { create_mesh_buffer(gl)? };
        // 缓存所有固定 uniform 位置，避免每次 draw 重新查找。
        Ok(Self {
            // 缓存渐变 uniforms。
            gradient_program,
            gradient_viewport: unsafe { gl.get_uniform_location(gradient_program, "u_viewport") },
            gradient_origin_edge_x: unsafe {
                gl.get_uniform_location(gradient_program, "u_quad_origin_edge_x")
            },
            gradient_edge_y: unsafe { gl.get_uniform_location(gradient_program, "u_quad_edge_y") },
            gradient_color_a: unsafe { gl.get_uniform_location(gradient_program, "u_color_a") },
            gradient_color_b: unsafe { gl.get_uniform_location(gradient_program, "u_color_b") },
            gradient_params: unsafe { gl.get_uniform_location(gradient_program, "u_params") },
            // 缓存 sector uniforms。
            sector_program,
            sector_viewport: unsafe { gl.get_uniform_location(sector_program, "u_viewport") },
            sector_rect: unsafe { gl.get_uniform_location(sector_program, "u_rect") },
            sector_color: unsafe { gl.get_uniform_location(sector_program, "u_color") },
            sector_angles: unsafe { gl.get_uniform_location(sector_program, "u_angles") },
            // 缓存 mesh program 和对象。
            mesh_program,
            mesh_vao,
            mesh_vbo,
            mesh_viewport: unsafe { gl.get_uniform_location(mesh_program, "u_viewport") },
            mesh_color: unsafe { gl.get_uniform_location(mesh_program, "u_color") },
            // 缓存图片 program 和 uniforms。
            image_program,
            image_viewport: unsafe { gl.get_uniform_location(image_program, "u_viewport") },
            image_texture_uniform: unsafe { gl.get_uniform_location(image_program, "u_tex") },
            // 图片纹理按首个 blit 的源尺寸延迟创建。
            image_texture: None,
            image_width: 0,
            image_height: 0,
        })
    }

    // 确保动态图片纹理与当前 blit 的源尺寸一致。
    fn ensure_image_texture(
        &mut self,
        gl: &glow::Context,
        width: u32,
        height: u32,
    ) -> Result<glow::Texture> {
        // 相同尺寸直接复用已有 texture。
        if self.image_texture.is_none() || self.image_width != width || self.image_height != height
        {
            // 使用通用 RGBA texture 容器承载 little-endian BGRA bytes。
            let texture = unsafe { super::create_texture(gl, width as i32, height as i32)? };
            // 释放旧尺寸纹理，避免每次图片 blit 累积 GPU 对象。
            if let Some(previous) = self.image_texture.replace(texture) {
                unsafe { gl.delete_texture(previous) };
            }
            // 记录纹理尺寸供后续复用判断。
            self.image_width = width;
            self.image_height = height;
        }
        // 新建后此处必然拥有有效图片纹理。
        self.image_texture.ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "OpenGL legacy image texture was not created",
            )
        })
    }

    // 释放 legacy queue 的所有 GL 资源；调用方保证 context current。
    pub(super) unsafe fn release(&mut self, gl: &glow::Context) {
        // 删除固定 shader program。
        gl.delete_program(self.gradient_program);
        gl.delete_program(self.sector_program);
        gl.delete_program(self.mesh_program);
        gl.delete_program(self.image_program);
        // 删除 mesh 的 VAO 和动态 VBO。
        gl.delete_vertex_array(self.mesh_vao);
        gl.delete_buffer(self.mesh_vbo);
        // 删除可复用的图片 texture。
        if let Some(texture) = self.image_texture.take() {
            gl.delete_texture(texture);
        }
    }
}

// 创建只含 float2 position attribute 的 solid mesh VAO/VBO。
unsafe fn create_mesh_buffer(gl: &glow::Context) -> Result<(glow::VertexArray, glow::Buffer)> {
    // 创建 mesh VAO。
    let vao = gl
        .create_vertex_array()
        .map_err(|error| super::gl_error("create legacy mesh vertex array", error))?;
    // 创建 mesh VBO，失败时回收已经创建的 VAO。
    let vbo = match gl.create_buffer() {
        Ok(vbo) => vbo,
        Err(error) => {
            gl.delete_vertex_array(vao);
            return Err(super::gl_error("create legacy mesh buffer", error));
        }
    };
    // 固定 float2 position layout，stride 为两个 float。
    gl.bind_vertex_array(Some(vao));
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
    gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
    gl.enable_vertex_attrib_array(0);
    gl.bind_vertex_array(None);
    gl.bind_buffer(glow::ARRAY_BUFFER, None);
    // 返回已配置的 mesh VAO/VBO。
    Ok((vao, vbo))
}

// 检查 native queue 传入的四角几何是否可以安全进入 GLSL。
fn finite_corners(corners: &[[f32; 2]; 4]) -> bool {
    // 任何非有限坐标都不得进入 vertex shader。
    corners.iter().flatten().all(|value| value.is_finite())
}

// 检查颜色浮点载荷是否有限。
fn finite_color(color: &[f32; 4]) -> bool {
    // 颜色异常时跳过当前 draw，保持与 D3D11 native queue 的容错边界一致。
    color.iter().all(|value| value.is_finite())
}

// 设置渐变 shader 所需的四角、颜色和参数常量。
unsafe fn set_gradient_uniforms(
    gl: &glow::Context,
    ops: &LegacyNativeOps,
    viewport_width: f32,
    viewport_height: f32,
    corners: [[f32; 2]; 4],
    color_a: [f32; 4],
    color_b: [f32; 4],
    mut params: [f32; 4],
) {
    // 以 TL 角和两条边恢复 affine parallelogram。
    let origin = corners[0];
    let edge_x = [corners[1][0] - origin[0], corners[1][1] - origin[1]];
    let edge_y = [corners[3][0] - origin[0], corners[3][1] - origin[1]];
    // 线性对角方向沿用通用 RHI 的变换后边长语义。
    if params[0] < 0.5 {
        params[2] = (edge_x[0] * edge_x[0] + edge_x[1] * edge_x[1]).sqrt();
        params[3] = (edge_y[0] * edge_y[0] + edge_y[1] * edge_y[1]).sqrt();
    }
    // 上传 viewport、四角、颜色和模式参数。
    gl.uniform_2_f32(
        ops.gradient_viewport.as_ref(),
        viewport_width.max(1.0),
        viewport_height.max(1.0),
    );
    gl.uniform_4_f32(
        ops.gradient_origin_edge_x.as_ref(),
        origin[0],
        origin[1],
        edge_x[0],
        edge_x[1],
    );
    gl.uniform_4_f32(ops.gradient_edge_y.as_ref(), edge_y[0], edge_y[1], 0.0, 0.0);
    gl.uniform_4_f32(
        ops.gradient_color_a.as_ref(),
        color_a[0],
        color_a[1],
        color_a[2],
        color_a[3],
    );
    gl.uniform_4_f32(
        ops.gradient_color_b.as_ref(),
        color_b[0],
        color_b[1],
        color_b[2],
        color_b[3],
    );
    gl.uniform_4_f32(
        ops.gradient_params.as_ref(),
        params[0],
        params[1],
        params[2],
        params[3],
    );
}

// OpenGL ES legacy native queue 的渐变、mesh、sector 和图片绘制入口。
impl OpenGlRasterPipeline {
    // 绘制保留 affine 四角的线性渐变 batch。
    pub(crate) fn draw_linear_gradients(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuLinearGradientRect],
    ) -> Result<()> {
        // 空 batch 或无效 viewport 不产生 GL 状态变化。
        if rects.is_empty() || viewport_width <= 0.0 || viewport_height <= 0.0 {
            return Ok(());
        }
        // 应用与其它 native queue 方法相同的逻辑 scissor。
        self.apply_scissor(scissor);
        unsafe {
            // 渐变 fragment 已输出 premultiplied RGBA。
            self.gl().enable(glow::BLEND);
            self.gl().blend_func_separate(
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
            );
            // 绑定渐变 program、viewport 和单位 rect VAO。
            self.gl().use_program(Some(self.legacy.gradient_program));
            self.gl().bind_vertex_array(Some(self.rect_vao));
            for rect in rects {
                // 跳过退化矩形、异常四角、异常方向和异常颜色。
                if rect.w <= 0.0
                    || rect.h <= 0.0
                    || rect.dir > 3
                    || !finite_corners(&rect.corners)
                    || !finite_color(&rect.color_a)
                    || !finite_color(&rect.color_b)
                {
                    continue;
                }
                // 上传当前线性渐变的 affine 常量。
                set_gradient_uniforms(
                    self.gl(),
                    &self.legacy,
                    viewport_width,
                    viewport_height,
                    rect.corners,
                    rect.color_a,
                    rect.color_b,
                    [0.0, rect.dir as f32, 0.0, 0.0],
                );
                // 六顶点单位 quad 覆盖当前 affine 四角。
                self.gl().draw_arrays(glow::TRIANGLES, 0, 6);
            }
            // 解除 VAO，避免把 legacy 状态泄漏给后续 pipeline。
            self.gl().bind_vertex_array(None);
        }
        // 在当前调用边界检查 GL 错误。
        self.check_gl_error("draw_linear_gradients")
    }

    // 绘制保留 affine 四角的径向渐变 batch。
    pub(crate) fn draw_radial_gradients(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        grads: &[GpuRadialGradient],
    ) -> Result<()> {
        // 空 batch 或无效 viewport 不产生 GL 状态变化。
        if grads.is_empty() || viewport_width <= 0.0 || viewport_height <= 0.0 {
            return Ok(());
        }
        // 应用与其它 native queue 方法相同的逻辑 scissor。
        self.apply_scissor(scissor);
        unsafe {
            // 径向渐变 fragment 已输出 premultiplied RGBA。
            self.gl().enable(glow::BLEND);
            self.gl().blend_func_separate(
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
            );
            // 绑定渐变 program、viewport 和单位 rect VAO。
            self.gl().use_program(Some(self.legacy.gradient_program));
            self.gl().bind_vertex_array(Some(self.rect_vao));
            for grad in grads {
                // 跳过退化半径、异常四角、中心和颜色。
                if !grad.cx.is_finite()
                    || !grad.cy.is_finite()
                    || !grad.inner_r.is_finite()
                    || !grad.outer_r.is_finite()
                    || grad.outer_r <= 0.0
                    || grad.inner_r < 0.0
                    || grad.inner_r > grad.outer_r
                    || !finite_corners(&grad.corners)
                    || !finite_color(&grad.color_inner)
                    || !finite_color(&grad.color_outer)
                {
                    continue;
                }
                // 使用与通用 RHI 相同的径向 inner/outer 比值 ABI。
                set_gradient_uniforms(
                    self.gl(),
                    &self.legacy,
                    viewport_width,
                    viewport_height,
                    grad.corners,
                    grad.color_inner,
                    grad.color_outer,
                    [1.0, (grad.inner_r / grad.outer_r) * 0.5, 0.5, 0.0],
                );
                // 六顶点单位 quad 覆盖当前 affine 四角。
                self.gl().draw_arrays(glow::TRIANGLES, 0, 6);
            }
            // 解除 VAO，避免把 legacy 状态泄漏给后续 pipeline。
            self.gl().bind_vertex_array(None);
        }
        // 在当前调用边界检查 GL 错误。
        self.check_gl_error("draw_radial_gradients")
    }

    // 绘制分析抗锯齿圆形 sector batch。
    pub(crate) fn draw_sectors(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        sectors: &[GpuSector],
    ) -> Result<()> {
        // 空 batch 或无效 viewport 不产生 GL 状态变化。
        if sectors.is_empty() || viewport_width <= 0.0 || viewport_height <= 0.0 {
            return Ok(());
        }
        // 应用与其它 native queue 方法相同的逻辑 scissor。
        self.apply_scissor(scissor);
        unsafe {
            // sector fragment 已输出 premultiplied RGBA。
            self.gl().enable(glow::BLEND);
            self.gl().blend_func_separate(
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
            );
            // 绑定 sector program、viewport 和单位 rect VAO。
            self.gl().use_program(Some(self.legacy.sector_program));
            self.gl().uniform_2_f32(
                self.legacy.sector_viewport.as_ref(),
                viewport_width.max(1.0),
                viewport_height.max(1.0),
            );
            self.gl().bind_vertex_array(Some(self.rect_vao));
            for sector in sectors {
                // 跳过退化半径、异常角度和异常颜色。
                if !sector.cx.is_finite()
                    || !sector.cy.is_finite()
                    || !sector.radius.is_finite()
                    || sector.radius <= 0.0
                    || !sector.start_angle.is_finite()
                    || !sector.sweep_angle.is_finite()
                    || sector.start_angle < 0.0
                    || sector.start_angle >= std::f32::consts::TAU
                    || sector.sweep_angle <= 0.0
                    || sector.sweep_angle > std::f32::consts::TAU
                    || !finite_color(&sector.rgba)
                {
                    continue;
                }
                // 上传 sector 外接矩形、颜色和角度常量。
                self.gl().uniform_4_f32(
                    self.legacy.sector_rect.as_ref(),
                    sector.cx - sector.radius,
                    sector.cy - sector.radius,
                    sector.radius * 2.0,
                    sector.radius * 2.0,
                );
                self.gl().uniform_4_f32(
                    self.legacy.sector_color.as_ref(),
                    sector.rgba[0],
                    sector.rgba[1],
                    sector.rgba[2],
                    sector.rgba[3],
                );
                self.gl().uniform_4_f32(
                    self.legacy.sector_angles.as_ref(),
                    sector.start_angle,
                    sector.sweep_angle,
                    0.0,
                    0.0,
                );
                // 六顶点单位 quad 覆盖 sector 外接矩形。
                self.gl().draw_arrays(glow::TRIANGLES, 0, 6);
            }
            // 解除 VAO，避免把 legacy 状态泄漏给后续 pipeline。
            self.gl().bind_vertex_array(None);
        }
        // 在当前调用边界检查 GL 错误。
        self.check_gl_error("draw_sectors")
    }

    // 绘制 CPU tessellation 产生的 solid triangle-list mesh。
    pub(crate) fn draw_solid_meshes(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        meshes: &[GpuSolidMesh],
    ) -> Result<()> {
        // 空 batch 或无效 viewport 不产生 GL 状态变化。
        if meshes.is_empty() || viewport_width <= 0.0 || viewport_height <= 0.0 {
            return Ok(());
        }
        // 应用与其它 native queue 方法相同的逻辑 scissor。
        self.apply_scissor(scissor);
        unsafe {
            // mesh 颜色已经是 premultiplied RGBA。
            self.gl().enable(glow::BLEND);
            self.gl().blend_func_separate(
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
                glow::ONE,
                glow::ONE_MINUS_SRC_ALPHA,
            );
            // 绑定 solid mesh program、viewport 和专用 VAO。
            self.gl().use_program(Some(self.legacy.mesh_program));
            self.gl().uniform_2_f32(
                self.legacy.mesh_viewport.as_ref(),
                viewport_width.max(1.0),
                viewport_height.max(1.0),
            );
            self.gl().bind_vertex_array(Some(self.legacy.mesh_vao));
            for mesh in meshes {
                // 只接受完整的 xy triangle-list，拒绝越界或奇数载荷。
                if mesh.vertices.len() < 6
                    || mesh.vertices.len() % 2 != 0
                    || !(mesh.vertices.len() / 2).is_multiple_of(3)
                    || mesh.vertices.iter().any(|value| !value.is_finite())
                    || !finite_color(&mesh.rgba)
                {
                    continue;
                }
                // 上传当前 mesh 的动态顶点数据。
                let bytes = std::slice::from_raw_parts(
                    mesh.vertices.as_ptr().cast::<u8>(),
                    std::mem::size_of_val(mesh.vertices.as_ref()),
                );
                self.gl()
                    .bind_buffer(glow::ARRAY_BUFFER, Some(self.legacy.mesh_vbo));
                self.gl()
                    .buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STREAM_DRAW);
                self.gl().uniform_4_f32(
                    self.legacy.mesh_color.as_ref(),
                    mesh.rgba[0],
                    mesh.rgba[1],
                    mesh.rgba[2],
                    mesh.rgba[3],
                );
                // 按 xy 对数绘制三角列表。
                self.gl()
                    .draw_arrays(glow::TRIANGLES, 0, (mesh.vertices.len() / 2) as i32);
            }
            // 解除 VAO，避免把 legacy 状态泄漏给后续 pipeline。
            self.gl().bind_vertex_array(None);
        }
        // 在当前调用边界检查 GL 错误。
        self.check_gl_error("draw_solid_meshes")
    }

    // 上传紧密 BGRA 像素并绘制任意 affine 图片 quad。
    pub(crate) fn draw_image_blits(
        &mut self,
        viewport_width: f32,
        viewport_height: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        blits: &[GpuImageBlit],
    ) -> Result<()> {
        // 空 batch 或无效 viewport 不产生 GL 状态变化。
        if blits.is_empty() || viewport_width <= 0.0 || viewport_height <= 0.0 {
            return Ok(());
        }
        // 应用与其它 native queue 方法相同的逻辑 scissor。
        self.apply_scissor(scissor);
        // 取得当前 owner-thread context 和可复用图片 VAO。
        let gl = self.runtime.context();
        unsafe {
            // 绑定图片 shader、viewport、采样器和 glyph-compatible VAO。
            self.gl().use_program(Some(self.legacy.image_program));
            self.gl().uniform_2_f32(
                self.legacy.image_viewport.as_ref(),
                viewport_width.max(1.0),
                viewport_height.max(1.0),
            );
            self.gl()
                .uniform_1_i32(self.legacy.image_texture_uniform.as_ref(), 0);
            self.gl().bind_vertex_array(Some(self.glyph_vao));
            self.gl()
                .bind_buffer(glow::ARRAY_BUFFER, Some(self.glyph_vbo));
            for blit in blits {
                // 检查源尺寸、紧密 payload、目标 geometry 和透明度。
                let expected = (blit.pixel_w as usize).checked_mul(blit.pixel_h as usize);
                if blit.pixel_w == 0
                    || blit.pixel_h == 0
                    || blit.pixel_w > i32::MAX as u32
                    || blit.pixel_h > i32::MAX as u32
                    || expected != Some(blit.pixels.len())
                    || blit.w <= 0.0
                    || blit.h <= 0.0
                    || !blit.x.is_finite()
                    || !blit.y.is_finite()
                    || !blit.w.is_finite()
                    || !blit.h.is_finite()
                    || !blit.opacity.is_finite()
                    || !finite_corners(&blit.corners)
                {
                    continue;
                }
                // 透明图片无需上传和 draw。
                let opacity = blit.opacity.clamp(0.0, 1.0);
                if opacity <= 0.0 {
                    continue;
                }
                // 复用当前源尺寸的动态纹理。
                let texture = self
                    .legacy
                    .ensure_image_texture(gl, blit.pixel_w, blit.pixel_h)?;
                // 上传 little-endian BGRA payload，避免 GLES row-length 扩展依赖。
                let bytes = std::slice::from_raw_parts(
                    blit.pixels.as_ptr().cast::<u8>(),
                    expected.unwrap_or(0) * std::mem::size_of::<u32>(),
                );
                self.gl().pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
                self.gl().active_texture(glow::TEXTURE0);
                self.gl().bind_texture(glow::TEXTURE_2D, Some(texture));
                self.gl().tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    0,
                    0,
                    blit.pixel_w as i32,
                    blit.pixel_h as i32,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(bytes)),
                );
                self.gl().pixel_store_i32(glow::UNPACK_ALIGNMENT, 4);
                // 使用与通用 RHI sampled quad 一致的六顶点 affine ABI。
                let vertices = [
                    GlyphVertex {
                        pos: blit.corners[0],
                        uv: [0.0, 0.0],
                        color: [opacity; 4],
                    },
                    GlyphVertex {
                        pos: blit.corners[1],
                        uv: [1.0, 0.0],
                        color: [opacity; 4],
                    },
                    GlyphVertex {
                        pos: blit.corners[2],
                        uv: [1.0, 1.0],
                        color: [opacity; 4],
                    },
                    GlyphVertex {
                        pos: blit.corners[0],
                        uv: [0.0, 0.0],
                        color: [opacity; 4],
                    },
                    GlyphVertex {
                        pos: blit.corners[2],
                        uv: [1.0, 1.0],
                        color: [opacity; 4],
                    },
                    GlyphVertex {
                        pos: blit.corners[3],
                        uv: [0.0, 1.0],
                        color: [opacity; 4],
                    },
                ];
                // 上传当前图片的六顶点 geometry。
                let vertex_bytes = std::slice::from_raw_parts(
                    vertices.as_ptr().cast::<u8>(),
                    std::mem::size_of_val(&vertices),
                );
                self.gl()
                    .buffer_data_u8_slice(glow::ARRAY_BUFFER, vertex_bytes, glow::STREAM_DRAW);
                // 图片 premultiplied sample 选择 SrcOver 或 additive blend。
                if blit.additive {
                    self.gl()
                        .blend_func_separate(glow::ONE, glow::ONE, glow::ONE, glow::ONE);
                } else {
                    self.gl().blend_func_separate(
                        glow::ONE,
                        glow::ONE_MINUS_SRC_ALPHA,
                        glow::ONE,
                        glow::ONE_MINUS_SRC_ALPHA,
                    );
                }
                self.gl().enable(glow::BLEND);
                // 绘制当前图片 affine quad。
                self.gl().draw_arrays(glow::TRIANGLES, 0, 6);
            }
            // 清理采样对象和 VAO 绑定，避免影响后续 legacy draw。
            self.gl().bind_texture(glow::TEXTURE_2D, None);
            self.gl().bind_vertex_array(None);
        }
        // 在当前调用边界检查 GL 错误。
        self.check_gl_error("draw_image_blits")
    }
}
