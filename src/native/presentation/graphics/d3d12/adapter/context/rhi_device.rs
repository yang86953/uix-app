//! D3D12 薄 RHI 的唯一资源 owner 与资源阶段 GraphicsDevice 原语。

use super::*;

use crate::platform::presentation::rhi::{
    BufferDesc, BufferHandle, DrawPacket, GraphicsDevice, GraphicsDeviceCapabilities, LoadAction,
    PipelineBinding, PipelineDesc, RenderTargetHandle, RhiBufferResource, RhiBufferResourceTable,
    RhiBufferUpload, RhiBufferUploadPreflight, RhiColor, RhiResourceTable, RhiScissor,
    RhiTextureResource, RhiTextureResourceTable, RhiTextureUpload, SamplerAddressMode, SamplerDesc,
    SamplerFilter, SamplerHandle, SamplerMipMode, SubmissionHandle, TextureCopy, TextureDesc,
    TextureFormat, TextureHandle, TextureMove, UIX_COLOR_CONTRACT, ValidatedRhiTextureUpload,
};
use ::windows::Win32::Graphics::Direct3D12::{
    D3D12_COMPARISON_FUNC_NEVER, D3D12_CPU_DESCRIPTOR_HANDLE, D3D12_DESCRIPTOR_HEAP_DESC,
    D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
    D3D12_FILTER_MIN_MAG_LINEAR_MIP_POINT, D3D12_FILTER_MIN_MAG_MIP_POINT, D3D12_HEAP_FLAG_NONE,
    D3D12_HEAP_PROPERTIES, D3D12_HEAP_TYPE_DEFAULT, D3D12_HEAP_TYPE_UPLOAD, D3D12_RESOURCE_DESC,
    D3D12_RESOURCE_DIMENSION_BUFFER, D3D12_RESOURCE_DIMENSION_TEXTURE2D,
    D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, D3D12_RESOURCE_FLAG_NONE,
    D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_GENERIC_READ,
    D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATES,
    D3D12_TEXTURE_ADDRESS_MODE_CLAMP, D3D12_TEXTURE_LAYOUT_ROW_MAJOR, D3D12_TEXTURE_LAYOUT_UNKNOWN,
    ID3D12DescriptorHeap, ID3D12Device, ID3D12Resource,
};
use ::windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM,
    DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
};

// 保存一个真实 D3D12 Buffer 与创建时冻结的共享描述。
struct D3d12RhiBuffer {
    // Upload heap 资源可直接作为顶点、索引或常量 Buffer 使用。
    native: ID3D12Resource,
    // 共享描述是所有上传预检的唯一事实。
    desc: BufferDesc,
}

// 让共享 Buffer 表读取 D3D12 创建时冻结的描述。
impl RhiBufferResource for D3d12RhiBuffer {
    fn desc(&self) -> BufferDesc {
        self.desc
    }
}

// 保存一个真实 D3D12 Texture、冻结描述和已提交原生状态。
struct D3d12RhiTexture {
    // Default heap 资源保持纹理的原生生命周期。
    native: ID3D12Resource,
    // 共享描述是上传、copy 预检和 render-target 提升的唯一事实。
    desc: TextureDesc,
    // 只在命令执行并等待成功后提交新的资源状态。
    state: D3D12_RESOURCE_STATES,
}

// 让共享 Texture 表读取 D3D12 创建时冻结的描述。
impl RhiTextureResource for D3d12RhiTexture {
    fn desc(&self) -> TextureDesc {
        self.desc
    }
}

// 保存一个真实 D3D12 sampler descriptor heap 与冻结描述。
struct D3d12RhiSampler {
    // 每个资源阶段 sampler 唯一拥有一个 shader-visible 原生描述符堆。
    heap: ID3D12DescriptorHeap,
    // 保存已经写入原生堆的 CPU descriptor 身份。
    cpu: D3D12_CPU_DESCRIPTOR_HANDLE,
    // 保留共享 sampler 语义，禁止未来绑定路径重新猜测状态。
    desc: SamplerDesc,
}

