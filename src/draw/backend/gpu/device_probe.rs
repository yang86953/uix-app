//! Drawing GPU Module 对共享 FramePlan、固定 pipeline 与 Shape ABI 的启动探针。

// 引入统一结果类型。
use crate::core::{Error, Result};
// 引入 Drawing 唯一拥有的类型化 FramePlan 及其离屏 pass 命令。
use crate::draw::backend::frame_plan::{
    FramePlan, FramePlanCommand, FrameUniformPayload, FrameVertexPayload, RenderPassPlan,
    RenderTargetRef,
};
// 引入探针创建、绘制和销毁所需的共享薄 RHI 类型。
use crate::platform::presentation::rhi::{
    BufferDesc, BufferHandle, DrawBufferBindings, DrawPacket, DrawRange, DrawRasterState,
    DrawSamplingBinding, GraphicsDevice, LoadAction, PipelineBinding, PipelineDesc, PipelineKind,
    RhiColor, RhiExtent, RhiMeshRasterParams, RhiScissor, RhiShapeRasterParams, RhiTextureRegion,
    RhiTextureTransfer, RhiTextureUpload, RhiViewport, SamplerDesc, SamplerHandle, TextureDesc,
    TextureFormat, TextureHandle, TextureMove,
};

// 按创建顺序保存探针资源，并封闭检查式清理生命周期。
struct ProbeScope {
    // 保存已经创建的 buffer。
    buffers: Vec<BufferHandle>,
    // 保存已经创建的 texture。
    textures: Vec<TextureHandle>,
    // 保存已经创建的 sampler。
    samplers: Vec<SamplerHandle>,
    // 保存已经创建的 pipeline。
    pipelines: Vec<PipelineBinding>,
}

// 为探针提供资源登记和全局逆序清理。
impl ProbeScope {
    // 创建一个尚未持有资源的作用域。
    fn new() -> Self {
        // 返回空的资源登记表。
        Self {
            // 初始没有 buffer。
            buffers: Vec::new(),
            // 初始没有 texture。
            textures: Vec::new(),
            // 初始没有 sampler。
            samplers: Vec::new(),
            // 初始没有 pipeline。
            pipelines: Vec::new(),
        }
    }

    // 记录一个已经成功创建的 buffer 并返回其句柄。
    fn track_buffer(&mut self, buffer: BufferHandle) -> BufferHandle {
        // 按创建顺序登记资源。
        self.buffers.push(buffer);
        // 返回句柄供后续探针命令使用。
        buffer
    }

    // 记录一个已经成功创建的 texture 并返回其句柄。
    fn track_texture(&mut self, texture: TextureHandle) -> TextureHandle {
        // 按创建顺序登记资源。
        self.textures.push(texture);
        // 返回句柄供后续探针命令使用。
        texture
    }

    // 记录一个已经成功创建的 sampler 并返回其句柄。
    fn track_sampler(&mut self, sampler: SamplerHandle) -> SamplerHandle {
        // 按创建顺序登记资源。
        self.samplers.push(sampler);
        // 返回句柄供后续探针命令使用。
        sampler
    }

    // 记录一个已经成功创建的 pipeline 并返回其绑定身份。
    fn track_pipeline(&mut self, pipeline: PipelineBinding) -> PipelineBinding {
        // 按创建顺序登记资源。
        self.pipelines.push(pipeline);
        // 返回绑定身份供后续探针命令使用。
        pipeline
    }

