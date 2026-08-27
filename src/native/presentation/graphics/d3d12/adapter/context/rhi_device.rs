//! D3D12 薄 RHI 的唯一资源 owner 与资源阶段 GraphicsDevice 原语。

use super::super::pipeline::{D3d12PipelineNativeBinding, D3d12PipelineResource};
use super::*;

use crate::platform::presentation::rhi::{
    BufferDesc, BufferHandle, DrawPacket, GraphicsDevice, GraphicsDeviceCapabilities, LoadAction,
    PipelineBinding, PipelineDesc, RenderTargetHandle, RhiBufferResource, RhiBufferResourceTable,
    RhiBufferUpload, RhiBufferUploadPreflight, RhiColor, RhiPassState, RhiPipelineResourceTable,
    RhiResourceTable, RhiScissor, RhiSubmissionSequence, RhiTextureResource,
    RhiTextureResourceTable, RhiTextureTransferBounds, RhiTextureUpload, SamplerAddressMode,
    SamplerDesc, SamplerFilter, SamplerHandle, SamplerMipMode, SubmissionHandle, TextureCopy,
    TextureDesc, TextureFormat, TextureHandle, TextureMove, UIX_COLOR_CONTRACT,
    ValidatedRhiTextureUpload,
};
use ::windows::Win32::Graphics::Direct3D12::{
    D3D12_BOX, D3D12_COMPARISON_FUNC_NEVER, D3D12_CPU_DESCRIPTOR_HANDLE,
    D3D12_DESCRIPTOR_HEAP_DESC, D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
    D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
    D3D12_DESCRIPTOR_HEAP_TYPE_RTV, D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
    D3D12_FILTER_MIN_MAG_LINEAR_MIP_POINT, D3D12_FILTER_MIN_MAG_MIP_POINT, D3D12_HEAP_FLAG_NONE,
    D3D12_HEAP_PROPERTIES, D3D12_HEAP_TYPE_DEFAULT, D3D12_HEAP_TYPE_UPLOAD, D3D12_RESOURCE_DESC,
    D3D12_RESOURCE_DIMENSION_BUFFER, D3D12_RESOURCE_DIMENSION_TEXTURE2D,
    D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, D3D12_RESOURCE_FLAG_NONE,
    D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_COPY_SOURCE,
    D3D12_RESOURCE_STATE_GENERIC_READ, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
    D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATES,
    D3D12_TEXTURE_ADDRESS_MODE_CLAMP, D3D12_TEXTURE_LAYOUT_ROW_MAJOR, D3D12_TEXTURE_LAYOUT_UNKNOWN,
    ID3D12DescriptorHeap, ID3D12Device, ID3D12Resource,
};
use ::windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM,
    DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
};

// 保存一个真实 D3D12 Buffer、创建时冻结的共享描述和完整逻辑内容。
struct D3d12RhiBuffer {
    // Upload heap 资源可直接作为顶点、索引或常量 Buffer 使用。
    native: ID3D12Resource,
    // 共享描述是所有上传预检的唯一事实。
    desc: BufferDesc,
    // CPU 镜像保存完整逻辑容量，供前缀更新继承未覆盖后缀。
    shadow: Vec<u8>,
}

// 让共享 Buffer 表读取 D3D12 创建时冻结的描述。
impl RhiBufferResource for D3d12RhiBuffer {
    fn desc(&self) -> BufferDesc {
        self.desc
    }
}