// 保存共享门禁完成后才能进入 Map/copy 的纹理上传计划。
struct D3d12TextureUploadPlan<'a> {
    // 目标句柄只用于 GPU 成功后的状态提交。
    texture: TextureHandle,
    // 目标原生资源由共享表解析后克隆，上传期间保持存活。
    target: ID3D12Resource,
    // 保存命令开始前已经提交的原生状态。
    before: D3D12_RESOURCE_STATES,
    // 上传堆必须存活到队列完成。
    staging: ID3D12Resource,
    // 保存由原生 footprint 查询得到的 copy 位置。
    footprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
    // 保存共享区域验证后的目标左边界。
    destination_x: u32,
    // 保存共享区域验证后的目标顶边界。
    destination_y: u32,
    // 让计划生命周期显式绑定已经验证的调用方载荷。
    _validated: ValidatedRhiTextureUpload<'a>,
}

// D3D12 context 唯一拥有的资源阶段 Component。
pub(super) struct D3d12RhiDevice {
    // 直接复用 platform 唯一 Buffer 资源表，不复制句柄状态机。
    buffers: RhiBufferResourceTable<D3d12RhiBuffer>,
    // 直接复用 platform 唯一 Texture 资源表和 render-target 门禁。
    textures: RhiTextureResourceTable<D3d12RhiTexture>,
    // 直接复用 platform 唯一通用表签发 sampler 身份。
    samplers: RhiResourceTable<SamplerHandle, D3d12RhiSampler>,
}

impl D3d12RhiDevice {
    // 创建不持有任何原生资源的唯一 owner。
    pub(super) const fn new() -> Self {
        Self {
            buffers: RhiBufferResourceTable::new(),
            textures: RhiTextureResourceTable::new(),
            samplers: RhiResourceTable::new(),
        }
    }

    // 返回只声明本阶段真实资源原语的能力快照。
    fn capabilities(&self) -> GraphicsDeviceCapabilities {
        GraphicsDeviceCapabilities {
            color_contract: UIX_COLOR_CONTRACT,
            dynamic_buffers: true,
            texture_upload: true,
            texture_copy: false,
            texture_region_move: false,
            clear_rect: false,
            sampled_textures: false,
            render_to_texture: false,
            scissor: false,
            premultiplied_alpha_blend: false,
            additive_blend: false,
        }
    }

    // 创建真实 upload-heap Buffer，并由共享表签发不可复用句柄。
    fn create_buffer(&mut self, device: &ID3D12Device, desc: BufferDesc) -> Result<BufferHandle> {
        // 共享容量、用途、步长与 Uniform ABI 必须先于 CreateCommittedResource。
        let native_desc = desc.validate()?;
        let requested = u64::from(native_desc.size_bytes_u32());
        // D3D12 常量 Buffer 的物理分配按 256 字节扩展，API 无关容量仍保持不变。
        let allocation = match desc.usage() {
            crate::platform::presentation::rhi::BufferUsage::Uniform => align_up(requested, 256),
            crate::platform::presentation::rhi::BufferUsage::Vertex
            | crate::platform::presentation::rhi::BufferUsage::Index => requested,
        };
        let resource_desc = buffer_resource_desc(allocation);
        let native = create_committed_resource(
            device,
            D3D12_HEAP_TYPE_UPLOAD,
            &resource_desc,
            D3D12_RESOURCE_STATE_GENERIC_READ,
            "ID3D12Device::CreateCommittedResource(RHI buffer)",
        )?;
        Ok(self.buffers.insert(D3d12RhiBuffer { native, desc }))
    }

