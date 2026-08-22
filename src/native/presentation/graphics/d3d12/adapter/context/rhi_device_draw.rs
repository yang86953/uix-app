//! D3D12 薄 RHI 的 DrawPacket 资源预检、原生绑定与命令编码。

use super::*;

use crate::platform::presentation::rhi::{
    BufferUsage, DrawRange, IndexFormat, PipelinePrimitiveTopology, SampledTextureBinding,
};
use ::windows::Win32::Foundation::RECT;
use ::windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use ::windows::Win32::Graphics::Direct3D12::{
    D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, D3D12_DESCRIPTOR_HEAP_TYPE,
    D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
    D3D12_GPU_DESCRIPTOR_HANDLE, D3D12_INDEX_BUFFER_VIEW, D3D12_RESOURCE_DIMENSION_BUFFER,
    D3D12_RESOURCE_DIMENSION_TEXTURE2D, D3D12_TEXTURE_LAYOUT_ROW_MAJOR, D3D12_VERTEX_BUFFER_VIEW,
    D3D12_VIEWPORT, ID3D12DescriptorHeap, ID3D12GraphicsCommandList, ID3D12Resource,
};
use ::windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R32_UINT, DXGI_FORMAT_UNKNOWN};

// 保存采样 draw 在命令列表与 GPU 完成前必须保持存活的纹理和两个 descriptor heap。
#[allow(dead_code)]
struct D3d12SampledDrawResources {
    // shader-visible sampler heap 同时拥有 s0 descriptor 的存活期。
    sampler_heap: ID3D12DescriptorHeap,
    // shader-visible SRV heap 同时拥有 t0 descriptor 的存活期。
    srv_heap: ID3D12DescriptorHeap,
    // SRV descriptor 引用的真实纹理最后释放。
    texture: ID3D12Resource,
    // 保存全部只读门禁通过后取得的 t0 GPU descriptor handle。
    srv_gpu: D3D12_GPU_DESCRIPTOR_HANDLE,
    // 保存全部只读门禁通过后取得的 s0 GPU descriptor handle。
    sampler_gpu: D3D12_GPU_DESCRIPTOR_HANDLE,
}

// 保存一条已编码 draw 从录制到 fence 成功期间引用的全部原生对象。
#[allow(dead_code)]
pub(super) struct D3d12RhiDrawResources {
    // 采样 draw 额外保持纹理、SRV heap 与 sampler heap 存活。
    sampled: Option<D3d12SampledDrawResources>,
    // 索引 draw 额外保持索引 buffer 的当前上传版本存活。
    index: Option<ID3D12Resource>,
    // 保持 Uniform buffer 的当前不可变上传版本存活。
    uniform: ID3D12Resource,
    // 保持顶点 buffer 的当前不可变上传版本存活。
    vertex: ID3D12Resource,
    // PSO 与 Root Signature 在其它 draw 资源之后释放。
    pipeline: D3d12PipelineNativeBinding,
}

impl D3d12RhiDrawResources {
    // 未知 GPU 状态下泄漏整组引用，禁止 Drop 提前释放命令列表仍可能访问的对象。
    pub(super) fn retain_after_undrained_drop(self) {
        std::mem::forget(self);
    }
}

// 保存所有共享与原生只读门禁完成后才能进入 Set*/Draw* 的不可变命令计划。
struct D3d12RhiDrawPlan {
    // 原生对象引用在命令编码后移交给资源 owner 的在途队列。
    resources: D3d12RhiDrawResources,
    // 顶点视图完全来自 DrawPacket 绑定的真实 buffer 描述。
    vertex_view: D3D12_VERTEX_BUFFER_VIEW,
    // 索引视图只由 DrawRange::Indices 创建。
    index_view: Option<D3D12_INDEX_BUFFER_VIEW>,
    // b0 直接绑定当前 Uniform buffer 版本的 GPU 虚拟地址。
    uniform_address: u64,
    // topology 只从共享 PipelineContract 投影。
    topology: ::windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY,
    // viewport 只从当前 packet 的 DrawRasterState 投影。
    viewport: D3D12_VIEWPORT,
    // scissor 只从当前 packet 的显式值或完整 viewport 投影。
    scissor: RECT,
    // 绘制数量和起点保留 platform 唯一封闭范围。
    range: DrawRange,
}