// 保存一个真实 D3D12 Texture、冻结描述和已提交原生状态。
struct D3d12RhiTexture {
    // 最后创建的 RTV descriptor heap 在正常关闭时最先释放。
    rtv_heap: Option<ID3D12DescriptorHeap>,
    // 每个纹理唯一拥有一个 shader-visible SRV descriptor heap。
    srv_heap: ID3D12DescriptorHeap,
    // Default heap 资源在两个 descriptor heap 之后释放。
    native: ID3D12Resource,
    // 共享描述是上传、copy 预检和 render-target 提升的唯一事实。
    desc: TextureDesc,
    // 只在命令执行并等待成功后提交新的资源状态。
    state: D3D12_RESOURCE_STATES,
    // 保存已经写入原生 SRV heap 的 CPU descriptor 身份。
    srv_cpu: D3D12_CPU_DESCRIPTOR_HANDLE,
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

// 保存共享传输验证完成后才能进入命令录制的跨纹理复制计划。
struct D3d12TextureCopyPlan {
    // 源句柄只用于 GPU 成功后的状态提交。
    source: TextureHandle,
    // 目标句柄只用于 GPU 成功后的状态提交。
    destination: TextureHandle,
    // 源原生资源在命令和 fence 完成前保持存活。
    source_native: ID3D12Resource,
    // 目标原生资源在命令和 fence 完成前保持存活。
    destination_native: ID3D12Resource,
    // 保存源资源命令开始前已提交的状态。
    source_before: D3D12_RESOURCE_STATES,
    // 保存目标资源命令开始前已提交的状态。
    destination_before: D3D12_RESOURCE_STATES,
    // 保存由 platform 唯一 TextureCopy 验证产生的两端边界。
    bounds: RhiTextureTransferBounds,
}

// D3D12 context 唯一拥有的资源阶段 Component。
pub(super) struct D3d12RhiDevice {
    // 直接复用 platform 唯一 Buffer 资源表，不复制句柄状态机。
    buffers: RhiBufferResourceTable<D3d12RhiBuffer>,
    // 直接复用 platform 唯一 Texture 资源表和 render-target 门禁。
    textures: RhiTextureResourceTable<D3d12RhiTexture>,
    // 直接复用 platform 唯一通用表签发 sampler 身份。
    samplers: RhiResourceTable<SamplerHandle, D3d12RhiSampler>,
    // 真实 Root Signature、shader、input layout、固定状态和 PSO 只由本资源表持有。
    pipelines: RhiPipelineResourceTable<D3d12PipelineResource>,
    // 直接复用 platform 唯一 pass 状态机，不复制目标、范围或打开状态。
    pass: RhiPassState,
    // 保存正在编码的唯一 D3D12 render target 与必要 COM owner。
    active_target: Option<rhi_device_pass::D3d12RhiRenderTarget>,
    // 保存已结束但尚未成功提交的原生目标与状态发布计划。
    pending_targets: Vec<rhi_device_pass::D3d12RhiRenderTarget>,
    // 保存命令列表从首个 draw 到 fence 成功期间引用的全部原生对象。
    pending_draw_resources: Vec<rhi_device_draw::D3d12RhiDrawResources>,
    // 直接复用 platform 唯一提交序列，供同一 Surface present 校验。
    submission_sequence: RhiSubmissionSequence,
}

#[path = "rhi_device_draw.rs"]
mod rhi_device_draw;
#[path = "rhi_device_pass.rs"]
mod rhi_device_pass;

impl D3d12RhiDevice {
    // 创建不持有任何原生资源的唯一 owner。
    pub(super) const fn new() -> Self {
        Self {
            buffers: RhiBufferResourceTable::new(),
            textures: RhiTextureResourceTable::new(),
            samplers: RhiResourceTable::new(),
            pipelines: RhiPipelineResourceTable::new(),
            pass: RhiPassState::new(),
            active_target: None,
            pending_targets: Vec::new(),
            pending_draw_resources: Vec::new(),
            submission_sequence: RhiSubmissionSequence::new(),
        }
    }

    // 返回只声明本阶段真实资源原语的能力快照。
    fn capabilities(&self) -> GraphicsDeviceCapabilities {
        GraphicsDeviceCapabilities {
            color_contract: UIX_COLOR_CONTRACT,
            dynamic_buffers: true,
            texture_upload: true,
            texture_copy: true,
            texture_region_move: true,
            clear_rect: true,
            sampled_textures: true,
            render_to_texture: true,
            scissor: true,
            premultiplied_alpha_blend: true,
            additive_blend: true,
        }
    }