    // 在 Map 前完成真实句柄、共享描述和载荷范围门禁。
    fn update_buffer(&self, upload: RhiBufferUpload<'_>) -> Result<()> {
        // 陈旧身份必须先于任何原生资源调用被共享表拒绝。
        let resource = self.buffers.get(upload.buffer())?;
        // 空载荷、容量、元素边界和 Uniform 完整替换由共享值对象唯一解释。
        let validated = upload.validate(resource.desc)?;
        let data = validated.data();
        let no_read = D3D12_RANGE { Begin: 0, End: 0 };
        let mut mapped = std::ptr::null_mut();
        // SAFETY: native 是存活的 upload-heap Buffer；共享门禁已证明写入范围位于冻结容量内。
        unsafe { resource.native.Map(0, Some(&no_read), Some(&mut mapped)) }
            .map_err(|error| d3d12_error("ID3D12Resource::Map(RHI buffer)", error))?;
        if mapped.is_null() {
            // SAFETY: Map 已成功但没有返回地址；空写范围结束本次映射。
            unsafe { resource.native.Unmap(0, Some(&no_read)) };
            return Err(platform_error("D3d12Context: RHI buffer Map returned null"));
        }
        // SAFETY: Map 返回的 upload-heap 地址至少覆盖资源物理分配；共享门禁证明 data 不超过逻辑容量。
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), mapped.cast::<u8>(), data.len());
            let written = D3D12_RANGE {
                Begin: 0,
                End: data.len(),
            };
            resource.native.Unmap(0, Some(&written));
        }
        Ok(())
    }

    // 只读预检类型化 Buffer 上传，不触碰 COM 或映射状态。
    fn preflight_buffer_upload(&self, upload: RhiBufferUploadPreflight) -> Result<()> {
        self.buffers.validate_upload(upload)
    }

    // 创建真实 default-heap Texture，并由共享表签发不可复用句柄。
    fn create_texture(
        &mut self,
        device: &ID3D12Device,
        desc: TextureDesc,
    ) -> Result<TextureHandle> {
        // 共享 extent 与格式值域必须先于任何原生格式和 heap 投影。
        desc.validate()?;
        let resource_desc = texture_resource_desc(desc);
        let native = create_committed_resource(
            device,
            D3D12_HEAP_TYPE_DEFAULT,
            &resource_desc,
            D3D12_RESOURCE_STATE_COPY_DEST,
            "ID3D12Device::CreateCommittedResource(RHI texture)",
        )?;
        Ok(self.textures.insert(D3d12RhiTexture {
            native,
            desc,
            state: D3D12_RESOURCE_STATE_COPY_DEST,
        }))
    }

    // 先完成全部共享纹理门禁，再创建和映射瞬时 upload heap。
    fn stage_texture_upload<'a>(
        &self,
        device: &ID3D12Device,
        upload: RhiTextureUpload<'a>,
    ) -> Result<D3d12TextureUploadPlan<'a>> {
        // 共享表先拒绝空、越界和已销毁身份。
        let texture = self.textures.get(upload.texture())?;
        // 共享 Transfer Component 再验证区域、格式字节宽度、载荷和紧密行跨度。
        let validated = upload.validate(texture.desc)?;
        let bounds = validated.bounds();
        let region = bounds.region().extent();
        let (destination_x, destination_y, _, _) = bounds.native_rect_u32();
        // 原生 footprint 只投影已经验证的区域尺寸和共享格式。
        let footprint_desc = texture_upload_footprint_desc(region, texture.desc.format());
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut total_bytes = 0u64;
        // SAFETY: device 存活；描述只含共享门禁通过的尺寸与格式，输出槽均有效。
        unsafe {
            device.GetCopyableFootprints(
                &footprint_desc,
                0,
                1,
                0,
                Some(&mut footprint),
                None,
                None,
                Some(&mut total_bytes),
            );
        }
        let total_bytes_usize = usize::try_from(total_bytes).map_err(|_| {
            platform_error("D3d12Context: RHI texture upload footprint exceeds address space")
        })?;
        let tight_pitch = validated.row_pitch() as usize;
        let native_pitch = footprint.Footprint.RowPitch as usize;
        if native_pitch < tight_pitch || total_bytes_usize == 0 {
            return Err(platform_error(
                "D3d12Context: RHI texture upload returned an invalid footprint",
            ));
        }
        let staging = create_upload_buffer(device, total_bytes)?;
        let no_read = D3D12_RANGE { Begin: 0, End: 0 };
        let mut mapped = std::ptr::null_mut();
        // SAFETY: staging 是存活的 upload heap；映射范围明确不读取 CPU cache。
        unsafe { staging.Map(0, Some(&no_read), Some(&mut mapped)) }
            .map_err(|error| d3d12_error("ID3D12Resource::Map(RHI texture upload)", error))?;
        if mapped.is_null() {
            // SAFETY: Map 已成功但地址为空；以空写范围结束映射。
            unsafe { staging.Unmap(0, Some(&no_read)) };
            return Err(platform_error(
                "D3d12Context: RHI texture upload Map returned null",
            ));
        }
        // 逐行把共享紧密载荷机械写入 D3D12 对齐 footprint。
        for row in 0..region.height as usize {
            let source = &validated.data()[row * tight_pitch..][..tight_pitch];
            // SAFETY: GetCopyableFootprints 保证每行 native_pitch 字节且 total_bytes 覆盖全部行。
            let destination = unsafe { mapped.cast::<u8>().add(row * native_pitch) };
            // SAFETY: source 与映射目标均覆盖 tight_pitch 字节且不会重叠。
            unsafe {
                std::ptr::copy_nonoverlapping(source.as_ptr(), destination, tight_pitch);
            }
        }
        // SAFETY: 上述写入完全落在 total_bytes footprint 内，映射此后不再访问。
        unsafe {
            let written = D3D12_RANGE {
                Begin: 0,
                End: total_bytes_usize,
            };
            staging.Unmap(0, Some(&written));
        }
        Ok(D3d12TextureUploadPlan {
            texture: upload.texture(),
            target: texture.native.clone(),
            before: texture.state,
            staging,
            footprint,
            destination_x,
            destination_y,
            _validated: validated,
        })
    }

    // 只有队列执行并等待成功后才能提交 texture 的原生状态事实。
    fn commit_texture_upload(&mut self, texture: TextureHandle) -> Result<()> {
        self.textures.get_mut(texture)?.state = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
        Ok(())
    }

    // 从共享描述提升 render-target 身份，不触碰描述符或命令列表。
    fn resolve_render_target(&self, texture: TextureHandle) -> Result<RenderTargetHandle> {
        self.textures.resolve_render_target(texture)
    }

    // 只读预检普通 texture copy 的真实资源和传输关系。
    fn preflight_texture_copy(&self, copy: TextureCopy) -> Result<()> {
        self.textures.validate_copy(copy)
    }

    // 只读预检 texture move 的真实资源和传输关系。
    fn preflight_texture_move(&self, movement: TextureMove) -> Result<()> {
        self.textures.validate_move(movement)
    }

    // 创建真实 shader-visible sampler descriptor，并由共享表签发句柄。
    fn create_sampler(
        &mut self,
        device: &ID3D12Device,
        desc: SamplerDesc,
    ) -> Result<SamplerHandle> {
        // 封闭 sampler 枚举先被穷尽映射，禁止原生调用自行补充隐含语义。
        let filter = match (desc.filter(), desc.mip_mode()) {
            (SamplerFilter::Nearest, SamplerMipMode::SingleLevel) => D3D12_FILTER_MIN_MAG_MIP_POINT,
            (SamplerFilter::Linear, SamplerMipMode::SingleLevel) => {
                D3D12_FILTER_MIN_MAG_LINEAR_MIP_POINT
            }
        };
        let address = match desc.address_mode() {
            SamplerAddressMode::ClampToEdge => D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        };
        let (min_lod, max_lod) = match desc.mip_mode() {
            SamplerMipMode::SingleLevel => (0.0, 0.0),
        };
        let native_desc = D3D12_SAMPLER_DESC {
            Filter: filter,
            AddressU: address,
            AddressV: address,
            AddressW: address,
            MipLODBias: 0.0,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D12_COMPARISON_FUNC_NEVER,
            BorderColor: [0.0; 4],
            MinLOD: min_lod,
            MaxLOD: max_lod,
        };
        let heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
            NumDescriptors: 1,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
            NodeMask: 0,
        };
        // SAFETY: device 存活；heap 描述在共享 sampler 映射完成后才进入原生创建。
        let heap: ID3D12DescriptorHeap = unsafe { device.CreateDescriptorHeap(&heap_desc) }
            .map_err(|error| {
                d3d12_error("ID3D12Device::CreateDescriptorHeap(RHI sampler)", error)
            })?;
        // SAFETY: heap 是一个含单槽的存活 sampler heap，起始 CPU handle 指向该槽。
        let cpu = unsafe { heap.GetCPUDescriptorHandleForHeapStart() };
        // SAFETY: native_desc 是共享封闭枚举的完整投影；cpu 指向刚创建的 sampler 槽。
        unsafe { device.CreateSampler(&native_desc, cpu) };
        Ok(self.samplers.insert(D3d12RhiSampler { heap, cpu, desc }))
    }

    // 检查式销毁 Buffer；共享表保证同一句柄不能再次使用。
    fn destroy_buffer(&mut self, buffer: BufferHandle) -> Result<()> {
        self.buffers.take(buffer)?;
        Ok(())
    }

    // 检查式销毁 Texture；本阶段没有可引用它的 pass 或 submission。
    fn destroy_texture(&mut self, texture: TextureHandle) -> Result<()> {
        self.textures.take(texture)?;
        Ok(())
    }

    // 检查式销毁 sampler descriptor heap。
    fn destroy_sampler(&mut self, sampler: SamplerHandle) -> Result<()> {
        self.samplers.take(sampler)?;
        Ok(())
    }

    // GPU 已检查式排空后，按创建逆序释放全部子资源。
    pub(super) fn shutdown(&mut self) -> Result<()> {
        for sampler in self.samplers.drain_reverse() {
            drop(sampler);
        }
        for texture in self.textures.drain_reverse() {
            drop(texture);
        }
        for buffer in self.buffers.drain_reverse() {
            drop(buffer);
        }
        Ok(())
    }

    // 无法证明 GPU 排空时泄漏最后一份 COM 引用，禁止 Drop 提前释放在途资源。
    pub(super) fn retain_after_undrained_drop(&mut self) {
        for sampler in self.samplers.drain_reverse() {
            std::mem::forget(sampler.heap);
        }
        for texture in self.textures.drain_reverse() {
            std::mem::forget(texture.native);
        }
        for buffer in self.buffers.drain_reverse() {
            std::mem::forget(buffer.native);
        }
    }
}

