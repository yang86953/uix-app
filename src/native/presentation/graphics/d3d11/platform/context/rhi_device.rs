//! D3D11 device 对薄 RHI 的资源与 pass 迁移期实现。
//!
//! 当前纵切已经覆盖资源生命周期、目标绑定、clear、copy、viewport、scissor、
//! solid/textured draw packet 与 submit；渐变、字形、离屏效果等高层语义仍
//! 由兼容 pipeline 承担，不在薄 RHI 中重新定义。

#![allow(dead_code)]
#![allow(nonstandard_style)]

// 引入统一错误和结果类型。
use crate::core::error::{Errc, Error, Result};
// 引入薄 RHI 的资源、命令和能力类型。
use crate::native::present::rhi::{
    BufferDesc, BufferHandle, BufferUsage, DrawPacket, GraphicsDevice, GraphicsDeviceCapabilities,
    LoadAction, PipelineBinding, PipelineDesc, PipelineHandle, PipelineKind, RenderTargetHandle,
    RhiBufferUpload, RhiColor, RhiColorClearContract, RhiExtent, RhiPassState, RhiScissor,
    RhiSubmissionSequence, RhiTextureRegion, RhiViewport, SampledTextureBinding, SamplerDesc,
    SamplerHandle, TextureCopy, TextureDesc, TextureFormat, TextureHandle, TextureMove,
    UIX_COLOR_CLEAR_CONTRACT,
};
// 引入 D3D11 的基础资源和绑定类型。
use ::windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_CONSTANT_BUFFER, D3D11_BIND_INDEX_BUFFER, D3D11_BIND_RENDER_TARGET,
    D3D11_BIND_SHADER_RESOURCE, D3D11_BIND_VERTEX_BUFFER, D3D11_BOX, D3D11_BUFFER_DESC,
    D3D11_COMPARISON_NEVER, D3D11_CPU_ACCESS_WRITE, D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT,
    D3D11_FILTER_MIN_MAG_MIP_POINT, D3D11_MAP_WRITE_DISCARD, D3D11_MAPPED_SUBRESOURCE,
    D3D11_SAMPLER_DESC, D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
    D3D11_USAGE_DYNAMIC, D3D11_VIEWPORT,
};
// 引入 D3D11 格式和统一的窗口矩形类型。
use ::windows::Win32::Foundation::RECT;
// 引入 D3D11 纹理格式。
use ::windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM,
};

// 引入 context 父模块的 D3D11 状态、swapchain 事实和 surface target 身份。
use super::{D3d11Context, D3d11IndexBinding, RHI_SURFACE_TARGET_RAW};

// 把资源生命周期拆到独立文件，保持每个代码文件处于可审阅的尺寸内。
#[path = "rhi_device_resources.rs"]
mod rhi_device_resources;
// 将 raster state 编码拆出，保持主适配器文件低于行数上限。
#[path = "rhi_device_state.rs"]
mod rhi_device_state;
// 将局部颜色清理拆出，保持 D3D11 资源主文件低于行数上限。
#[path = "rhi_device_clear.rs"]
mod rhi_device_clear;
// 将固定 draw ABI 分派拆出，保持资源与命令主文件低于行数上限。
#[path = "rhi_device_draw.rs"]
mod rhi_device_draw;
// 将 pass 收尾与提交拆出，保持资源与命令主文件低于行数上限。
#[path = "rhi_device_submit.rs"]
mod rhi_device_submit;

// 保存一个 RHI buffer 的原生对象和通用描述。
struct D3d11RhiBuffer {
    // 保持 D3D11 buffer 的生命周期。
    native: ::windows::Win32::Graphics::Direct3D11::ID3D11Buffer,
    // 保存已经通过共同门禁的完整 Buffer 描述。
    desc: BufferDesc,
}

// 保存一个 RHI texture 的原生对象和可选 view。
struct D3d11RhiTexture {
    // 保持 D3D11 texture 的生命周期。
    native: ::windows::Win32::Graphics::Direct3D11::ID3D11Texture2D,
    // 保存 render target view，R8 覆盖率纹理可以没有它。
    rtv: Option<::windows::Win32::Graphics::Direct3D11::ID3D11RenderTargetView>,
    // 保存 sampled shader resource view。
    srv: ::windows::Win32::Graphics::Direct3D11::ID3D11ShaderResourceView,
    // 保存通用资源尺寸。
    extent: RhiExtent,
    // 保存通用资源格式。
    format: TextureFormat,
}

