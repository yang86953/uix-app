//! GPU bootstrap 阶段对共享 RHI 基线和 Shape ABI 的真实命令探针。

// 引入探针创建、绘制、提交和销毁所需的共享 RHI 类型。
use super::{
    BufferDesc, BufferUsage, DrawPacket, DrawRange, GraphicsDevice, LoadAction, PipelineDesc,
    PipelineKind, RenderTargetHandle, RhiColor, RhiExtent, RhiScissor, RhiShapeRasterParams,
    RhiTextureRegion, RhiTextureTransfer, RhiViewport, SamplerDesc, TextureDesc, TextureFormat,
    TextureMove,
};
// 引入统一结果类型。
use crate::core::Result;

// 在 bootstrap 阶段执行最小资源、固定 pipeline、Solid 和 Shape 命令探针。
pub(super) fn probe_device<D: GraphicsDevice + ?Sized>(device: &mut D) -> Result<()> {
    // 单位 quad 前三个顶点同时形成 solid 探针所需的最小三角形。
    let unit_vertices = [
        // 左上。
        0.0f32, // 左上。
        0.0,    // 右上。
        1.0,    // 右上。
        0.0,    // 右下。
        1.0,    // 右下。
        1.0,    // 左上。
        0.0,    // 左上。
        0.0,    // 右下。
        1.0,    // 右下。
        1.0,    // 左下。
        0.0,    // 左下。
        1.0,
    ];
    // 把单位 quad 编码为共享 host 上的紧密 float 字节。
    let mut vertex_bytes = Vec::with_capacity(unit_vertices.len() * std::mem::size_of::<f32>());
    // 逐个保持顶点 IEEE float 表示。
    for value in unit_vertices {
        // adapter 与 probe 在同一 host，使用 native-endian ABI。
        vertex_bytes.extend_from_slice(&value.to_ne_bytes());
    }
    // 创建 Solid 与 Shape 共用的真实单位 quad 顶点 buffer。
    let vertex_buffer = device.create_buffer(BufferDesc {
        // 六个 float2 顶点占四十八字节。
        size_bytes: vertex_bytes.len(),
        // 两个 float 组成一个 position。
        stride_bytes: PipelineKind::SolidMesh.contract().vertex.stride_bytes(),
        // 声明顶点用途。
        usage: BufferUsage::Vertex,
    })?;
    // 上传单位 quad，避免零顶点只能验证命令而不能覆盖 Shape 插值。
    device.update_buffer(vertex_buffer, 0, &vertex_bytes)?;
    // 创建 Solid mesh 所需的 32 字节 uniform ABI。
    let solid_uniform_buffer = device.create_buffer(BufferDesc {
        // MeshConstants 固定占三十二字节。
        size_bytes: PipelineKind::SolidMesh.contract().uniform.size_bytes(),
        // uniform 不使用顶点步长。
        stride_bytes: 0,
        // 声明 uniform 用途。
        usage: BufferUsage::Uniform,
    })?;
    // 上传有限 viewport、padding 和透明颜色常量。
    device.update_buffer(
        solid_uniform_buffer,
        0,
        &vec![0; PipelineKind::SolidMesh.contract().uniform.size_bytes()],
    )?;
    // 使用共享 Shape 值对象构造一个真实描边探针。
    let shape_params = RhiShapeRasterParams::new(
        // 使用最小 render target 的物理视口。
        RhiViewport {
            // 视口宽度。
            width: 1.0,
            // 视口高度。
            height: 1.0,
        },
        // 在目标中心放置一个半像素矩形。
        [0.25, 0.25, 0.5, 0.5],
        // 使用透明颜色，探针不污染目标内容语义。
        [0.0; 4],
        // 使用可进入圆角 SDF 的非零半径。
        [0.2; 4],
        // 使用非零半宽确保执行描边与同心双 SDF 分支。
        0.125,
    );
    // 编码完整共享 Shape ABI。
    let shape_uniform_bytes = shape_params.encode_ne_bytes();
    // 创建 Shape 固定 ABI uniform buffer。
    let shape_uniform_buffer = device.create_buffer(BufferDesc {
        // 使用共享常量，禁止 probe 与 renderer 产生布局分叉。
        size_bytes: PipelineKind::ShapeRect.contract().uniform.size_bytes(),
        // uniform 不使用顶点步长。
        stride_bytes: 0,
        // 声明 uniform 用途。
        usage: BufferUsage::Uniform,
    })?;
    // 上传会真实触发描边分支的 Shape 参数。
    device.update_buffer(shape_uniform_buffer, 0, &shape_uniform_bytes)?;
    // 使用最小的 RGBA texture 验证 render target 颜色格式。
    let rgba_texture = device.create_texture(TextureDesc {
        // 使用一像素离屏目标。
        extent: RhiExtent::new(1, 1),
        // 使用通用 RGBA8 格式。
        format: TextureFormat::Rgba8Unorm,
    })?;
    // 使用最小的 BGRA texture 验证主 surface/Picture 采样格式。
    let bgra_texture = device.create_texture(TextureDesc {
        // 使用一像素采样目标。
        extent: RhiExtent::new(1, 1),
        // 使用通用 BGRA8 格式。
        format: TextureFormat::Bgra8Unorm,
    })?;
    // 使用最小的 R8 texture 验证 coverage 采样格式。
    let coverage_texture = device.create_texture(TextureDesc {
        // 使用一像素 coverage 目标。
        extent: RhiExtent::new(1, 1),
        // 使用通用 R8 格式。
        format: TextureFormat::R8Unorm,
    })?;
    // 上传 RGBA 纹理的一个 premultiplied 像素。
    device.update_texture(rgba_texture, RhiExtent::new(1, 1), &[0, 0, 0, 0])?;
    // 上传 BGRA 纹理的一个 premultiplied 像素。
    device.update_texture(bgra_texture, RhiExtent::new(1, 1), &[0, 0, 0, 0])?;
    // 上传 coverage 纹理的一个覆盖率像素。
    device.update_texture(coverage_texture, RhiExtent::new(1, 1), &[0])?;
    // 创建 2x2 RGBA texture，验证 atlas 所需的带偏移子区域上传。
    let region_texture = device.create_texture(TextureDesc {
        // 使用可容纳右下角子区域的两像素目标。
        extent: RhiExtent::new(2, 2),
        // 使用通用 RGBA8 格式。
        format: TextureFormat::Rgba8Unorm,
    })?;
    // 把一个像素写入右下角，避免零偏移整块上传冒充 region 能力。
    device.update_texture_region(
        // 更新探针纹理。
        region_texture,
        // 把右下偏移与单位尺寸封闭为一个区域。
        RhiTextureRegion::from_xy(1, 1, RhiExtent::new(1, 1)),
        // 上传单个透明像素。
        &[0, 0, 0, 0],
    )?;
    // 只有声明区域移动能力的 adapter 才进入 retained framebuffer 探针。
    if device.device_capabilities().texture_region_move {
        // 以同一纹理的重叠区域移动验证 memmove 语义。
        device.move_texture_region(TextureMove::new(
            // 从 region texture 读取。
            region_texture,
            // 写回同一个 region texture。
            region_texture,
            // 将两个原点与单位尺寸封闭为共享传输。
            RhiTextureTransfer::from_xy(1, 1, 0, 0, RhiExtent::new(1, 1)),
        ))?;
    }
    // 创建真实 sampler，验证纹理绑定所需的过滤状态。
    let sampler = device.create_sampler(
        // 使用命名的线性 clamp 语义覆盖生产采样状态。
        SamplerDesc::linear_clamp(),
    )?;
    // 枚举通用 renderer 当前使用的全部固定 pipeline。
    let pipeline_kinds = [
        // Solid mesh。
        PipelineKind::SolidMesh,
        // 普通纹理 quad。
        PipelineKind::TexturedQuad,
        // 渐变矩形。
        PipelineKind::GradientRect,
        // coverage 字形 quad。
        PipelineKind::GlyphCoverageQuad,
        // 普通 Shape。
        PipelineKind::ShapeRect,
        // Additive Shape。
        PipelineKind::ShapeRectAdditive,
        // 阴影。
        PipelineKind::BoxShadow,
        // Additive 纹理 quad。
        PipelineKind::TexturedQuadAdditive,
        // 模糊 pass。
        PipelineKind::BlurPass,
        // MSDF 字形 quad。
        PipelineKind::MsdfGlyphQuad,
        // 扇形。
        PipelineKind::Sector,
    ];
    // 逐个创建 pipeline，让 adapter 在首帧前暴露 shader/layout 失败。
    let mut pipelines = Vec::with_capacity(pipeline_kinds.len());
    // 保留固定顺序以便随后选择 Solid 与 Shape pipeline。
    for kind in pipeline_kinds {
        // 创建当前固定 pipeline。
        pipelines.push(device.create_pipeline(PipelineDesc { kind })?);
    }
    // 在 1x1 离屏颜色 target 上执行真实的 pass、draw 和 submit。
    device.begin_render_pass(
        // 把探针 texture 解释为 render target 句柄。
        RenderTargetHandle::from_raw(rgba_texture.raw()),
        // 以透明色清空目标。
        LoadAction::Clear(RhiColor::transparent()),
    )?;
    // 绑定最小正 viewport，验证 adapter 的 viewport 状态编码。
    device.set_viewport(RhiViewport {
        // 视口宽度。
        width: 1.0,
        // 视口高度。
        height: 1.0,
    })?;
    // 清除显式 scissor，验证 pass 初始 raster 状态可用。
    device.set_scissor(None)?;
    // 只有声明局部清理能力的 adapter 才进入 ClearRect 探针。
    if device.device_capabilities().clear_rect {
        // 以完整 1x1 区域验证清理颜色和 scissor 代际。
        device.clear_rect(
            // 使用透明清理色。
            RhiColor::transparent(),
            // 覆盖完整探针目标。
            RhiScissor {
                // 左侧。
                x: 0,
                // 顶部。
                y: 0,
                // 宽度。
                width: 1,
                // 高度。
                height: 1,
            },
        )?;
    }
    // 使用第一个固定 pipeline 执行最小 solid draw。
    device.draw(DrawPacket {
        // 选择 Solid pipeline。
        pipeline: pipelines[0],
        // 复用单位 quad 前三个顶点。
        vertex_buffer,
        // 绑定 Solid uniform。
        uniform_buffer: Some(solid_uniform_buffer),
        // 用封闭非索引范围绘制一个三角形。
        range: DrawRange::vertices(3),
    })?;
    // 使用共享 pipeline 顺序中的 Shape 执行一次真实描边 draw。
    device.draw(DrawPacket {
        // 第五个固定 pipeline 是普通 Shape。
        pipeline: pipelines[4],
        // Shape 使用完整单位 quad。
        vertex_buffer,
        // 绑定共享 Shape ABI uniform。
        uniform_buffer: Some(shape_uniform_buffer),
        // 用封闭非索引范围绘制两个三角形组成的 quad。
        range: DrawRange::vertices(6),
    })?;
    // 关闭 probe pass，防止资源销毁跨过打开的 render pass。
    device.end_render_pass()?;
    // 提交 probe 命令，验证 adapter 的提交边界和状态清理。
    device.submit()?;
    // 释放 probe 创建的固定 pipeline，资源生命周期仍由 adapter 检查。
    for pipeline in pipelines {
        // 逐个销毁 adapter pipeline。
        device.destroy_pipeline(pipeline)?;
    }
    // 释放 probe 创建的 sampler。
    device.destroy_sampler(sampler)?;
    // 释放 probe 创建的 coverage texture。
    device.destroy_texture(coverage_texture)?;
    // 释放 probe 创建的 region texture。
    device.destroy_texture(region_texture)?;
    // 释放 probe 创建的 BGRA texture。
    device.destroy_texture(bgra_texture)?;
    // 释放 probe 创建的 RGBA texture。
    device.destroy_texture(rgba_texture)?;
    // 释放 probe 创建的共用顶点 buffer。
    device.destroy_buffer(vertex_buffer)?;
    // 释放 probe 创建的 Solid uniform buffer。
    device.destroy_buffer(solid_uniform_buffer)?;
    // 释放 probe 创建的 Shape uniform buffer。
    device.destroy_buffer(shape_uniform_buffer)?;
    // 所有固定资源、Shape shader ABI 和真实命令均通过首帧前探针。
    Ok(())
}