// 为 context 暴露资源阶段的 GraphicsDevice 形状，但组合入口仍保持未激活。
impl GraphicsDevice for D3d12Context {
    fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
        self.rhi_device.capabilities()
    }

    fn create_buffer(&mut self, desc: BufferDesc) -> Result<BufferHandle> {
        self.ensure_healthy()?;
        let result = self.rhi_device.create_buffer(&self.device, desc);
        self.observe_rhi_result("create RHI buffer", result)
    }

    fn update_buffer(&mut self, upload: RhiBufferUpload<'_>) -> Result<()> {
        self.ensure_healthy()?;
        let result = self.rhi_device.update_buffer(upload);
        self.observe_rhi_result("update RHI buffer", result)
    }

    fn preflight_buffer_upload(&self, upload: RhiBufferUploadPreflight) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.preflight_buffer_upload(upload)
    }

    fn create_texture(&mut self, desc: TextureDesc) -> Result<TextureHandle> {
        self.ensure_healthy()?;
        let result = self.rhi_device.create_texture(&self.device, desc);
        self.observe_rhi_result("create RHI texture", result)
    }

    fn resolve_render_target(&self, texture: TextureHandle) -> Result<RenderTargetHandle> {
        self.ensure_healthy()?;
        self.rhi_device.resolve_render_target(texture)
    }

    fn preflight_texture_copy(&self, copy: TextureCopy) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.preflight_texture_copy(copy)
    }

    fn preflight_texture_move(&self, movement: TextureMove) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.preflight_texture_move(movement)
    }

    fn preflight_draw_resources(&self, _packet: DrawPacket) -> Result<()> {
        self.ensure_healthy()?;
        Err(resource_stage_deferred("preflight_draw_resources"))
    }

    fn update_texture(&mut self, upload: RhiTextureUpload<'_>) -> Result<()> {
        self.ensure_healthy()?;
        // 所有共享门禁和 staging Map 必须在命令列表副作用之前完成。
        let result = self.rhi_device.stage_texture_upload(&self.device, upload);
        let plan = self.observe_rhi_result("stage RHI texture upload", result)?;
        self.begin_rhi_transfer_commands()?;
        record_transition(
            &self.command_list,
            &plan.target,
            plan.before,
            D3D12_RESOURCE_STATE_COPY_DEST,
        );
        let mut source = texture_copy_location_footprint(&plan.staging, plan.footprint);
        let mut destination = texture_copy_location_subresource(&plan.target);
        // SAFETY: 两个 copy location 均来自存活资源；共享区域与 footprint 已在所有原生副作用前验证。
        unsafe {
            self.command_list.CopyTextureRegion(
                &destination,
                plan.destination_x,
                plan.destination_y,
                0,
                &source,
                None,
            );
        }
        release_copy_location(&mut source);
        release_copy_location(&mut destination);
        record_transition(
            &self.command_list,
            &plan.target,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
        );
        // staging 在 fence 成功前必须由 context 的在途资源 owner 保持存活。
        self.pending_gpu_resources.push(plan.staging.clone());
        if let Err(error) = self.execute_recording_and_wait() {
            // 等待失败时保留 staging，由 checked shutdown 重试排空或 Drop 泄漏保护。
            return Err(error);
        }
        self.pending_gpu_resources.pop();
        // GPU 成功后才提交资源状态；失败 context 不得继续使用不确定状态。
        if let Err(error) = self.rhi_device.commit_texture_upload(plan.texture) {
            self.latch_fault("commit RHI texture upload state", &error);
            return Err(error);
        }
        Ok(())
    }

    fn create_sampler(&mut self, desc: SamplerDesc) -> Result<SamplerHandle> {
        self.ensure_healthy()?;
        let result = self.rhi_device.create_sampler(&self.device, desc);
        self.observe_rhi_result("create RHI sampler", result)
    }

    fn create_pipeline(&mut self, _desc: PipelineDesc) -> Result<PipelineBinding> {
        self.ensure_healthy()?;
        Err(resource_stage_deferred("create_pipeline"))
    }

    fn destroy_buffer(&mut self, buffer: BufferHandle) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.destroy_buffer(buffer)
    }

    fn destroy_texture(&mut self, texture: TextureHandle) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.destroy_texture(texture)
    }

    fn destroy_sampler(&mut self, sampler: SamplerHandle) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.destroy_sampler(sampler)
    }

    fn destroy_pipeline(&mut self, _pipeline: PipelineBinding) -> Result<()> {
        self.ensure_healthy()?;
        Err(resource_stage_deferred("destroy_pipeline"))
    }

    fn begin_render_pass(&mut self, _target: RenderTargetHandle, _load: LoadAction) -> Result<()> {
        self.ensure_healthy()?;
        Err(resource_stage_deferred("begin_render_pass"))
    }

    fn clear_rect(&mut self, _color: RhiColor, _scissor: RhiScissor) -> Result<()> {
        self.ensure_healthy()?;
        Err(resource_stage_deferred("clear_rect"))
    }

    fn draw(&mut self, _packet: DrawPacket) -> Result<()> {
        self.ensure_healthy()?;
        Err(resource_stage_deferred("draw"))
    }

    fn copy_texture(&mut self, _copy: TextureCopy) -> Result<()> {
        self.ensure_healthy()?;
        Err(resource_stage_deferred("copy_texture"))
    }

    fn move_texture_region(&mut self, _movement: TextureMove) -> Result<()> {
        self.ensure_healthy()?;
        Err(resource_stage_deferred("move_texture_region"))
    }

    fn end_render_pass(&mut self) -> Result<()> {
        self.ensure_healthy()?;
        Err(resource_stage_deferred("end_render_pass"))
    }

    fn submit(&mut self) -> Result<SubmissionHandle> {
        self.ensure_healthy()?;
        Err(resource_stage_deferred("submit"))
    }

    fn activate(&mut self) -> Result<()> {
        self.ensure_healthy()
    }

    fn maintain(&mut self) -> Result<()> {
        self.ensure_healthy()
    }
}