impl D3d12RhiDevice {
    // 只读预检真实 pipeline、Buffer 与条件采样资源，不取得原生命令状态。
    pub(super) fn preflight_draw_resources(&self, packet: DrawPacket) -> Result<()> {
        // 共享 pipeline 表先验证句柄仍存活且 kind 与资源一致。
        self.pipelines.get(packet.pipeline())?;
        // 共享 Buffer 表唯一验证角色、stride、ABI 与实际容量。
        self.buffers.validate_draw(packet)?;
        // 条件采样必须复用 SampledTextureBinding 的格式与 sampler 语义门禁。
        if let Some(binding) = packet.sampling().sampled_texture() {
            self.validate_sampled_resources(binding)?;
        }
        Ok(())
    }

    // 复用共享采样绑定验证真实纹理格式与 sampler 描述。
    fn validate_sampled_resources(&self, binding: SampledTextureBinding) -> Result<()> {
        let texture = self.textures.get(binding.texture())?;
        let sampler = self.samplers.get(binding.sampler())?;
        binding.validate_resources(texture.desc.format(), sampler.desc)
    }

    // 在任何命令列表副作用前解析并冻结一条 DrawPacket 的完整原生计划。
    fn stage_draw(&self, packet: DrawPacket, recording: bool) -> Result<D3d12RhiDrawPlan> {
        // draw 必须位于共享状态机已经打开的 pass 内。
        self.pass.require_open()?;
        // 资源表与共享 packet 关系在每次直接 draw 调用内重新验证。
        self.preflight_draw_resources(packet)?;
        // 动态 viewport/scissor 必须完整落在当前共享目标范围内。
        self.pass.validate_draw_raster(packet.raster())?;
        if !recording {
            return Err(draw_state_error(
                "D3D12 RHI draw requires an active command recording",
            ));
        }
        let target = self
            .active_target
            .as_ref()
            .ok_or_else(|| draw_state_error("D3D12 RHI draw has no active native render target"))?;
        if target.target() != self.pass.target()? {
            return Err(draw_state_error(
                "D3D12 RHI draw target does not match the active pass",
            ));
        }

        // 当前目标格式只由 begin_render_pass 冻结的真实 Surface 或 Texture 事实提供。
        let contract = packet.pipeline().contract();
        let pipeline = self
            .pipelines
            .get(packet.pipeline())?
            .native_binding(contract, target.format())?;
        let buffers = packet.buffers();
        let vertex = self.buffers.get(buffers.vertex())?;
        let vertex_address = validate_buffer_native(vertex, BufferUsage::Vertex)?;
        let vertex_view = D3D12_VERTEX_BUFFER_VIEW {
            BufferLocation: vertex_address,
            SizeInBytes: vertex.desc.size_bytes() as u32,
            StrideInBytes: vertex.desc.stride_bytes(),
        };
        let uniform = self.buffers.get(buffers.uniform())?;
        let uniform_address = validate_buffer_native(uniform, BufferUsage::Uniform)?;
        if uniform_address % 256 != 0 {
            return Err(draw_platform_error(
                "D3D12 RHI uniform buffer address is not 256-byte aligned",
            ));
        }

        let range = packet.range();
        let (index_view, index_native) = if let Some(binding) = range.index_binding() {
            let index = self.buffers.get(binding.buffer())?;
            let index_address = validate_buffer_native(index, BufferUsage::Index)?;
            let format = match binding.format() {
                IndexFormat::Uint32 => DXGI_FORMAT_R32_UINT,
            };
            (
                Some(D3D12_INDEX_BUFFER_VIEW {
                    BufferLocation: index_address,
                    SizeInBytes: index.desc.size_bytes() as u32,
                    Format: format,
                }),
                Some(index.native.clone()),
            )
        } else {
            (None, None)
        };

        // 采样状态、反馈环、原生纹理状态与两个 descriptor heap 在 SetDescriptorHeaps 前完成。
        let sampled = packet
            .sampling()
            .sampled_texture()
            .map(|binding| self.stage_sampled_resources(binding))
            .transpose()?;
        if pipeline.sampled_parameters().is_some() != sampled.is_some() {
            return Err(draw_invalid(
                "D3D12 RHI pipeline root parameters do not match draw sampling",
            ));
        }
        let (viewport, scissor) = draw_raster_state(packet)?;
        let topology = match contract.topology {
            PipelinePrimitiveTopology::TriangleList => D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        };
        Ok(D3d12RhiDrawPlan {
            resources: D3d12RhiDrawResources {
                pipeline,
                vertex: vertex.native.clone(),
                uniform: uniform.native.clone(),
                index: index_native,
                sampled,
            },
            vertex_view,
            index_view,
            uniform_address,
            topology,
            viewport,
            scissor,
            range,
        })
    }

