//! D3D11 薄 RHI 的固定 draw ABI 分派。

// 引入统一结果类型。
use crate::core::error::Result;
// 引入 draw packet 与有限资源语义。
use crate::platform::presentation::rhi::{
    BufferUsage, DrawPacket, PipelineKind, SampledTextureBinding,
};

// 引入父模块的 D3D11 context、资源状态和错误辅助。
use super::{D3d11Context, D3d11IndexBinding, rhi_invalid};

// 从当前 DrawPacket 取得已验证且不与活动输出冲突的采样资源。
fn packet_sampled_binding(
    // 只读借用当前 D3D11 owner 与共享 pass 状态。
    context: &D3d11Context,
    // 接收已经冻结全部资源角色的 DrawPacket。
    packet: DrawPacket,
) -> Result<SampledTextureBinding> {
    // sampled pipeline 必须由同一个 packet 携带完整纹理与 sampler。
    let binding = packet
        // 只读取得封闭条件采样角色。
        .sampling()
        // 只读投影完整采样资源事实。
        .sampled_texture()
        // 防御直接 Device 调用绕过 FramePlan 结构门禁。
        .ok_or_else(|| rhi_invalid("D3d11 RHI sampled draw binding is missing"))?;
    // 共享 pass 只验证当前输出与 packet 输入不会形成反馈环。
    context
        .rhi_device
        .pass
        .validate_sampled_texture(binding.texture())?;
    // 最终 Adapter 防御复用真实资源描述与绑定采样语义门禁。
    context.rhi_validate_sampled_resources(binding)?;
    // 返回仍保持纹理、sampler 与采样语义原子的绑定值。
    Ok(binding)
}