// 创建 D3D12 committed resource 并拒绝驱动返回空对象。
fn create_committed_resource(
    device: &ID3D12Device,
    heap_type: D3D12_HEAP_TYPE,
    desc: &D3D12_RESOURCE_DESC,
    initial_state: D3D12_RESOURCE_STATES,
    operation: &'static str,
) -> Result<ID3D12Resource> {
    let heap = D3D12_HEAP_PROPERTIES {
        Type: heap_type,
        CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
        MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
        CreationNodeMask: 0,
        VisibleNodeMask: 0,
    };
    let mut resource = None;
    // SAFETY: device 存活；heap/desc 完整初始化，输出槽由 COM 智能指针接管。
    unsafe {
        device.CreateCommittedResource(
            &heap,
            D3D12_HEAP_FLAG_NONE,
            desc,
            initial_state,
            None,
            &mut resource,
        )
    }
    .map_err(|error| d3d12_error(operation, error))?;
    resource
        .ok_or_else(|| platform_error(format!("D3d12Context: {operation} returned no resource")))
}

// 创建 upload heap Buffer，供一次纹理 copy 保持到 fence 完成。
fn create_upload_buffer(device: &ID3D12Device, size: u64) -> Result<ID3D12Resource> {
    let desc = buffer_resource_desc(size);
    create_committed_resource(
        device,
        D3D12_HEAP_TYPE_UPLOAD,
        &desc,
        D3D12_RESOURCE_STATE_GENERIC_READ,
        "ID3D12Device::CreateCommittedResource(RHI texture upload)",
    )
}