// 保存一个 RHI pipeline 的封闭通用语义。
struct D3d11RhiPipeline {
    // 保存由通用 renderer 选择的类型化 pipeline 语义。
    kind: PipelineKind,
}

// 保存一个 RHI sampler 的原生状态对象。
struct D3d11RhiSampler {
    // 保持 D3D11 sampler state 的生命周期。
    native: ::windows::Win32::Graphics::Direct3D11::ID3D11SamplerState,
    // 保存共享 pipeline draw 门禁需要的 API 无关过滤事实。
    desc: SamplerDesc,
}

// 持有 D3D11 RHI 资源表与 owner-thread pass 状态。
pub(super) struct D3d11RhiDevice {
    // 保存按不透明 id 索引的 buffer 资源。
    buffers: Vec<Option<D3d11RhiBuffer>>,
    // 保存按不透明 id 索引的 texture 资源。
    textures: Vec<Option<D3d11RhiTexture>>,
    // 保存按不透明 id 索引的有限 pipeline 资源。
    pipelines: Vec<Option<D3d11RhiPipeline>>,
    // 保存按不透明 id 索引的 sampler 资源。
    samplers: Vec<Option<D3d11RhiSampler>>,
    // 保存两个 Adapter 共用的 pass 生命周期、目标、几何与采样绑定事实。
    pass: RhiPassState,
    // 只保存 D3D11 编码当前 pass 所需的原生 render target view。
    active_target: Option<::windows::Win32::Graphics::Direct3D11::ID3D11RenderTargetView>,
    // 保存共享的提交身份状态机，禁止 D3D11 绕过 Surface present 校验。
    submission_sequence: RhiSubmissionSequence,
}

// 为 D3D11 RHI 状态提供初始化和资源查找辅助。
impl D3d11RhiDevice {
    // 创建没有资源和打开 pass 的初始状态。
    pub(super) const fn new() -> Self {
        // 返回可安全嵌入 D3D11 context 的空状态。
        Self {
            buffers: Vec::new(),
            textures: Vec::new(),
            pipelines: Vec::new(),
            samplers: Vec::new(),
            // 使用 API 无关状态机初始化 pass 生命周期。
            pass: RhiPassState::new(),
            active_target: None,
            // 使用 API 无关状态机初始化提交序列。
            submission_sequence: RhiSubmissionSequence::new(),
        }
    }