    // 创建真实 upload-heap Buffer，并由共享表签发不可复用句柄。
    fn create_buffer(&mut self, device: &ID3D12Device, desc: BufferDesc) -> Result<BufferHandle> {
        // 共享容量、用途、步长与 Uniform ABI 必须先于 CreateCommittedResource。
        desc.validate()?;
        // 新资源的完整逻辑内容从确定性零值开始，分配失败不登记任何状态。
        let shadow = zeroed_buffer_shadow(desc.size_bytes())?;
        // 原生资源只有在完整逻辑镜像写入成功后才交给资源表。
        let native = create_buffer_upload_version(
            device,
            desc,
            &shadow,
            "ID3D12Device::CreateCommittedResource(RHI buffer)",
        )?;
        Ok(self.buffers.insert(D3d12RhiBuffer {
            native,
            desc,
            shadow,
        }))
    }

    // 在 Map 前完成真实句柄、共享描述和载荷范围门禁。
    fn update_buffer(&mut self, device: &ID3D12Device, upload: RhiBufferUpload<'_>) -> Result<()> {
        // 陈旧身份必须先于任何原生资源调用被共享表拒绝。
        let handle = upload.buffer();
        let current = self.buffers.get(handle)?;
        let desc = current.desc;
        // 空载荷、容量、元素边界和 Uniform 完整替换由共享值对象唯一解释。
        let validated = upload.validate(desc)?;
        let data = validated.data();
        // 先以可失败方式复制旧逻辑内容，再只覆盖共享合同允许的前缀。
        let mut next_shadow = clone_buffer_shadow(&current.shadow)?;
        next_shadow[..data.len()].copy_from_slice(data);
        // 每次上传创建新版本并写入完整逻辑容量，旧版本继续由既有 draw 引用保活。
        let native = create_buffer_upload_version(
            device,
            desc,
            &next_shadow,
            "ID3D12Device::CreateCommittedResource(RHI buffer upload version)",
        )?;
        // 只有镜像复制、前缀覆盖、原生创建和完整写入全部成功后，才整体替换当前版本。
        let replacement = D3d12RhiBuffer {
            native,
            desc,
            shadow: next_shadow,
        };
        let previous = std::mem::replace(self.buffers.get_mut(handle)?, replacement);
        // 不在 pending draw 中的旧 COM 引用可在提交新版本后正常释放。
        drop(previous);
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
        // 所有纹理由同一资源 owner 创建并保留唯一 shader-visible SRV heap。
        let (srv_heap, srv_cpu) = create_texture_srv_heap(device, &native, desc)?;
        // 可渲染纹理由同一资源 owner 额外创建并保留唯一 RTV heap。
        let rtv_heap = create_texture_rtv_heap(device, &native, desc)?;
        Ok(self.textures.insert(D3d12RhiTexture {
            native,
            desc,
            state: D3D12_RESOURCE_STATE_COPY_DEST,
            rtv_heap,
            srv_heap,
            srv_cpu,
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

    // 先完成真实身份、格式、非空范围和两端边界门禁，再克隆任何原生资源引用。
    fn stage_texture_copy(&self, copy: TextureCopy) -> Result<D3d12TextureCopyPlan> {
        // 共享资源表只读解析源身份与冻结描述。
        let source = self.textures.get(copy.source())?;
        // 共享资源表只读解析目标身份与冻结描述。
        let destination = self.textures.get(copy.destination())?;
        // 唯一 TextureCopy 契约拒绝同资源、格式差异、空范围、溢出和越界。
        let bounds = copy.validate_transfer(source.desc, destination.desc)?;
        // 只有共享门禁全部成功后才冻结原生资源与已提交状态。
        Ok(D3d12TextureCopyPlan {
            source: copy.source(),
            destination: copy.destination(),
            source_native: source.native.clone(),
            destination_native: destination.native.clone(),
            source_before: source.state,
            destination_before: destination.state,
            bounds,
        })
    }

    // 只读消费 platform 唯一 TextureMove 验证，并返回 scratch 所需的源描述。
    fn validate_texture_move(&self, movement: TextureMove) -> Result<TextureDesc> {
        // 先解析两端真实身份，禁止陈旧句柄触发后续原生资源创建。
        let source = self.textures.get(movement.source())?;
        let destination = self.textures.get(movement.destination())?;
        // 同纹理和跨纹理移动共用格式、非空范围与两端边界门禁。
        movement.validate_transfer(source.desc, destination.desc)?;
        Ok(source.desc)
    }

    // GPU 完成后一次提交复制两端的冻结原生状态。
    fn commit_texture_copy(
        &mut self,
        source: TextureHandle,
        destination: TextureHandle,
    ) -> Result<()> {
        // 提交前先证明两个句柄仍同时存活，避免部分状态发布。
        self.textures.get(source)?;
        self.textures.get(destination)?;
        self.textures.get_mut(source)?.state = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
        self.textures.get_mut(destination)?.state = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
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
        self.validate_texture_move(movement).map(|_| ())
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

    // 在完整原生对象成功创建后才由共享表签发不可拆 pipeline 身份。
    fn create_pipeline(
        &mut self,
        device: &ID3D12Device,
        desc: PipelineDesc,
    ) -> Result<PipelineBinding> {
        // Component 内部先完成共享契约门禁、Root Signature、HLSL 与全部 PSO 变体创建。
        let resource = D3d12PipelineResource::create(device, desc)?;
        // 只有完整资源存在时才登记，失败路径不会留下半成品句柄。
        Ok(self.pipelines.insert(desc.kind, resource))
    }

    // 检查式销毁 Buffer；共享表保证同一句柄不能再次使用。
    fn destroy_buffer(&mut self, buffer: BufferHandle) -> Result<()> {
        self.buffers.take(buffer)?;
        Ok(())
    }

    // 检查式销毁 Texture；活动或待提交 target 继续保留其唯一身份。
    fn destroy_texture(&mut self, texture: TextureHandle) -> Result<()> {
        self.pass.validate_texture_destroy(texture)?;
        self.validate_pending_texture_destroy(texture)?;
        self.textures.take(texture)?;
        Ok(())
    }

    // 检查式销毁 sampler descriptor heap。
    fn destroy_sampler(&mut self, sampler: SamplerHandle) -> Result<()> {
        self.samplers.take(sampler)?;
        Ok(())
    }

    // 检查 kind 与句柄身份后取出并释放完整原生 pipeline 资源。
    fn destroy_pipeline(&mut self, pipeline: PipelineBinding) -> Result<()> {
        self.pipelines.take(pipeline)?;
        Ok(())
    }

    // GPU 已检查式排空后，按创建逆序释放全部子资源。
    pub(super) fn shutdown(&mut self) -> Result<()> {
        // terminal fence 已排空，先失效提交并释放 pass/待提交目标的额外 COM owner。
        self.submission_sequence.invalidate();
        self.pass.reset();
        self.pending_draw_resources.clear();
        self.active_target = None;
        self.pending_targets.clear();
        for pipeline in self.pipelines.drain_reverse() {
            drop(pipeline);
        }
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
        // 未知 GPU 状态下保留活动与待提交 target 的资源和 descriptor owner。
        if let Some(target) = self.active_target.take() {
            target.retain_after_undrained_drop();
        }
        for target in self.pending_targets.drain(..) {
            target.retain_after_undrained_drop();
        }
        for draw in self.pending_draw_resources.drain(..) {
            draw.retain_after_undrained_drop();
        }
        for pipeline in self.pipelines.drain_reverse() {
            pipeline.retain_after_undrained_drop();
        }
        for sampler in self.samplers.drain_reverse() {
            std::mem::forget(sampler.heap);
        }
        for texture in self.textures.drain_reverse() {
            if let Some(rtv_heap) = texture.rtv_heap {
                std::mem::forget(rtv_heap);
            }
            std::mem::forget(texture.srv_heap);
            std::mem::forget(texture.native);
        }
        for buffer in self.buffers.drain_reverse() {
            std::mem::forget(buffer.native);
        }
    }
}

// 为生产 context 暴露完整资源与命令阶段的 GraphicsDevice 形状。
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
        let result = self.rhi_device.update_buffer(&self.device, upload);
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

    fn preflight_draw_resources(&self, packet: DrawPacket) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.preflight_draw_resources(packet)
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

    fn create_pipeline(&mut self, desc: PipelineDesc) -> Result<PipelineBinding> {
        self.ensure_healthy()?;
        let result = self.rhi_device.create_pipeline(&self.device, desc);
        self.observe_rhi_result("create RHI pipeline", result)
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

    fn destroy_pipeline(&mut self, pipeline: PipelineBinding) -> Result<()> {
        self.ensure_healthy()?;
        let result = self.rhi_device.destroy_pipeline(pipeline);
        self.observe_rhi_result("destroy RHI pipeline", result)
    }

    fn begin_render_pass(&mut self, target: RenderTargetHandle, load: LoadAction) -> Result<()> {
        self.rhi_begin_render_pass(target, load)
    }

    fn clear_rect(&mut self, color: RhiColor, scissor: RhiScissor) -> Result<()> {
        self.rhi_clear_rect(color, scissor)
    }

    fn draw(&mut self, packet: DrawPacket) -> Result<()> {
        self.ensure_healthy()?;
        self.draw_rhi_packet(packet)
    }

    fn copy_texture(&mut self, copy: TextureCopy) -> Result<()> {
        self.ensure_healthy()?;
        // 共享验证必须先于命令列表 Reset、barrier 和 CopyTextureRegion。
        let result = self.rhi_device.stage_texture_copy(copy);
        let plan = self.observe_rhi_result("stage RHI texture copy", result)?;
        self.begin_rhi_transfer_commands()?;
        record_texture_copy_commands(&self.command_list, &plan);
        // 两端原生资源在 fence 成功前由 context 在途 owner 保持存活。
        let pending_start = self.pending_gpu_resources.len();
        self.pending_gpu_resources.push(plan.source_native.clone());
        self.pending_gpu_resources
            .push(plan.destination_native.clone());
        if let Err(error) = self.execute_recording_and_wait() {
            // GPU 结果不确定时保留两端引用，交给 checked shutdown 排空或 Drop 泄漏保护。
            return Err(error);
        }
        self.pending_gpu_resources.truncate(pending_start);
        // 只有 GPU 完成成功后才同时发布源与目标的新状态事实。
        if let Err(error) = self
            .rhi_device
            .commit_texture_copy(plan.source, plan.destination)
        {
            self.latch_fault("commit RHI texture copy state", &error);
            return Err(error);
        }
        Ok(())
    }

    fn move_texture_region(&mut self, movement: TextureMove) -> Result<()> {
        self.ensure_healthy()?;
        // 在任何 scratch 创建或命令录制前直接消费共享 TextureMove 门禁。
        let result = self.rhi_device.validate_texture_move(movement);
        let source_desc = self.observe_rhi_result("validate RHI texture move", result)?;
        // 不同纹理没有重叠风险，无损复用已经闭环的 copy 原语。
        if movement.source() != movement.destination() {
            return self.copy_texture(movement.into_copy());
        }
        // 同纹理移动的 scratch 必须由唯一 D3D12 资源 owner 创建和登记。
        let scratch_desc = TextureDesc::new(movement.transfer().extent(), source_desc.format());
        let result = self.rhi_device.create_texture(&self.device, scratch_desc);
        let scratch = self.observe_rhi_result("create RHI texture move scratch", result)?;
        // platform 唯一 through_scratch 契约固定先保存、再恢复的 memmove 顺序。
        let (to_scratch, from_scratch) = movement.through_scratch(scratch);
        let operation = self
            .copy_texture(to_scratch)
            .and_then(|()| self.copy_texture(from_scratch));
        // 即使 context 已因命令失败锁存，也由资源 owner 检查式移除 scratch 身份。
        let cleanup = self.rhi_device.destroy_texture(scratch);
        match (operation, cleanup) {
            // 两者都失败时保留主移动错误，并把清理错误挂入来源链。
            (Err(primary_error), Err(cleanup_error)) => {
                Err(primary_error.with_source(cleanup_error))
            }
            // 只有移动失败时原样返回主错误。
            (Err(primary_error), Ok(())) => Err(primary_error),
            // 移动成功但清理失败不能伪造完整成功。
            (Ok(()), Err(cleanup_error)) => Err(cleanup_error),
            // 两段复制与 scratch 清理全部成功。
            (Ok(()), Ok(())) => Ok(()),
        }
    }

    fn end_render_pass(&mut self) -> Result<()> {
        self.rhi_end_render_pass()
    }

    fn submit(&mut self) -> Result<SubmissionHandle> {
        self.rhi_submit()
    }

    fn activate(&mut self) -> Result<()> {
        self.ensure_healthy()
    }

    fn maintain(&mut self) -> Result<()> {
        self.ensure_healthy()
    }
}

// 为每个纹理创建单槽 shader-visible SRV heap，并写入其唯一原生 descriptor。
fn create_texture_srv_heap(
    device: &ID3D12Device,
    texture: &ID3D12Resource,
    desc: TextureDesc,
) -> Result<(ID3D12DescriptorHeap, D3D12_CPU_DESCRIPTOR_HANDLE)> {
    // 共享纹理描述必须先于任何 descriptor heap 副作用通过完整值域门禁。
    desc.validate()?;
    let heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
        Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
        NumDescriptors: 1,
        Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
        NodeMask: 0,
    };
    // SAFETY: device 存活；heap 描述完整且只申请一个 shader-visible SRV 槽位。
    let heap: ID3D12DescriptorHeap =
        unsafe { device.CreateDescriptorHeap(&heap_desc) }.map_err(|error| {
            d3d12_error("ID3D12Device::CreateDescriptorHeap(RHI texture SRV)", error)
        })?;
    // SAFETY: 单槽 SRV heap 存活，起始 CPU handle 指向唯一 descriptor。
    let cpu = unsafe { heap.GetCPUDescriptorHandleForHeapStart() };
    // SAFETY: texture 是共享描述对应的有类型二维纹理；空 view 描述创建完整单 mip 默认 SRV。
    unsafe { device.CreateShaderResourceView(texture, None, cpu) };
    Ok((heap, cpu))
}

// 为可渲染纹理创建单槽 RTV heap；非渲染格式不伪造 descriptor。
fn create_texture_rtv_heap(
    device: &ID3D12Device,
    texture: &ID3D12Resource,
    desc: TextureDesc,
) -> Result<Option<ID3D12DescriptorHeap>> {
    if !desc.format().supports_render_target() {
        return Ok(None);
    }
    let heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
        Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
        NumDescriptors: 1,
        Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
        NodeMask: 0,
    };
    // SAFETY: device 存活；共享格式已证明支持 render target，heap 描述完整。
    let heap: ID3D12DescriptorHeap =
        unsafe { device.CreateDescriptorHeap(&heap_desc) }.map_err(|error| {
            d3d12_error("ID3D12Device::CreateDescriptorHeap(RHI texture RTV)", error)
        })?;
    // SAFETY: 单槽 RTV heap 存活，起始 CPU handle 指向唯一描述符。
    let rtv = unsafe { heap.GetCPUDescriptorHandleForHeapStart() };
    // SAFETY: texture 与 heap 属于同一 device，默认 view 与冻结的原生 texture 格式一致。
    unsafe { device.CreateRenderTargetView(texture, None, rtv) };
    Ok(Some(heap))
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

// 以可报告失败的方式创建确定性零值 Buffer 逻辑镜像。
fn zeroed_buffer_shadow(size: usize) -> Result<Vec<u8>> {
    let mut shadow = Vec::new();
    // 先完成唯一可能失败的容量扩展，调用方状态保持未提交。
    shadow
        .try_reserve_exact(size)
        .map_err(|_| platform_error("D3d12Context: RHI buffer shadow allocation failed"))?;
    // 已预留完整容量，零值扩展不会再次分配。
    shadow.resize(size, 0);
    Ok(shadow)
}

// 以可报告失败的方式复制完整 Buffer 逻辑镜像。
fn clone_buffer_shadow(current: &[u8]) -> Result<Vec<u8>> {
    let mut next = Vec::new();
    // 先尝试预留全部容量，失败时不改变当前镜像或原生版本。
    next.try_reserve_exact(current.len())
        .map_err(|_| platform_error("D3d12Context: RHI buffer shadow copy failed"))?;
    // 容量已经完整预留，因此复制旧后缀不会再触发分配。
    next.extend_from_slice(current);
    Ok(next)
}

// 创建并写满一个 D3D12 upload-heap Buffer 版本。
fn create_buffer_upload_version(
    device: &ID3D12Device,
    desc: BufferDesc,
    shadow: &[u8],
    operation: &'static str,
) -> Result<ID3D12Resource> {
    // CPU owner 的内部镜像必须精确覆盖 API 无关逻辑容量。
    if shadow.len() != desc.size_bytes() {
        return Err(platform_error(
            "D3d12Context: RHI buffer shadow size diverged",
        ));
    }
    let requested = desc.size_bytes() as u64;
    // D3D12 常量 Buffer 的物理分配按 256 字节扩展，逻辑内容仍只由共享描述决定。
    let allocation = match desc.usage() {
        crate::platform::presentation::rhi::BufferUsage::Uniform => align_up(requested, 256),
        crate::platform::presentation::rhi::BufferUsage::Vertex
        | crate::platform::presentation::rhi::BufferUsage::Index => requested,
    };
    let native_desc = buffer_resource_desc(allocation);
    let native = create_committed_resource(
        device,
        D3D12_HEAP_TYPE_UPLOAD,
        &native_desc,
        D3D12_RESOURCE_STATE_GENERIC_READ,
        operation,
    )?;
    let no_read = D3D12_RANGE { Begin: 0, End: 0 };
    let mut mapped = std::ptr::null_mut();
    // SAFETY: native 是新建且存活的 upload-heap Buffer，物理容量不小于完整逻辑镜像。
    unsafe { native.Map(0, Some(&no_read), Some(&mut mapped)) }
        .map_err(|error| d3d12_error("ID3D12Resource::Map(RHI buffer)", error))?;
    if mapped.is_null() {
        // SAFETY: Map 已成功但没有返回地址；空写范围结束本次映射。
        unsafe { native.Unmap(0, Some(&no_read)) };
        return Err(platform_error("D3d12Context: RHI buffer Map returned null"));
    }
    // SAFETY: Map 返回的地址覆盖物理分配，shadow 精确覆盖已验证的完整逻辑容量。
    unsafe {
        std::ptr::copy_nonoverlapping(shadow.as_ptr(), mapped.cast::<u8>(), shadow.len());
        let written = D3D12_RANGE {
            Begin: 0,
            End: shadow.len(),
        };
        native.Unmap(0, Some(&written));
    }
    Ok(native)
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

// 把已经通过共享门禁的复制计划机械录制为 D3D12 状态转换与区域复制。
fn record_texture_copy_commands(
    command_list: &ID3D12GraphicsCommandList,
    plan: &D3d12TextureCopyPlan,
) {
    // 共享边界直接投影为 D3D12 二维源 box。
    let (source_left, source_top, source_right, source_bottom) =
        plan.bounds.source().native_rect_u32();
    let source_box = D3D12_BOX {
        left: source_left,
        top: source_top,
        front: 0,
        right: source_right,
        bottom: source_bottom,
        back: 1,
    };
    // 共享目标边界只投影已验证的左上原点。
    let destination_origin = plan.bounds.destination().region().origin();
    record_transition(
        command_list,
        &plan.source_native,
        plan.source_before,
        D3D12_RESOURCE_STATE_COPY_SOURCE,
    );
    record_transition(
        command_list,
        &plan.destination_native,
        plan.destination_before,
        D3D12_RESOURCE_STATE_COPY_DEST,
    );
    let mut source = texture_copy_location_subresource(&plan.source_native);
    let mut destination = texture_copy_location_subresource(&plan.destination_native);
    // SAFETY: 两端均为同一 device 创建的存活二维纹理；格式、资源关系和区域已由 TextureCopy 验证。
    unsafe {
        command_list.CopyTextureRegion(
            &destination,
            destination_origin.x(),
            destination_origin.y(),
            0,
            &source,
            Some(&source_box),
        );
    }
    release_copy_location(&mut source);
    release_copy_location(&mut destination);
    // 完成复制后统一回到本资源阶段已冻结的可采样状态。
    record_transition(
        command_list,
        &plan.source_native,
        D3D12_RESOURCE_STATE_COPY_SOURCE,
        D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
    );
    record_transition(
        command_list,
        &plan.destination_native,
        D3D12_RESOURCE_STATE_COPY_DEST,
        D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
    );
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

    // 为薄 RHI 操作重置同一原生命令列表，但不绑定或转换 swapchain backbuffer。
    pub(super) fn reset_rhi_command_list(&mut self, operation: &'static str) -> Result<()> {
        self.ensure_healthy()?;
        if self.frame_index >= self.allocators.len() {
            return Err(platform_error(format!(
                "D3d12Context: invalid {operation} allocator index={} allocators={}",
                self.frame_index,
                self.allocators.len()
            )));
        }
        if let Err(error) = self.wait_for_fence(self.fence_values[self.frame_index]) {
            self.latch_fault("wait for RHI command allocator", &error);
            return Err(error);
        }
        let allocator = &self.allocators[self.frame_index];
        // SAFETY: 对应 frame fence 已完成，allocator 不再被 GPU 使用。
        if let Err(error) = unsafe { allocator.Reset() } {
            let error = d3d12_error("ID3D12CommandAllocator::Reset(RHI)", error);
            self.latch_fault("reset RHI command allocator", &error);
            return Err(error);
        }
        // SAFETY: command list 处于 closed，allocator 已完成并重置；薄 RHI 不隐式绑定 pipeline state。
        if let Err(error) = unsafe {
            self.command_list
                .Reset(allocator, None::<&ID3D12PipelineState>)
        } {
            let error = d3d12_error("ID3D12GraphicsCommandList::Reset(RHI)", error);
            self.latch_fault("reset RHI command list", &error);
            return Err(error);
        }
        self.recording = true;
        Ok(())
    }

    // 为独立资源传输开启新命令序列，禁止打断尚未提交的 render pass 批次。
    fn begin_rhi_transfer_commands(&mut self) -> Result<()> {
        if self.recording {
            return Err(Error::new(
                Errc::InvalidState,
                "D3D12 RHI texture transfer cannot interrupt command recording",
            ));
        }
        self.reset_rhi_command_list("RHI texture transfer")
    }
}