// 构造 D3D12 Buffer 的唯一原生描述。
fn buffer_resource_desc(size: u64) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
        Alignment: 0,
        Width: size,
        Height: 1,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: DXGI_FORMAT_UNKNOWN,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
        Flags: D3D12_RESOURCE_FLAG_NONE,
    }
}

// 构造真实 Texture 资源的唯一 D3D12 描述。
fn texture_resource_desc(desc: TextureDesc) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        Alignment: 0,
        Width: u64::from(desc.extent().width),
        Height: desc.extent().height,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: texture_format(desc.format()),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
        Flags: if desc.format().supports_render_target() {
            D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET
        } else {
            D3D12_RESOURCE_FLAG_NONE
        },
    }
}

// 构造只供 GetCopyableFootprints 消费的区域纹理描述。
fn texture_upload_footprint_desc(extent: RhiExtent, format: TextureFormat) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        Alignment: 0,
        Width: u64::from(extent.width),
        Height: extent.height,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: texture_format(format),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
        Flags: D3D12_RESOURCE_FLAG_NONE,
    }
}

// 穷尽映射共享 TextureFormat，原生 DXGI 差异不越过 Adapter。
fn texture_format(format: TextureFormat) -> DXGI_FORMAT {
    match format {
        TextureFormat::Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_UNORM,
        TextureFormat::Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
        TextureFormat::R8Unorm => DXGI_FORMAT_R8_UNORM,
    }
}