    // 验证并冻结 sampled texture、SRV heap 与 sampler heap 的当前原生版本。
    fn stage_sampled_resources(
        &self,
        binding: SampledTextureBinding,
    ) -> Result<D3d12SampledDrawResources> {
        // 共享 pass 唯一拒绝当前输出与采样输入形成反馈环。
        self.pass.validate_sampled_texture(binding.texture())?;
        // 真实描述必须再次满足绑定时冻结的格式与 sampler 语义。
        self.validate_sampled_resources(binding)?;
        let texture = self.textures.get(binding.texture())?;
        let effective = self.effective_sampled_texture_state(binding.texture(), texture.state);
        if effective != D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE {
            return Err(draw_state_error(
                "D3D12 RHI sampled texture is not in pixel-shader-resource state",
            ));
        }
        validate_texture_native(texture)?;
        let sampler = self.samplers.get(binding.sampler())?;
        let srv_gpu = validate_descriptor_heap(
            &texture.srv_heap,
            texture.srv_cpu,
            D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            "D3D12 RHI texture SRV heap is invalid",
        )?;
        let sampler_gpu = validate_descriptor_heap(
            &sampler.heap,
            sampler.cpu,
            D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
            "D3D12 RHI sampler heap is invalid",
        )?;
        Ok(D3d12SampledDrawResources {
            texture: texture.native.clone(),
            srv_heap: texture.srv_heap.clone(),
            sampler_heap: sampler.heap.clone(),
            srv_gpu,
            sampler_gpu,
        })
    }
}

impl D3d12Context {
    // 把已完整预检的 packet 编码为 D3D12 根绑定、IA/RS 状态与单实例 draw。
    pub(super) fn draw_rhi_packet(&mut self, packet: DrawPacket) -> Result<()> {
        let plan = self.rhi_device.stage_draw(packet, self.recording)?;
        record_draw_commands(&self.command_list, &plan);
        // 编码成功后把全部 COM owner 移入批次，只有 submit/fence 成功才释放。
        self.rhi_device.pending_draw_resources.push(plan.resources);
        Ok(())
    }
}

// 验证真实 D3D12 Buffer 仍与共享描述和创建期物理分配一致。
fn validate_buffer_native(resource: &D3d12RhiBuffer, usage: BufferUsage) -> Result<u64> {
    if resource.desc.usage() != usage {
        return Err(draw_invalid("D3D12 RHI draw buffer usage is invalid"));
    }
    // 原生版本与 CPU 完整逻辑镜像必须继续对应同一份共享描述。
    if resource.shadow.len() != resource.desc.size_bytes() {
        return Err(draw_platform_error(
            "D3D12 RHI draw buffer shadow size diverged",
        ));
    }
    // SAFETY: native 由资源表保持存活，GetDesc 与 GetGPUVirtualAddress 均为只读查询。
    let native_desc = unsafe { resource.native.GetDesc() };
    let expected_width = match resource.desc.usage() {
        BufferUsage::Uniform => align_up(resource.desc.size_bytes() as u64, 256),
        BufferUsage::Vertex | BufferUsage::Index => resource.desc.size_bytes() as u64,
    };
    if native_desc.Dimension != D3D12_RESOURCE_DIMENSION_BUFFER
        || native_desc.Width != expected_width
        || native_desc.Format != DXGI_FORMAT_UNKNOWN
        || native_desc.Layout != D3D12_TEXTURE_LAYOUT_ROW_MAJOR
    {
        return Err(draw_platform_error(
            "D3D12 RHI draw buffer native description diverged",
        ));
    }
    // SAFETY: upload-heap buffer 存活且属于当前 device，查询不会改变资源状态。
    let address = unsafe { resource.native.GetGPUVirtualAddress() };
    if address == 0 {
        return Err(draw_platform_error(
            "D3D12 RHI draw buffer returned a null GPU address",
        ));
    }
    Ok(address)
}

// 验证 sampled texture 原生形态仍与唯一共享描述一致。
fn validate_texture_native(texture: &D3d12RhiTexture) -> Result<()> {
    // SAFETY: native 由资源表保持存活，GetDesc 是只读查询。
    let native_desc = unsafe { texture.native.GetDesc() };
    if native_desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D
        || native_desc.Format != texture_format(texture.desc.format())
        || native_desc.Width != u64::from(texture.desc.extent().width)
        || native_desc.Height != texture.desc.extent().height
    {
        return Err(draw_platform_error(
            "D3D12 RHI sampled texture native description diverged",
        ));
    }
    Ok(())
}