    // 通过一开始从 1 分配的句柄读取 buffer。
    fn buffer(&self, handle: BufferHandle) -> Result<&D3d11RhiBuffer> {
        // 零句柄和越界句柄都表示调用方没有完成资源绑定。
        let Some(index) = handle.raw().checked_sub(1) else {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("buffer handle is null"));
        };
        // 读取资源槽并检查已销毁状态。
        self.buffers
            .get(index as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| rhi_invalid("buffer handle is stale"))
    }

    // 通过一开始从 1 分配的句柄读取可变 buffer。
    fn buffer_mut(&mut self, handle: BufferHandle) -> Result<&mut D3d11RhiBuffer> {
        // 零句柄和越界句柄都表示调用方没有完成资源绑定。
        let Some(index) = handle.raw().checked_sub(1) else {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("buffer handle is null"));
        };
        // 读取可变资源槽并检查已销毁状态。
        self.buffers
            .get_mut(index as usize)
            .and_then(Option::as_mut)
            .ok_or_else(|| rhi_invalid("buffer handle is stale"))
    }

    // 通过不透明句柄读取已创建的 pipeline。
    fn pipeline(&self, handle: PipelineHandle) -> Result<&D3d11RhiPipeline> {
        // 零句柄和越界句柄都表示调用方没有完成 pipeline 绑定。
        let Some(index) = handle.raw().checked_sub(1) else {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("pipeline handle is null"));
        };
        // 读取资源槽并检查已销毁状态。
        self.pipelines
            .get(index as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| rhi_invalid("pipeline handle is stale"))
    }

    // 通过不透明句柄读取已创建的 sampler。
    fn sampler(&self, handle: SamplerHandle) -> Result<&D3d11RhiSampler> {
        // 零句柄和越界句柄都表示调用方没有完成 sampler 绑定。
        let Some(index) = handle.raw().checked_sub(1) else {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("sampler handle is null"));
        };
        // 读取资源槽并检查已销毁状态。
        self.samplers
            .get(index as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| rhi_invalid("sampler handle is stale"))
    }

    // 通过一开始从 1 分配的句柄读取 texture。
    fn texture(&self, handle: TextureHandle) -> Result<&D3d11RhiTexture> {
        // 零句柄和越界句柄都表示调用方没有完成资源绑定。
        let Some(index) = handle.raw().checked_sub(1) else {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("texture handle is null"));
        };
        // 读取资源槽并检查已销毁状态。
        self.textures
            .get(index as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| rhi_invalid("texture handle is stale"))
    }

    // 通过 render target 句柄读取对应 texture。
    fn target_texture(&self, raw: u64) -> Result<&D3d11RhiTexture> {
        // RHI target 和 texture 共用同一个不透明资源身份。
        self.texture(TextureHandle::from_raw(raw))
    }

    // 保存纹理格式对应的 DXGI 格式、每像素字节数和是否可作为 RTV。
    fn texture_format(format: TextureFormat) -> (i32, usize, bool) {
        // 把有限的通用格式映射到 D3D11 事实格式。
        match format {
            // 映射 BGRA 八位格式。
            TextureFormat::Bgra8Unorm => (DXGI_FORMAT_B8G8R8A8_UNORM.0, 4, true),
            // 映射 RGBA 八位格式。
            TextureFormat::Rgba8Unorm => (DXGI_FORMAT_R8G8B8A8_UNORM.0, 4, true),
            // 映射单通道覆盖率格式。
            TextureFormat::R8Unorm => (DXGI_FORMAT_R8_UNORM.0, 1, false),
        }
    }
}