// 为 D3D11 context 编码通用 draw packet。
impl D3d11Context {
    // 执行一个已经选择固定 pipeline 与资源句柄的通用 draw packet。
    pub(super) fn draw_rhi_packet(&mut self, packet: DrawPacket) -> Result<()> {
        // draw 必须发生在显式 render pass 内。
        self.rhi_device.pass.require_open()?;
        // 直接 Device 调用也不得用历史原生状态补齐残缺采样角色。
        if !packet.has_valid_sampling() {
            // 使用稳定参数错误拒绝缺失、多余或语义错配的条件绑定。
            return Err(rhi_invalid("D3d11 RHI draw sampling binding is invalid"));
        }
        // 先由共享 pipeline 表验证句柄仍存活且 kind 与资源一致。
        self.rhi_device.pipeline(packet.pipeline())?;
        // 读取 FramePlan 绑定的共享 pipeline 语义。
        let pipeline_kind = packet.pipeline().kind();
        // 一次取得不可拆的顶点与 Uniform 资源身份。
        let buffers = packet.buffers();
        // 解析顶点 buffer 并保留通用 ABI 所需的步长。
        let vertex = self.rhi_device.buffer(buffers.vertex())?;
        // 拒绝错误用途的顶点资源。
        if vertex.desc.usage() != BufferUsage::Vertex {
            // 返回稳定的资源类型错误。
            return Err(rhi_invalid("D3d11 RHI draw vertex buffer has wrong usage"));
        }
        // 复制顶点 COM 对象，结束资源表借用。
        let vertex_native = vertex.native.clone();
        // 复制通用顶点步长。
        let vertex_stride = vertex.desc.stride_bytes();
        // 解析 packet 已由 typed bindings 保证存在的 uniform buffer。
        let uniform_handle = buffers.uniform();
        // 读取 uniform 资源事实。
        let uniform = self.rhi_device.buffer(uniform_handle)?;
        // 拒绝错误用途的 uniform 资源。
        if uniform.desc.usage() != BufferUsage::Uniform {
            // 返回稳定的资源类型错误。
            return Err(rhi_invalid("D3d11 RHI draw uniform buffer has wrong usage"));
        }
        // 复制 uniform COM 对象，结束资源表借用。
        let uniform_native = uniform.native.clone();
        // 复制 uniform 总字节数供共享 pipeline 契约统一校验。
        let uniform_size = uniform.desc.size_bytes();
        // 复制 FramePlan 已经封闭为顶点或索引变体的绘制范围。
        let range = packet.range();
        // 从唯一共享契约读取当前 pipeline 的资源与混合语义。
        let contract = packet.pipeline().contract();
        // Drawing 生产端与 D3D11 消费端必须严格使用同一个 ABI。
        if vertex_stride != contract.vertex.stride_bytes()
            || uniform_size != contract.uniform.size_bytes()
            || !range.is_valid()
        {
            // 使用统一门禁拒绝任何 pipeline 的漂移载荷。
            return Err(rhi_invalid("D3d11 RHI pipeline ABI is invalid"));
        }
        // 索引 buffer 如存在必须与 FramePlan 绑定的共享格式一致。
        let index_binding = if let Some(binding) = range.index_binding() {
            // 解析绑定中保存的不透明索引资源。
            let index = self.rhi_device.buffer(binding.buffer())?;
            // 读取后续步长与原生格式映射的唯一共享格式。
            let format = binding.format();
            // 校验资源用途和创建步长精确符合共享格式。
            if index.desc.usage() != BufferUsage::Index
                || index.desc.stride_bytes() != format.stride_bytes()
            {
                // 返回稳定且不泄漏 DXGI 枚举的格式错误。
                return Err(rhi_invalid("D3d11 RHI draw index buffer format is invalid"));
            }
            // 一次完成共享格式到 DXGI 格式的封闭映射。
            Some(D3d11IndexBinding::new(index.native.clone(), format))
        } else {
            // 非索引 packet 不绑定 index buffer。
            None
        };
        // 统一投影两个原生命令需要的互斥数量与起点。
        let vertex_count = range.vertex_count();
        // 索引变体只在该投影中返回非零索引数量。
        let index_count = range.index_count();
        // 非索引变体只在该投影中返回首顶点。
        let first_vertex = range.first_vertex();
        // 索引变体只在该投影中返回首索引。
        let first_index = range.first_index();
        // 每次 Draw 都必须从自身 packet 覆盖完整动态栅格状态。
        self.rhi_apply_draw_raster(packet.raster())?;
        // 在 shader helper 分派前统一绑定共享二维光栅与深度模板状态。
        self.pipeline.apply_rhi_fixed_state(
            // 使用当前 owner-thread immediate context。
            &self.context,
            // 传入 FramePlan pipeline 的共享原语拓扑。
            contract.topology,
            // 传入 FramePlan pipeline 的共享采样覆盖状态。
            contract.multisample,
            // 传入 FramePlan pipeline 的共享颜色抖动状态。
            contract.dither,
            // 传入 FramePlan pipeline 的共享光栅状态。
            contract.raster,
            // 传入 FramePlan pipeline 的共享深度模板状态。
            contract.depth_stencil,
        )?;
        // 按封闭 pipeline 语义选择已经验证过的 D3D11 shader ABI。
        match pipeline_kind {
            // 实心 mesh 使用 32 字节 MeshConstants（viewport、padding、颜色）。
            PipelineKind::SolidMesh => {
                // 把已解析的低层对象交给 D3D11 pipeline 编码 draw。
                self.pipeline.draw_rhi_solid_mesh(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_binding.as_ref(),
                    &uniform_native,
                    contract.blend,
                    vertex_count,
                    index_count,
                    first_vertex,
                    first_index,
                )
            }
            // 采样 quad 使用 16 字节 viewport uniform 和 t0/s0 绑定。
            PipelineKind::TexturedQuad | PipelineKind::TexturedQuadAdditive => {
                // 只从当前 packet 取得完整采样资源并验证目标关系。
                let binding = packet_sampled_binding(self, packet)?;
                // 解析 SRV 并复制 COM 句柄，结束资源表借用。
                let texture = self.rhi_device.texture(binding.texture())?;
                // 解析已经在 bind 边界验证的 sampler 原生状态。
                let sampler = self.rhi_device.sampler(binding.sampler())?;
                // 复制 sampled texture view。
                let texture_srv = texture.srv.clone();
                // 复制 sampler state。
                let sampler_native = sampler.native.clone();
                // 把已解析的低层对象交给 D3D11 pipeline 编码 draw。
                self.pipeline.draw_rhi_textured_quad(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_binding.as_ref(),
                    &uniform_native,
                    &texture_srv,
                    &sampler_native,
                    contract.blend,
                    vertex_count,
                    index_count,
                    first_vertex,
                    first_index,
                )
            }
            // R8 字形覆盖率 quad 复用 16 字节 viewport uniform 和 t0/s0 绑定。
            PipelineKind::GlyphCoverageQuad => {
                // 只从当前 packet 取得完整采样资源并验证目标关系。
                let binding = packet_sampled_binding(self, packet)?;
                // 解析已经在 bind 边界验证为 R8 的 coverage 纹理。
                let texture = self.rhi_device.texture(binding.texture())?;
                // 解析已经在 bind 边界验证的最近点 sampler 原生状态。
                let sampler = self.rhi_device.sampler(binding.sampler())?;
                // 复制 R8 SRV，结束资源表借用。
                let texture_srv = texture.srv.clone();
                // 复制 sampler state。
                let sampler_native = sampler.native.clone();
                // 把已解析的低层对象交给 glyph coverage shader 编码 draw。
                self.pipeline.draw_rhi_coverage_quad(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_binding.as_ref(),
                    &uniform_native,
                    &texture_srv,
                    &sampler_native,
                    contract.blend,
                    vertex_count,
                    index_count,
                    first_vertex,
                    first_index,
                )
            }
            // RGBA8 MSDF 字形 quad 使用 32 字节 viewport/extent/range uniform。
            PipelineKind::MsdfGlyphQuad => {
                // 只从当前 packet 取得完整采样资源并验证目标关系。
                let binding = packet_sampled_binding(self, packet)?;
                // 解析已经在 bind 边界验证为 RGBA8 的 MSDF 距离纹理。
                let texture = self.rhi_device.texture(binding.texture())?;
                // 解析已经在 bind 边界验证的线性 sampler 原生状态。
                let sampler = self.rhi_device.sampler(binding.sampler())?;
                // 复制 RGBA8 SRV，结束资源表借用。
                let texture_srv = texture.srv.clone();
                // 复制 sampler state。
                let sampler_native = sampler.native.clone();
                // 把已解析的低层对象交给 D3D11 MSDF shader 编码 draw。
                self.pipeline.draw_rhi_msdf_quad(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_binding.as_ref(),
                    &uniform_native,
                    &texture_srv,
                    &sampler_native,
                    contract.blend,
                    vertex_count,
                    index_count,
                    first_vertex,
                    first_index,
                )
            }
            // 圆角/描边矩形使用包含共享 draw rect 的固定 ShapeConstants。
            PipelineKind::ShapeRect | PipelineKind::ShapeRectAdditive => {
                // 把已经由通用 renderer 验证的常量交给矩形 SDF shader。
                self.pipeline.draw_rhi_shape_rect(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_binding.as_ref(),
                    &uniform_native,
                    contract.blend,
                    vertex_count,
                    index_count,
                    first_vertex,
                    first_index,
                )
            }
            // 原生扇形使用 64 字节 SectorConstants 和 position float2 quad。
            PipelineKind::Sector => {
                // 把固定扇形 ABI 交给 D3D11 sector shader 编码。
                self.pipeline.draw_rhi_sector(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_binding.as_ref(),
                    &uniform_native,
                    contract.blend,
                    vertex_count,
                    index_count,
                    first_vertex,
                    first_index,
                )
            }
            // 解析线段使用共享 64 字节常量 ABI 和胶囊距离场。
            PipelineKind::LineSegment => self.pipeline.draw_rhi_line(
                &self.context,
                &vertex_native,
                vertex_stride,
                index_binding.as_ref(),
                &uniform_native,
                contract.blend,
                vertex_count,
                index_count,
                first_vertex,
                first_index,
            ),
            // 仿射阴影使用 96 字节 AffineShadowConstants。
            PipelineKind::BoxShadow => {
                // 把已经由通用 renderer 验证的常量交给阴影 SDF shader。
                self.pipeline.draw_rhi_shadow(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_binding.as_ref(),
                    &uniform_native,
                    contract.blend,
                    vertex_count,
                    index_count,
                    first_vertex,
                    first_index,
                )
            }
            // 线性/径向渐变使用 96 字节 affine GradientConstants。
            PipelineKind::GradientRect => {
                // 把已经由通用 renderer 验证的常量交给渐变 shader。
                self.pipeline.draw_rhi_gradient_rect(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_binding.as_ref(),
                    &uniform_native,
                    contract.blend,
                    vertex_count,
                    index_count,
                    first_vertex,
                    first_index,
                )
            }
            // 单方向 blur 使用 304 字节 BlurConstants 和 float2 区域 quad。
            PipelineKind::BlurPass => {
                // 只从当前 packet 取得完整采样资源并验证目标关系。
                let binding = packet_sampled_binding(self, packet)?;
                // 解析已经在 bind 边界验证的 Blur 颜色源纹理。
                let texture = self.rhi_device.texture(binding.texture())?;
                // 解析已经在 bind 边界验证的线性 sampler 原生状态。
                let sampler = self.rhi_device.sampler(binding.sampler())?;
                // 复制 source SRV，结束资源表借用。
                let texture_srv = texture.srv.clone();
                // 复制 sampler state。
                let sampler_native = sampler.native.clone();
                // 复制当前 pass 已绑定的 RTV。
                let target = self
                    .rhi_device
                    .active_target
                    .as_ref()
                    .cloned()
                    .ok_or_else(|| rhi_invalid("D3d11 RHI blur draw has no target"))?;
                // 把已经完成 region lowering 的对象交给 D3D11 blur ABI。
                self.pipeline.draw_rhi_blur_pass(
                    &self.context,
                    &vertex_native,
                    vertex_stride,
                    index_binding.as_ref(),
                    &uniform_native,
                    &texture_srv,
                    &sampler_native,
                    &target,
                    contract.blend,
                    vertex_count,
                    index_count,
                    first_vertex,
                    first_index,
                )
            }
        }
    }
}