// 验证单槽 shader-visible heap 与创建时冻结的 CPU descriptor 身份一致。
fn validate_descriptor_heap(
    heap: &ID3D12DescriptorHeap,
    expected_cpu: D3D12_CPU_DESCRIPTOR_HANDLE,
    expected_type: D3D12_DESCRIPTOR_HEAP_TYPE,
    message: &'static str,
) -> Result<D3D12_GPU_DESCRIPTOR_HANDLE> {
    // SAFETY: heap 由资源 owner 保持存活，三个查询均只读。
    let desc = unsafe { heap.GetDesc() };
    let cpu = unsafe { heap.GetCPUDescriptorHandleForHeapStart() };
    let gpu = unsafe { heap.GetGPUDescriptorHandleForHeapStart() };
    if desc.Type != expected_type
        || desc.NumDescriptors != 1
        || desc.Flags != D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE
        || cpu.ptr != expected_cpu.ptr
        || gpu.ptr == 0
    {
        return Err(draw_platform_error(message));
    }
    Ok(gpu)
}

// 把共享 DrawRasterState 机械投影为左上原点 D3D12 viewport 与 scissor。
fn draw_raster_state(packet: DrawPacket) -> Result<(D3D12_VIEWPORT, RECT)> {
    let raster = packet.raster();
    let (width, height) = raster
        .viewport()
        .native_size_i32()
        .ok_or_else(|| draw_invalid("D3D12 RHI draw viewport cannot be represented natively"))?;
    let scissor = if let Some(scissor) = raster.scissor() {
        let (left, top, right, bottom) = scissor
            .native_rect()
            .ok_or_else(|| draw_invalid("D3D12 RHI draw scissor cannot be represented natively"))?;
        RECT {
            left,
            top,
            right,
            bottom,
        }
    } else {
        RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        }
    };
    Ok((
        D3D12_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: width as f32,
            Height: height as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        },
        scissor,
    ))
}

// 只消费不可变计划编码原生命令；本函数内不得新增任何可失败门禁。
fn record_draw_commands(command_list: &ID3D12GraphicsCommandList, plan: &D3d12RhiDrawPlan) {
    // SAFETY: 计划内全部对象、地址、视图、描述符和范围已完成只读验证并保持存活。
    unsafe {
        if let Some(sampled) = plan.resources.sampled.as_ref() {
            command_list.SetDescriptorHeaps(&[
                Some(sampled.srv_heap.clone()),
                Some(sampled.sampler_heap.clone()),
            ]);
        }
        command_list.SetGraphicsRootSignature(plan.resources.pipeline.root_signature());
        command_list.SetPipelineState(plan.resources.pipeline.pipeline_state());
        command_list.IASetPrimitiveTopology(plan.topology);
        command_list.IASetVertexBuffers(0, Some(std::slice::from_ref(&plan.vertex_view)));
        if let Some(index_view) = plan.index_view.as_ref() {
            command_list.IASetIndexBuffer(Some(index_view));
        } else {
            command_list.IASetIndexBuffer(None);
        }
        command_list.RSSetViewports(std::slice::from_ref(&plan.viewport));
        command_list.RSSetScissorRects(std::slice::from_ref(&plan.scissor));
        command_list.SetGraphicsRootConstantBufferView(
            plan.resources.pipeline.constant_buffer_parameter(),
            plan.uniform_address,
        );
        if let (Some(sampled), Some((srv_parameter, sampler_parameter))) = (
            plan.resources.sampled.as_ref(),
            plan.resources.pipeline.sampled_parameters(),
        ) {
            command_list.SetGraphicsRootDescriptorTable(srv_parameter, sampled.srv_gpu);
            command_list.SetGraphicsRootDescriptorTable(sampler_parameter, sampled.sampler_gpu);
        }
        if plan.range.index_binding().is_some() {
            command_list.DrawIndexedInstanced(
                plan.range.index_count(),
                1,
                plan.range.first_index(),
                0,
                0,
            );
        } else {
            command_list.DrawInstanced(plan.range.vertex_count(), 1, plan.range.first_vertex(), 0);
        }
    }
}

// 构造稳定的 D3D12 draw 参数错误。
fn draw_invalid(message: &'static str) -> Error {
    Error::new(Errc::InvalidArgument, message)
}

// 构造稳定的 D3D12 draw 顺序错误。
fn draw_state_error(message: &'static str) -> Error {
    Error::new(Errc::InvalidState, message)
}

// 构造原生资源形态与 owner 事实不一致的错误。
fn draw_platform_error(message: &'static str) -> Error {
    Error::new(Errc::PlatformError, message)
}