// 为 D3D11 context 实现 buffer、texture、pass、copy 和 submit 原语。
impl GraphicsDevice for D3d11Context {
    // 返回当前迁移期 device 的事实能力，能力只描述低层原语而非 UI 操作。
    fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
        // D3D11 已实现通用 Renderer 需要的完整 Device 原语基线。
        let mut capabilities = GraphicsDeviceCapabilities::full_gpu_baseline();
        // D3D11 scratch texture 提供重叠安全的区域移动。
        capabilities.texture_region_move = true;
        // D3D11 scissor clear 已接入 FramePlan 局部清理。
        capabilities.clear_rect = true;
        // 返回不再读取 swapchain 或 Surface 状态的 Device 能力快照。
        capabilities
    }

    // 把设备健康检查委托给独立模块，避免资源实现超过文件行数边界。
    fn maintain(&mut self) -> Result<()> {
        // 统一沿用 context 健康维护实现和 DXGI typed mapping。
        self.maintain_rhi_device()
    }
    #[cfg(feature = "test-harness")]
    fn inject_device_lost_for_test(&mut self) -> Result<()> {
        self.arm_rhi_device_lost_for_test();
        Ok(())
    }

    // 创建 D3D11 默认 buffer。
    fn create_buffer(&mut self, desc: BufferDesc) -> Result<BufferHandle> {
        // 先通过两个 Adapter 共用的容量、步长与 Uniform ABI 门禁。
        let native = desc.validate()?;
        // 选择底层绑定类型、更新方式和 CPU 写入权限。
        let (bind_flags, usage, cpu_access, stride_bytes) = match desc.usage() {
            // 顶点 buffer 使用默认显存资源并按 stride 绑定。
            BufferUsage::Vertex => (
                D3D11_BIND_VERTEX_BUFFER.0 as u32,
                D3D11_USAGE_DEFAULT,
                0,
                desc.stride_bytes(),
            ),
            // 索引 buffer 使用默认显存资源，索引格式由 draw ABI 固定。
            BufferUsage::Index => (
                D3D11_BIND_INDEX_BUFFER.0 as u32,
                D3D11_USAGE_DEFAULT,
                0,
                desc.stride_bytes(),
            ),
            // Uniform Buffer 使用共享门禁已经验证的动态常量资源。
            BufferUsage::Uniform => (
                D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                D3D11_USAGE_DYNAMIC,
                D3D11_CPU_ACCESS_WRITE.0 as u32,
                0,
            ),
        };
        // 准备 D3D11 buffer 描述。
        let native_desc = D3D11_BUFFER_DESC {
            ByteWidth: native.size_bytes_u32(),
            Usage: usage,
            BindFlags: bind_flags,
            CPUAccessFlags: cpu_access,
            MiscFlags: 0,
            StructureByteStride: stride_bytes,
        };
        // 为 CreateBuffer 准备空初始数据。
        let mut native = None;
        // SAFETY: device 属于当前 owner thread 的 D3D11 context，描述和输出槽
        // 由本函数构造且在调用期间保持有效。
        unsafe {
            self.device
                .CreateBuffer(&native_desc, None, Some(&mut native))
                .map_err(|error| d3d_error("ID3D11Device::CreateBuffer(rhi)", error))?;
        }
        // 拒绝驱动返回的空资源。
        let native =
            native.ok_or_else(|| rhi_platform("D3d11 RHI CreateBuffer returned no buffer"))?;
        // 分配从 1 开始的不透明资源身份。
        self.rhi_device.buffers.push(Some(D3d11RhiBuffer {
            native,
            // Adapter 只保存唯一共享描述，不再复制三个可漂移字段。
            desc,
        }));
        // 计算刚刚追加的资源句柄。
        let raw = self.rhi_device.buffers.len() as u64;
        // 返回 opaque buffer handle。
        Ok(BufferHandle::from_raw(raw))
    }

    // 将紧密排列的数据写入已有 D3D11 buffer。
    fn update_buffer(&mut self, upload: RhiBufferUpload<'_>) -> Result<()> {
        // 先解析目标身份，空载荷也不能绕过陈旧句柄门禁。
        let resource = self.rhi_device.buffer(upload.buffer())?;
        // 由共享 Component 验证前缀范围、元素边界和 Uniform 完整替换。
        let validated = upload.validate(resource.desc)?;
        // 原生调用只消费已经验证的不可变字节。
        let data = validated.data();
        // 动态 uniform 通过 Map/WRITE_DISCARD 完整更新，避免把 default buffer
        // 的 UpdateSubresource 语义错误地套到 CPU 可写资源上。
        if resource.desc.usage() == BufferUsage::Uniform {
            // 准备动态映射输出。
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            // SAFETY: uniform resource 由当前 device 以 DYNAMIC 创建，data 在
            // 调用期间保持只读有效，Map/Unmap 只在 owner thread 执行。
            unsafe {
                self.context
                    .Map(
                        &resource.native,
                        0,
                        D3D11_MAP_WRITE_DISCARD,
                        0,
                        Some(&mut mapped),
                    )
                    .map_err(|error| d3d_error("ID3D11DeviceContext::Map(rhi)", error))?;
                std::ptr::copy_nonoverlapping(data.as_ptr(), mapped.pData.cast::<u8>(), data.len());
                self.context.Unmap(&resource.native, 0);
            }
            // 返回 uniform 更新成功。
            return Ok(());
        }
        // 构造只覆盖本次更新的 D3D11 buffer box。
        let dst_box = D3D11_BOX {
            // 类型化上传固定从 Buffer 起点开始。
            left: 0,
            // 右边界由共享上传门禁完成无损投影。
            right: validated.size_bytes_u32(),
            top: 0,
            bottom: 1,
            front: 0,
            back: 1,
        };
        // SAFETY: resource 由同一 owner-thread device 创建，data 在调用期间
        // 保持只读有效；D3D11 立即上下文不会跨线程使用该资源。
        unsafe {
            self.context.UpdateSubresource(
                &resource.native,
                0,
                Some(&dst_box),
                data.as_ptr().cast(),
                data.len() as u32,
                0,
            );
        }
        // 返回上传成功。
        Ok(())
    }

    // 创建可采样且尽可能可作为 render target 的 D3D11 texture。
    fn create_texture(&mut self, desc: TextureDesc) -> Result<TextureHandle> {
        // 把资源创建细节委托给按文件拆分的 RHI resource helper。
        self.rhi_create_texture(desc)
    }

    // 创建当前 D3D11 适配器已经具备 shader ABI 的有限 pipeline。
    fn create_pipeline(&mut self, desc: PipelineDesc) -> Result<PipelineBinding> {
        // 把原生 pipeline 资源登记委托给 resource helper。
        let handle = self.rhi_create_pipeline(desc)?;
        // 让上层只取得句柄与同一创建语义组成的不可拆身份。
        Ok(PipelineBinding::new(handle, desc.kind))
    }

    // 创建带 clamp 地址模式的 D3D11 sampler。
    fn create_sampler(&mut self, desc: SamplerDesc) -> Result<SamplerHandle> {
        // 把 sampler state 创建委托给 resource helper。
        self.rhi_create_sampler(desc)
    }

    // 上传 texture 的紧密排列像素。
    fn update_texture(
        &mut self,
        texture: TextureHandle,
        extent: RhiExtent,
        data: &[u8],
    ) -> Result<()> {
        // 整块上传是从左上角开始的类型化完整区域。
        self.update_texture_region(texture, RhiTextureRegion::full(extent), data)
    }

    // 上传 texture 中任意合法的紧密排列子区域。
    fn update_texture_region(
        &mut self,
        texture: TextureHandle,
        region: RhiTextureRegion,
        data: &[u8],
    ) -> Result<()> {
        // 读取目标纹理的格式和尺寸事实。
        let resource = self.rhi_device.texture(texture)?;
        // 共享区域门禁统一验证原点、尺寸、溢出与资源边界。
        let bounds = region.validate_within(resource.extent)?;
        // 计算当前格式的每像素字节数。
        let (_, bytes_per_pixel, _) = D3d11RhiDevice::texture_format(resource.format);
        // 从共享区域读取紧密载荷长度和 D3D11 行跨度。
        let (required, row_pitch) = bounds
            // 使用当前格式的字节宽度投影布局。
            .tight_payload_layout(bytes_per_pixel)
            .ok_or_else(|| rhi_invalid("D3d11 RHI texture region size overflows"))?;
        // 拒绝短 payload，避免驱动读取未初始化内存。
        if data.len() != required {
            // 返回稳定的参数错误。
            return Err(rhi_invalid(
                "D3d11 RHI texture region payload length is invalid",
            ));
        }
        // 读取共享区域已经验证的四条无符号边。
        let (left, top, right, bottom) = bounds.native_rect_u32();
        // 构造覆盖目标区域的 texture box。
        let dst_box = D3D11_BOX {
            // 左边界来自共享类型化区域。
            left,
            // 右边界来自共享 checked 加法。
            right,
            // 顶边界来自共享类型化区域。
            top,
            // 底边界来自共享 checked 加法。
            bottom,
            front: 0,
            back: 1,
        };
        // SAFETY: resource 由同一 owner-thread device 创建，data 在调用期间
        // 保持只读有效，row pitch 与紧密排列的 payload 一致。
        unsafe {
            self.context.UpdateSubresource(
                &resource.native,
                0,
                Some(&dst_box),
                data.as_ptr().cast(),
                row_pitch,
                0,
            );
        }
        // 返回上传成功。
        Ok(())
    }

    // 销毁 buffer 资源槽。
    fn destroy_buffer(&mut self, buffer: BufferHandle) -> Result<()> {
        // 解析资源身份并检查是否已销毁。
        let index = buffer
            .raw()
            .checked_sub(1)
            .ok_or_else(|| rhi_invalid("buffer handle is null"))? as usize;
        // 读取资源槽。
        let Some(slot) = self.rhi_device.buffers.get_mut(index) else {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("buffer handle is stale"));
        };
        // 拒绝重复销毁。
        if slot.is_none() {
            // 返回稳定的状态错误。
            return Err(rhi_invalid("buffer handle was already destroyed"));
        }
        // 清空资源槽，让旧句柄立即失效。
        *slot = None;
        // 返回成功。
        Ok(())
    }

    // 销毁 texture 资源槽。
    fn destroy_texture(&mut self, texture: TextureHandle) -> Result<()> {
        // 解析资源身份并检查是否已销毁。
        let index = texture
            .raw()
            .checked_sub(1)
            .ok_or_else(|| rhi_invalid("texture handle is null"))? as usize;
        // 不能在当前 pass 仍引用资源时销毁它。
        if self.rhi_device.pass.references_target(texture) {
            // 返回稳定的状态错误。
            return Err(Error::new(
                Errc::InvalidState,
                "D3d11 RHI cannot destroy the active render target",
            ));
        }
        // 读取资源槽。
        let Some(slot) = self.rhi_device.textures.get_mut(index) else {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("texture handle is stale"));
        };
        // 拒绝重复销毁。
        if slot.is_none() {
            // 返回稳定的状态错误。
            return Err(rhi_invalid("texture handle was already destroyed"));
        }
        // 清空资源槽，让旧句柄立即失效。
        *slot = None;
        // 清理当前 pass 可能持有的 sampled texture 身份。
        self.rhi_device.pass.unbind_texture(texture);
        // 返回成功。
        Ok(())
    }

    // 销毁 pipeline 资源槽。
    fn destroy_pipeline(&mut self, pipeline: PipelineBinding) -> Result<()> {
        // 把绑定中不透明句柄的检查式销毁委托给 resource helper。
        self.rhi_destroy_pipeline(pipeline.handle())
    }

    // 销毁 sampler 资源槽。
    fn destroy_sampler(&mut self, sampler: SamplerHandle) -> Result<()> {
        // 把检查式销毁委托给 resource helper。
        self.rhi_destroy_sampler(sampler)
    }

    // 开始一个 D3D11 render pass，并绑定 surface 或 RHI texture target。
    fn begin_render_pass(&mut self, target: RenderTargetHandle, load: LoadAction) -> Result<()> {
        // 拒绝嵌套 pass，保持 FramePlan 的显式边界。
        self.rhi_device.pass.require_closed()?;
        // 清理契约必须在建立 pass 或绑定原生目标前完成 fail-stop 验收。
        if matches!(load, LoadAction::Clear(_)) {
            // D3D11 整目标清理只接受其固有行为能够精确表达的共享状态。
            rhi_device_clear::validate_color_clear_contract(UIX_COLOR_CLEAR_CONTRACT)?;
        }
        // 解析 surface 或离屏纹理目标，同时复制 COM view 避免借用跨越状态更新。
        let (rtv, extent) =
            if target.raw() == RHI_SURFACE_TARGET_RAW {
                // surface target 使用当前 swapchain backbuffer。
                self.ensure_rtv()?;
                // 没有 RTV 就不能开始 surface pass。
                let rtv =
                    self.rtv.as_ref().cloned().ok_or_else(|| {
                        rhi_platform("D3d11 RHI surface has no render target view")
                    })?;
                // surface extent 以当前物理 drawable 尺寸为准。
                let extent = RhiExtent::new(self.width.max(1) as u32, self.height.max(1) as u32);
                (rtv, extent)
            } else {
                // 离屏 target 必须指向已创建且可渲染的 RHI texture。
                let texture = self.rhi_device.target_texture(target.raw())?;
                // 复制 texture 的 RTV 和尺寸后再修改 pass 状态。
                let rtv =
                    texture.rtv.as_ref().cloned().ok_or_else(|| {
                        rhi_not_implemented("D3d11 RHI target texture render view")
                    })?;
                (rtv, texture.extent)
            };
        // 由共享状态机统一验证目标范围、清屏颜色并建立 pass 事实。
        self.rhi_device.pass.begin(target, extent, load)?;
        // 绑定本 pass 的唯一 render target。
        // SAFETY: rtv 来自当前 device 的 swapchain 或 texture，数组在同步调用期间存活且 context 位于 owner thread。
        unsafe {
            self.context
                .OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
        }
        // 只在明确要求时清理目标，Load 保留底层既有内容。
        if let LoadAction::Clear(color) = load {
            // Adapter 只读取共享层已经验证并预乘的目标颜色。
            let components = color.components();
            // SAFETY: rtv 属于当前 D3D11 device，颜色值已由 FramePlan 验证有限。
            unsafe {
                self.context.ClearRenderTargetView(&rtv, &components);
            }
        }
        // 只记录 Adapter 编码后续 clear/blur 所需的原生目标 view。
        self.rhi_device.active_target = Some(rtv);
        // 返回 pass 开始成功。
        Ok(())
    }

    // 绑定当前 pass 的采样纹理和 sampler。
    fn bind_sampled_texture(&mut self, binding: SampledTextureBinding) -> Result<()> {
        // 把绑定校验和状态保存委托给 resource helper。
        self.rhi_bind_sampled_texture(binding)
    }

    // 设置当前 pass viewport。
    fn set_viewport(&mut self, viewport: RhiViewport) -> Result<()> {
        // 把状态编码委托给按文件拆分的 RHI state helper。
        self.rhi_set_viewport(viewport)
    }

    // 设置当前 pass scissor。
    fn set_scissor(&mut self, scissor: Option<RhiScissor>) -> Result<()> {
        // 把状态编码委托给按文件拆分的 RHI state helper。
        self.rhi_set_scissor(scissor)
    }

    // 在当前 D3D11 render pass 内清理一个物理矩形。
    fn clear_rect(&mut self, color: RhiColor, scissor: RhiScissor) -> Result<()> {
        // 把 API 细节委托给独立的 ClearView helper。
        self.rhi_clear_rect(color, scissor)
    }

    // 执行一个已经选择固定 pipeline 与资源句柄的通用 draw packet。
    fn draw(&mut self, packet: DrawPacket) -> Result<()> {
        // 把固定 ABI 分派委托给独立模块，保持资源主文件短小。
        self.draw_rhi_packet(packet)
    }

    // 在 pass 外复制两个 RHI texture。
    fn copy_texture(&mut self, copy: TextureCopy) -> Result<()> {
        // 复制必须位于显式 pass 之外，避免 render target 和 copy source 重叠。
        self.rhi_device.pass.require_closed()?;
        // 读取源和目标资源。
        let source = self.rhi_device.texture(copy.source())?;
        let destination = self.rhi_device.texture(copy.destination())?;
        // 格式、非空、范围与资源关系全部由共享传输契约验证。
        let bounds = copy.validate_transfer(
            // 构造源纹理的 API 无关描述。
            TextureDesc {
                // 保存源物理尺寸。
                extent: source.extent,
                // 保存源格式。
                format: source.format,
            },
            // 构造目标纹理的 API 无关描述。
            TextureDesc {
                // 保存目标物理尺寸。
                extent: destination.extent,
                // 保存目标格式。
                format: destination.format,
            },
        )?;
        // 读取共享源区域已经验证的四条无符号边。
        let (source_left, source_top, source_right, source_bottom) =
            bounds.source().native_rect_u32();
        // 读取共享目标区域的左上原点。
        let destination_origin = bounds.destination().region().origin();
        // 构造源纹理复制区域。
        let source_box = D3D11_BOX {
            // 左边界来自类型化源区域。
            left: source_left,
            // 右边界来自共享 checked 加法。
            right: source_right,
            // 顶边界来自类型化源区域。
            top: source_top,
            // 底边界来自共享 checked 加法。
            bottom: source_bottom,
            // 二维纹理从唯一深度切片开始。
            front: 0,
            // 二维纹理只复制一个深度切片。
            back: 1,
        };
        // SAFETY: 两个 texture 均由同一 D3D11 device 创建，区域经过边界验证，
        // immediate context 只在 owner thread 上使用。
        unsafe {
            self.context.CopySubresourceRegion(
                &destination.native,
                0,
                destination_origin.x(),
                destination_origin.y(),
                0,
                &source.native,
                0,
                Some(&source_box),
            );
        }
        // 返回复制成功。
        Ok(())
    }

    // 在 D3D11 上执行同纹理重叠安全的区域移动。
    fn move_texture_region(&mut self, movement: TextureMove) -> Result<()> {
        // 移动必须发生在显式 pass 之外。
        self.rhi_device.pass.require_closed()?;
        // 先复制资源描述，避免后续 scratch 操作持有资源表借用。
        let (source_extent, source_format) = {
            // 读取源纹理的尺寸和格式事实。
            let source = self.rhi_device.texture(movement.source())?;
            (source.extent, source.format)
        };
        // 读取目标纹理的尺寸和格式事实。
        let (destination_extent, destination_format) = {
            // 读取目标纹理的尺寸和格式事实。
            let destination = self.rhi_device.texture(movement.destination())?;
            (destination.extent, destination.format)
        };
        // 格式、非空、范围和溢出统一委托共享 move 契约。
        movement.validate_transfer(
            // 构造源纹理的 API 无关描述。
            TextureDesc {
                // 保存源物理尺寸。
                extent: source_extent,
                // 保存源格式。
                format: source_format,
            },
            // 构造目标纹理的 API 无关描述。
            TextureDesc {
                // 保存目标物理尺寸。
                extent: destination_extent,
                // 保存目标格式。
                format: destination_format,
            },
        )?;
        // 不同纹理没有重叠风险，复用已验证的 copy 原语。
        if movement.source() != movement.destination() {
            // 将完整类型化移动无损转换为普通纹理复制。
            return self.copy_texture(movement.into_copy());
        }
        // 同一纹理必须先复制到 scratch，不能依赖 CopySubresourceRegion 的重叠行为。
        let scratch = self.rhi_create_texture(TextureDesc {
            // scratch 精确采用传输 Component 的唯一尺寸。
            extent: movement.transfer().extent(),
            format: source_format,
        })?;
        // 由共享传输 Component 唯一拆分保存与恢复两段 copy。
        let (to_scratch, from_scratch) = movement.through_scratch(scratch);
        // 先保存源区域，再写回目标区域，形成明确的 memmove 顺序。
        let operation = self.copy_texture(to_scratch).and_then(|()| {
            // 将 scratch 的完整区域写入目标位置。
            self.copy_texture(from_scratch)
        });
        // 无论第二次 copy 是否失败，都尝试销毁 scratch，避免隐藏资源泄漏。
        let cleanup = self.destroy_texture(scratch);
        // 优先返回移动错误，再返回 scratch 清理错误。
        match (operation, cleanup) {
            // 移动失败时保留原始错误。
            (Err(error), _) => Err(error),
            // 清理失败也不能伪造移动完整成功。
            (Ok(()), Err(error)) => Err(error),
            // 移动和临时资源清理均成功。
            (Ok(()), Ok(())) => Ok(()),
        }
    }

    // 结束当前 D3D11 render pass。
    fn end_render_pass(&mut self) -> Result<()> {
        // 收尾实现拆在独立 submit 模块，保持本文件处于行数上限内。
        self.end_render_pass_impl()
    }

    // 提交 D3D11 immediate context 当前命令序列。
    fn submit(&mut self) -> Result<crate::native::present::rhi::SubmissionHandle> {
        // 提交实现拆在独立 submit 模块，保持本文件处于行数上限内。
        self.submit_impl()
    }
}

// 生成 D3D11 RHI 参数错误。
fn rhi_invalid(message: &'static str) -> Error {
    // 使用统一错误码，避免把资源句柄错误伪装为平台崩溃。
    Error::new(Errc::InvalidArgument, message)
}

// 生成 D3D11 RHI 未实现错误。
fn rhi_not_implemented(operation: &'static str) -> Error {
    // 使用稳定前缀区分迁移缺口和驱动失败。
    Error::new(
        Errc::NotImplemented,
        format!("D3d11 thin RHI operation is not implemented: {operation}"),
    )
}

// 生成 D3D11 RHI 平台错误。
fn rhi_platform(message: &'static str) -> Error {
    // 资源创建失败属于平台资源错误而不是参数错误。
    Error::new(Errc::PlatformError, message)
}

// 将 Windows HRESULT 错误转换到 D3D11 现有诊断语义。
fn d3d_error(operation: &'static str, error: ::windows::core::Error) -> Error {
    // 保持与现有 D3D11 pipeline 相同的可观察错误前缀。
    Error::new(
        Errc::PlatformError,
        format!("D3d11 RHI {operation} failed: {error}"),
    )
}