// 按 D3D12 常量 Buffer 物理对齐扩展容量。
fn align_up(value: u64, alignment: u64) -> u64 {
    value.div_ceil(alignment) * alignment
}

// 明确拒绝尚未进入本资源所有权阶段的 Device 原语。
fn resource_stage_deferred(operation: &'static str) -> Error {
    Error::new(
        Errc::NotImplemented,
        format!("D3D12 resource stage does not implement {operation}"),
    )
}

impl D3d12Context {
    // 只在 device-lost 时锁存资源原语失败，其余 typed error 原样交还调用方。
    fn observe_rhi_result<T>(&mut self, operation: &'static str, result: Result<T>) -> Result<T> {
        if let Err(error) = result.as_ref()
            && error.is(Errc::GraphicsDeviceLost)
        {
            self.latch_fault(operation, error);
        }
        result
    }

    // 为资源上传重置同一原生命令列表，但不绑定或转换 swapchain backbuffer。
    fn begin_rhi_transfer_commands(&mut self) -> Result<()> {
        self.ensure_healthy()?;
        if self.recording {
            return Err(Error::new(
                Errc::InvalidState,
                "D3D12 RHI texture upload cannot interrupt command recording",
            ));
        }
        if self.frame_index >= self.allocators.len() {
            return Err(platform_error(format!(
                "D3d12Context: invalid RHI transfer allocator index={} allocators={}",
                self.frame_index,
                self.allocators.len()
            )));
        }
        if let Err(error) = self.wait_for_fence(self.fence_values[self.frame_index]) {
            self.latch_fault("wait for RHI transfer allocator", &error);
            return Err(error);
        }
        let allocator = &self.allocators[self.frame_index];
        // SAFETY: 对应 frame fence 已完成，allocator 不再被 GPU 使用。
        if let Err(error) = unsafe { allocator.Reset() } {
            let error = d3d12_error("ID3D12CommandAllocator::Reset(RHI transfer)", error);
            self.latch_fault("reset RHI transfer allocator", &error);
            return Err(error);
        }
        // SAFETY: command list 处于 closed，allocator 已完成并重置；资源上传不绑定 pipeline state。
        if let Err(error) = unsafe {
            self.command_list
                .Reset(allocator, None::<&ID3D12PipelineState>)
        } {
            let error = d3d12_error("ID3D12GraphicsCommandList::Reset(RHI transfer)", error);
            self.latch_fault("reset RHI transfer command list", &error);
            return Err(error);
        }
        self.recording = true;
        Ok(())
    }
}