    // 按资源全局创建逆序持续销毁并保留首错。
    fn cleanup<D: GraphicsDevice + ?Sized>(&mut self, device: &mut D) -> Result<()> {
        // 记录第一个清理失败，后续失败不能阻断剩余资源销毁。
        let mut first_error: Option<Error> = None;
        // 按 pipeline 创建逆序销毁全部 pipeline。
        while let Some(pipeline) = self.pipelines.pop() {
            // 记录 pipeline 销毁失败但继续清理。
            if let Err(error) = device.destroy_pipeline(pipeline) {
                // 只保留最早的清理错误。
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        // 按 sampler 创建逆序销毁全部 sampler。
        while let Some(sampler) = self.samplers.pop() {
            // 记录 sampler 销毁失败但继续清理。
            if let Err(error) = device.destroy_sampler(sampler) {
                // 只保留最早的清理错误。
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        // 按 texture 创建逆序销毁全部 texture。
        while let Some(texture) = self.textures.pop() {
            // 记录 texture 销毁失败但继续清理。
            if let Err(error) = device.destroy_texture(texture) {
                // 只保留最早的清理错误。
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        // 按 buffer 创建逆序销毁全部 buffer。
        while let Some(buffer) = self.buffers.pop() {
            // 记录 buffer 销毁失败但继续清理。
            if let Err(error) = device.destroy_buffer(buffer) {
                // 只保留最早的清理错误。
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        // 返回首个清理错误或成功。
        first_error.map_or(Ok(()), Err)
    }
}

// 在 bootstrap 阶段执行最小资源、固定 pipeline、Solid 和 Shape 命令探针。
pub(super) fn probe_device<D: GraphicsDevice + ?Sized>(device: &mut D) -> Result<()> {
    // 资源创建前先确认已经激活的 Device 仍保持健康。
    GraphicsDevice::maintain(device)?;
    // 创建共享作用域，使主体任意失败都进入统一清理路径。
    let mut scope = ProbeScope::new();
    // 冻结本次探针使用的 Device 能力，禁止资源创建期间改变可选命令集合。
    let capabilities = device.device_capabilities();
    // 执行探针主体，保留原始主错误直到清理完成。
    let probe_result = (|| -> Result<()> {
        // Shape 等解析图元共用的 position-float2 单位 quad。
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
        let shape_vertex_payload = FrameVertexPayload::position_f32x2(unit_vertices);
        // Solid 探针使用独立的 position + coverage 最小三角形。
        let solid_vertex_payload = FrameVertexPayload::position_coverage_f32([
            0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0,
        ]);
        let solid_vertex_buffer = scope.track_buffer(device.create_buffer(BufferDesc::vertex(
            // 字节容量只从类型化顶点载荷派生。
            solid_vertex_payload.size_bytes(),
            // stride 只从同一载荷声明的共享布局派生。
            solid_vertex_payload.layout().stride_bytes(),
        ))?);
        // Shape 保留 position-float2 单位 quad 的独立资源。
        let shape_vertex_buffer = scope.track_buffer(device.create_buffer(BufferDesc::vertex(
            shape_vertex_payload.size_bytes(),
            shape_vertex_payload.layout().stride_bytes(),
        ))?);
        // 创建 Solid mesh 所需的 32 字节 uniform ABI。
        let solid_uniform_buffer =
            scope.track_buffer(device.create_buffer(BufferDesc::uniform(
                // MeshConstants 固定占三十二字节。
                PipelineKind::SolidMesh.contract().uniform.size_bytes(),
            ))?);
        // 构造 1x1 viewport、padding 和透明颜色的共享 Solid 常量。
        let solid_params = RhiMeshRasterParams::new(
            // 使用最小 render target 的物理视口。
            RhiViewport {
                // 视口宽度。
                width: 1.0,
                // 视口高度。
                height: 1.0,
            },
            // 使用透明颜色填充 Solid 探针。
            [0.0; 4],
        );
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
        // 创建 Shape 固定 ABI uniform buffer。
        let shape_uniform_buffer =
            scope.track_buffer(device.create_buffer(BufferDesc::uniform(
                // 使用共享常量，禁止 probe 与 renderer 产生布局分叉。
                PipelineKind::ShapeRect.contract().uniform.size_bytes(),
            ))?);
        // 使用最小的 RGBA texture 验证 render target 颜色格式。
        let rgba_texture = scope.track_texture(device.create_texture(TextureDesc::new(
            // 使用一像素离屏目标。
            RhiExtent::new(1, 1),
            // 使用通用 RGBA8 格式。
            TextureFormat::Rgba8Unorm,
        ))?);
        // 使用最小的 BGRA texture 验证主 surface/Picture 采样格式。
        let bgra_texture = scope.track_texture(device.create_texture(TextureDesc::new(
            // 使用一像素采样目标。
            RhiExtent::new(1, 1),
            // 使用通用 BGRA8 格式。
            TextureFormat::Bgra8Unorm,
        ))?);
        // 使用最小的 R8 texture 验证 coverage 采样格式。
        let coverage_texture = scope.track_texture(device.create_texture(TextureDesc::new(
            // 使用一像素 coverage 目标。
            RhiExtent::new(1, 1),
            // 使用通用 R8 格式。
            TextureFormat::R8Unorm,
        ))?);
        // 上传 RGBA 纹理的一个 premultiplied 像素。
        device.update_texture(RhiTextureUpload::full(
            // 绑定 RGBA 探针纹理。
            rgba_texture,
            // 上传范围覆盖完整一像素资源。
            RhiExtent::new(1, 1),
            // 使用透明 premultiplied 像素。
            &[0, 0, 0, 0],
        ))?;
        // 上传 BGRA 纹理的一个 premultiplied 像素。
        device.update_texture(RhiTextureUpload::full(
            // 绑定 BGRA 探针纹理。
            bgra_texture,
            // 上传范围覆盖完整一像素资源。
            RhiExtent::new(1, 1),
            // 使用透明 premultiplied 像素。
            &[0, 0, 0, 0],
        ))?;
        // 上传 coverage 纹理的一个覆盖率像素。
        device.update_texture(RhiTextureUpload::full(
            // 绑定 coverage 探针纹理。
            coverage_texture,
            // 上传范围覆盖完整一像素资源。
            RhiExtent::new(1, 1),
            // 使用零覆盖率像素。
            &[0],
        ))?;
        // 创建 2x2 RGBA texture，验证 atlas 所需的带偏移子区域上传。
        let region_texture = scope.track_texture(device.create_texture(TextureDesc::new(
            // 使用可容纳右下角子区域的两像素目标。
            RhiExtent::new(2, 2),
            // 使用通用 RGBA8 格式。
            TextureFormat::Rgba8Unorm,
        ))?);
        // 把一个像素写入右下角，避免零偏移整块上传冒充 region 能力。
        device.update_texture(RhiTextureUpload::new(
            // 更新探针纹理。
            region_texture,
            // 把右下偏移与单位尺寸封闭为一个区域。
            RhiTextureRegion::from_xy(1, 1, RhiExtent::new(1, 1)),
            // 上传单个透明像素。
            &[0, 0, 0, 0],
        ))?;
        // 只为声明区域移动能力的 Adapter 构造 retained framebuffer 探针步骤。
        let texture_move = capabilities.texture_region_move.then(|| {
            // 以同一纹理的重叠区域移动验证 memmove 语义。
            TextureMove::new(
                // 从 region texture 读取。
                region_texture,
                // 写回同一个 region texture。
                region_texture,
                // 将两个原点与单位尺寸封闭为共享传输。
                RhiTextureTransfer::from_xy(1, 1, 0, 0, RhiExtent::new(1, 1)),
            )
        });
        // 登记真实 sampler，验证纹理绑定所需的过滤状态。
        scope.track_sampler(device.create_sampler(
            // 使用命名的线性 clamp 语义覆盖生产采样状态。
            SamplerDesc::linear_clamp(),
        )?);
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
            // 解析抗锯齿线段。
            PipelineKind::LineSegment,
        ];
        // 逐个创建 pipeline，让 adapter 在首帧前暴露 shader/layout 失败。
        let mut pipelines = Vec::with_capacity(pipeline_kinds.len());
        // 保留固定顺序以便随后选择 Solid 与 Shape pipeline。
        for kind in pipeline_kinds {
            // 创建当前固定 pipeline。
            pipelines.push(scope.track_pipeline(device.create_pipeline(PipelineDesc { kind })?));
        }
        // 创建只允许 Device 命令与唯一 submit 的离屏计划。
        let mut plan = FramePlan::offscreen();
        // 可选区域移动必须和后续 probe pass 共享同一次提交边界。
        if let Some(movement) = texture_move {
            // 将重叠安全移动纳入 FramePlan 的统一预检与执行顺序。
            plan.push_move(movement);
        }
        // 创建 1x1 离屏颜色 target 的真实 probe pass。
        let mut pass = RenderPassPlan::new(
            // 目标只能是显式 probe texture，不能取得 Surface image。
            RenderTargetRef::Texture(rgba_texture),
            // 以透明色清空目标。
            LoadAction::Clear(RhiColor::transparent()),
        );
        // 定义每个 Probe DrawPacket 共同使用的最小正 viewport。
        let viewport = RhiViewport {
            // 视口宽度。
            width: 1.0,
            // 视口高度。
            height: 1.0,
        };
        // 只有声明局部清理能力的 Adapter 才进入 ClearRect 探针。
        if capabilities.clear_rect {
            // 以完整 1x1 区域验证清理颜色和 scissor 代际。
            pass.push(FramePlanCommand::ClearRect {
                // 使用透明 premultiplied-alpha 清理色。
                color: RhiColor::transparent(),
                // 覆盖完整探针目标。
                scissor: RhiScissor {
                    // 左侧。
                    x: 0,
                    // 顶部。
                    y: 0,
                    // 宽度。
                    width: 1,
                    // 高度。
                    height: 1,
                },
            });
        }
        // 在首个 draw 前建立 Solid coverage 顶点内容事实。
        pass.push(FramePlanCommand::UploadVertex {
            buffer: solid_vertex_buffer,
            data: solid_vertex_payload,
        });
        // 在 Solid draw 前交付完整 Mesh uniform 值对象。
        pass.push(FramePlanCommand::UploadUniform {
            // 绑定刚创建的 Solid uniform 资源。
            buffer: solid_uniform_buffer,
            // 复用 Drawing 与两个 Adapter 共享的 Mesh ABI。
            data: FrameUniformPayload::Mesh(solid_params),
        });
        // 使用第一个固定 pipeline 执行最小 solid draw。
        pass.push(FramePlanCommand::Draw(DrawPacket::new(
            // 选择 Solid pipeline。
            pipelines[0],
            // 绑定完整 coverage 三角形与 Solid uniform。
            DrawBufferBindings::new(solid_vertex_buffer, solid_uniform_buffer),
            // Solid probe draw 不使用采样资源。
            DrawSamplingBinding::none(),
            // Probe draw 明确携带完整 viewport 与无 scissor 栅格事实。
            DrawRasterState::new(viewport, None),
            // 用封闭非索引范围绘制一个三角形。
            DrawRange::vertices(3),
        )));
        // Shape draw 前建立独立 position-float2 单位 quad 内容事实。
        pass.push(FramePlanCommand::UploadVertex {
            buffer: shape_vertex_buffer,
            data: shape_vertex_payload,
        });
        // 在 Shape draw 前交付完整 Shape uniform 值对象。
        pass.push(FramePlanCommand::UploadUniform {
            // 绑定 Shape 专用 uniform 资源。
            buffer: shape_uniform_buffer,
            // 复用 Drawing 与两个 Adapter 共享的 Shape ABI。
            data: FrameUniformPayload::Shape(shape_params),
        });
        // 使用共享 pipeline 顺序中的 Shape 执行一次真实描边 draw。
        pass.push(FramePlanCommand::Draw(DrawPacket::new(
            // 第五个固定 pipeline 是普通 Shape。
            pipelines[4],
            // Shape 使用完整单位 quad 并绑定共享 Shape ABI uniform。
            DrawBufferBindings::new(shape_vertex_buffer, shape_uniform_buffer),
            // Shape probe draw 不使用采样资源。
            DrawSamplingBinding::none(),
            // Probe draw 明确携带完整 viewport 与无 scissor 栅格事实。
            DrawRasterState::new(viewport, None),
            // 用封闭非索引范围绘制两个三角形组成的 quad。
            DrawRange::vertices(6),
        )));
        // 把完整 probe pass 追加到唯一离屏计划。
        plan.push_pass(pass);
        // 由 FramePlan 统一预检、激活、执行、收尾和提交，禁止第二套命令路径。
        plan.execute_offscreen_on_device(device)?;
        // 所有固定资源、Shape shader ABI 和真实命令均通过首帧前探针。
        Ok(())
    })();
    // 主体完成后无论成功失败都执行统一清理。
    let cleanup_result = scope.cleanup(device);
    // 主体失败时保留原始错误，并记录清理失败的附加诊断。
    if let Err(main_error) = probe_result {
        // 清理失败不能覆盖导致探针失败的主错误。
        if let Err(cleanup_error) = cleanup_result {
            // 主错误与清理错误分别经边界观察入口记录，便于定位双重故障。
            crate::diagnostics::observe_boundary_error("gpu_probe", &main_error);
            crate::diagnostics::observe_boundary_error("gpu_probe/cleanup", &cleanup_error);
        }
        // 返回原始主错误。
        return Err(main_error);
    }
    // 主体成功时清理失败必须成为探针结果。
    cleanup_result
}
