//! OpenGL ES 3.0 薄 RHI 的资源、pass 与提交状态。

// 引入父 raster 模块的 OpenGL 错误辅助。
use super::gl_error;
// 引入 glow 的上下文扩展方法。
use glow::HasContext as _;

// 引入统一错误、结果和 RHI 原语。
use crate::core::error::{Errc, Error, Result};
use crate::native::present::rhi::{
    BufferDesc, BufferHandle, BufferUsage, LoadAction, PipelineDesc, PipelineHandle,
    RenderTargetHandle, RhiColor, RhiExtent, RhiScissor, RhiViewport, SamplerDesc, SamplerHandle,
    SubmissionHandle, TextureCopy, TextureDesc, TextureFormat, TextureHandle, TextureMove,
};

// 将 retained 区域移动拆出，保持资源设备文件低于行数上限。
#[path = "rhi_device_copy.rs"]
mod rhi_device_copy;

// 保留一份 OpenGL ES 资源的通用描述和 CPU 更新镜像。
struct OpenGlRhiBuffer {
    // 保存底层 GL buffer 对象。
    native: glow::Buffer,
    // 保存通用 buffer 容量。
    size_bytes: usize,
    // 保存顶点或索引步长。
    stride_bytes: u32,
    // 保存资源用途，draw 阶段据此校验 ABI。
    usage: BufferUsage,
    // 保存最近一次上传的数据，uniform 通过它解码为 GL uniforms。
    data: Vec<u8>,
}

// 保存 OpenGL ES texture 及其可选 render target framebuffer。
struct OpenGlRhiTexture {
    // 保存底层 GL texture 对象。
    native: glow::Texture,
    // 保存颜色 texture 对应的 framebuffer，R8 coverage 不创建它。
    framebuffer: Option<glow::Framebuffer>,
    // 保存通用 texture extent。
    extent: RhiExtent,
    // 保存通用 texture 格式。
    format: TextureFormat,
}

// 保存固定 pipeline key 与编译后的 GL program。
struct OpenGlRhiPipeline {
    // 保存通用 renderer 选择的稳定 key。
    key: u64,
    // 保存 OpenGL ES program 对象。
    program: glow::Program,
}

// 保存固定 clamp sampler。
struct OpenGlRhiSampler {
    // 保存 OpenGL ES sampler 对象。
    native: glow::Sampler,
}

// OpenGL ES 的 swapchain target 身份不占用 texture 句柄空间。
pub(super) const RHI_SURFACE_TARGET_RAW: u64 = u64::MAX;

// 持有 OpenGL ES RHI 的资源表、pass 状态和 VAO。
pub(super) struct OpenGlRhiDevice {
    // 保存按一开始从 1 分配的 buffer 句柄索引的资源表。
    buffers: Vec<Option<OpenGlRhiBuffer>>,
    // 保存按一开始从 1 分配的 texture 句柄索引的资源表。
    textures: Vec<Option<OpenGlRhiTexture>>,
    // 保存按一开始从 1 分配的 pipeline 句柄索引的资源表。
    pipelines: Vec<Option<OpenGlRhiPipeline>>,
    // 保存按一开始从 1 分配的 sampler 句柄索引的资源表。
    samplers: Vec<Option<OpenGlRhiSampler>>,
    // 保存所有 RHI draw 共用的 VAO。
    vao: glow::VertexArray,
    // 保存 surface 是否需要 Y 翻转（Wayland EGL 为 top-left 行序，WGL 为 bottom-up）。
    flip_y: bool,
    // 保存是否已经打开一个 render pass。
    pass_open: bool,
    // 保存当前 pass 的 target 句柄。
    active_target: Option<RenderTargetHandle>,
    // 保存当前 pass 的物理 extent。
    active_extent: Option<RhiExtent>,
    // 保存当前 pass 的 RHI scissor，供局部清理后恢复原状态。
    scissor: Option<RhiScissor>,
    // 保存当前 pass 最近绑定的 texture。
    bound_texture: Option<TextureHandle>,
    // 保存当前 pass 最近绑定的 sampler。
    bound_sampler: Option<SamplerHandle>,
    // 保存下一个 submit 的不透明序号。
    next_submission: u64,
    // 保存最近一次成功 submit 的序号，供 surface present 校验。
    last_submission: Option<SubmissionHandle>,
    // 保存 test-harness 安排的一次设备丢失。
    #[cfg(feature = "test-harness")]
    device_lost_for_test: bool,
    // 保存 test-harness 安排的一次 surface 丢失。
    #[cfg(feature = "test-harness")]
    surface_lost_for_test: bool,
}

// 引入固定 draw ABI 的 OpenGL ES 分派实现。
#[path = "rhi_device_draw.rs"]
mod draw;

// 将 shader 选择与 program 编译拆到独立文件，保持资源表文件的行数边界。
#[path = "rhi_device_pipeline.rs"]
mod pipeline;

// 将整块和子区域 texture 上传拆到独立文件，保持资源表与上传语义分离。
#[path = "rhi_device_upload.rs"]
mod upload;

// 将局部颜色清理拆出，保持 OpenGL 资源主文件低于行数上限。
#[path = "rhi_device_clear.rs"]
mod clear;
// 将 OpenGL RHI 健康与 test-harness 故障安排拆出，保持资源表职责单一。
#[path = "rhi_device_health.rs"]
mod health;

// 引入拆分后的固定 shader ABI 辅助。
use pipeline::{compile_program, shader_sources};

// 为 OpenGL ES RHI 提供初始资源表和 VAO。
impl OpenGlRhiDevice {
    // 在已经 current 的 GLES 3 context 中创建共享 VAO。
    pub(super) fn new(gl: &glow::Context, flip_y: bool) -> Result<Self> {
        // 创建 VAO 失败必须阻止 context 进入 RHI 路径。
        let vao = unsafe {
            gl.create_vertex_array()
                .map_err(|error| gl_error("create RHI vertex array", error))?
        };
        // 返回没有打开 pass 的 owner-thread 状态。
        Ok(Self {
            buffers: Vec::new(),
            textures: Vec::new(),
            pipelines: Vec::new(),
            samplers: Vec::new(),
            vao,
            // 保存平台 surface 行序约定（Wayland EGL 需要翻转）。
            flip_y,
            pass_open: false,
            active_target: None,
            active_extent: None,
            // 初始没有启用任何 scissor。
            scissor: None,
            bound_texture: None,
            bound_sampler: None,
            next_submission: 1,
            last_submission: None,
            // 默认不安排测试设备丢失。
            #[cfg(feature = "test-harness")]
            device_lost_for_test: false,
            // 默认不安排测试 surface 丢失。
            #[cfg(feature = "test-harness")]
            surface_lost_for_test: false,
        })
    }

    // 把从 1 开始的 opaque handle 转换为资源表索引。
    fn index(raw: u64, kind: &'static str) -> Result<usize> {
        // 零句柄不能代表已经创建的资源。
        raw.checked_sub(1)
            .map(|value| value as usize)
            .ok_or_else(|| rhi_invalid(format!("OpenGL RHI {kind} handle is null")))
    }

    // 读取一个已经存在的 buffer。
    fn buffer(&self, handle: BufferHandle) -> Result<&OpenGlRhiBuffer> {
        // 先校验句柄的零值和索引转换。
        let index = Self::index(handle.raw(), "buffer")?;
        // 拒绝越界或已经销毁的槽位。
        self.buffers
            .get(index)
            .and_then(Option::as_ref)
            .ok_or_else(|| rhi_invalid("OpenGL RHI buffer handle is stale"))
    }

    // 读取一个可变 buffer。
    fn buffer_mut(&mut self, handle: BufferHandle) -> Result<&mut OpenGlRhiBuffer> {
        // 先校验句柄的零值和索引转换。
        let index = Self::index(handle.raw(), "buffer")?;
        // 拒绝越界或已经销毁的槽位。
        self.buffers
            .get_mut(index)
            .and_then(Option::as_mut)
            .ok_or_else(|| rhi_invalid("OpenGL RHI buffer handle is stale"))
    }

    // 读取一个已经存在的 texture。
    fn texture(&self, handle: TextureHandle) -> Result<&OpenGlRhiTexture> {
        // 先校验句柄的零值和索引转换。
        let index = Self::index(handle.raw(), "texture")?;
        // 拒绝越界或已经销毁的槽位。
        self.textures
            .get(index)
            .and_then(Option::as_ref)
            .ok_or_else(|| rhi_invalid("OpenGL RHI texture handle is stale"))
    }

    // 读取一个已经存在的 pipeline。
    fn pipeline(&self, handle: PipelineHandle) -> Result<&OpenGlRhiPipeline> {
        // 先校验句柄的零值和索引转换。
        let index = Self::index(handle.raw(), "pipeline")?;
        // 拒绝越界或已经销毁的槽位。
        self.pipelines
            .get(index)
            .and_then(Option::as_ref)
            .ok_or_else(|| rhi_invalid("OpenGL RHI pipeline handle is stale"))
    }

    // 读取一个已经存在的 sampler。
    fn sampler(&self, handle: SamplerHandle) -> Result<&OpenGlRhiSampler> {
        // 先校验句柄的零值和索引转换。
        let index = Self::index(handle.raw(), "sampler")?;
        // 拒绝越界或已经销毁的槽位。
        self.samplers
            .get(index)
            .and_then(Option::as_ref)
            .ok_or_else(|| rhi_invalid("OpenGL RHI sampler handle is stale"))
    }

    // 将通用 texture format 转换为 GLES 3.0 的内部格式和上传格式。
    fn texture_format(format: TextureFormat) -> (i32, u32, usize) {
        // 颜色 texture 统一用 RGBA8 存储，BGRA 由 shader 做通道交换。
        match format {
            // BGRA payload 按四字节紧密排列上传到 RGBA8。
            TextureFormat::Bgra8Unorm => (glow::RGBA8 as i32, glow::RGBA, 4),
            // RGBA MSDF 和离屏颜色 texture 保持通道原序。
            TextureFormat::Rgba8Unorm => (glow::RGBA8 as i32, glow::RGBA, 4),
            // coverage 使用 GLES 3.0 的单通道 R8 纹理。
            TextureFormat::R8Unorm => (glow::R8 as i32, glow::RED, 1),
        }
    }

    // 为 render target texture 创建并检查颜色 framebuffer。
    fn create_framebuffer(gl: &glow::Context, texture: glow::Texture) -> Result<glow::Framebuffer> {
        // 创建 framebuffer 对象。
        let framebuffer = unsafe {
            gl.create_framebuffer()
                .map_err(|error| gl_error("create RHI framebuffer", error))?
        };
        // 把 texture 接到颜色附件并检查完整性。
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(texture),
                0,
            );
        }
        // 读取当前 framebuffer 完整性状态。
        let complete =
            unsafe { gl.check_framebuffer_status(glow::FRAMEBUFFER) } == glow::FRAMEBUFFER_COMPLETE;
        // 不让资源表保留不完整的 framebuffer。
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        }
        if !complete {
            // 释放不完整 framebuffer 并返回平台错误。
            unsafe {
                gl.delete_framebuffer(framebuffer);
            }
            return Err(Error::new(
                Errc::PlatformError,
                "OpenGL RHI framebuffer is incomplete",
            ));
        }
        // 返回已经检查过的颜色 framebuffer。
        Ok(framebuffer)
    }

    // 创建动态 buffer 并登记其 CPU 镜像。
    pub(super) fn create_buffer(
        &mut self,
        gl: &glow::Context,
        desc: BufferDesc,
    ) -> Result<BufferHandle> {
        // 拒绝零容量和超过 GL 可表达范围的 buffer。
        if desc.size_bytes == 0 || desc.size_bytes > i32::MAX as usize {
            return Err(rhi_invalid("OpenGL RHI buffer size is invalid"));
        }
        // 顶点和索引步长必须能被 draw ABI 使用。
        if desc.usage != BufferUsage::Uniform && desc.stride_bytes == 0 {
            return Err(rhi_invalid("OpenGL RHI vertex/index stride is invalid"));
        }
        // 创建底层 buffer。
        let native = unsafe {
            gl.create_buffer()
                .map_err(|error| gl_error("create RHI buffer", error))?
        };
        // 为所有通用 buffer 分配固定字节容量；uniform 由 CPU 镜像解码。
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(native));
            gl.buffer_data_size(
                glow::ARRAY_BUFFER,
                desc.size_bytes as i32,
                glow::DYNAMIC_DRAW,
            );
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }
        // 保存资源事实和初始化为零的 CPU 镜像。
        self.buffers.push(Some(OpenGlRhiBuffer {
            native,
            size_bytes: desc.size_bytes,
            stride_bytes: desc.stride_bytes,
            usage: desc.usage,
            data: vec![0; desc.size_bytes],
        }));
        // 句柄从一开始递增，零值永远表示无资源。
        Ok(BufferHandle::from_raw(self.buffers.len() as u64))
    }

    // 更新 buffer 的指定字节范围，并同步 CPU uniform 镜像。
    pub(super) fn update_buffer(
        &mut self,
        gl: &glow::Context,
        handle: BufferHandle,
        offset: usize,
        data: &[u8],
    ) -> Result<()> {
        // 先验证目标范围不会越过资源容量。
        let buffer = self.buffer_mut(handle)?;
        let end = offset
            .checked_add(data.len())
            .ok_or_else(|| rhi_invalid("OpenGL RHI buffer update overflows"))?;
        if end > buffer.size_bytes {
            return Err(rhi_invalid("OpenGL RHI buffer update is out of bounds"));
        }
        // 写入 CPU 镜像，供随后 draw 的 uniform 解码使用。
        buffer.data[offset..end].copy_from_slice(data);
        // 上传同一范围到 GLES buffer。
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer.native));
            gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, offset as i32, data);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }
        // 返回统一成功结果。
        Ok(())
    }

    // 创建 sampled texture，并为颜色格式创建 render target framebuffer。
    pub(super) fn create_texture(
        &mut self,
        gl: &glow::Context,
        desc: TextureDesc,
    ) -> Result<TextureHandle> {
        // 拒绝空 extent。
        if !desc.extent.is_positive() {
            return Err(rhi_invalid("OpenGL RHI texture extent is invalid"));
        }
        // 取得 GL 内部格式、上传格式和每像素字节数。
        let (internal, upload_format, _) = Self::texture_format(desc.format);
        // 创建底层 texture。
        let native = unsafe {
            gl.create_texture()
                .map_err(|error| gl_error("create RHI texture", error))?
        };
        // 分配紧密排列的二维 texture 存储并设置 clamp sampler 默认值。
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(native));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                internal,
                desc.extent.width as i32,
                desc.extent.height as i32,
                0,
                upload_format,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.bind_texture(glow::TEXTURE_2D, None);
        }
        // R8 coverage 只作为 sampled texture，颜色格式才可作为目标。
        let framebuffer = if desc.format == TextureFormat::R8Unorm {
            None
        } else {
            Some(Self::create_framebuffer(gl, native)?)
        };
        // 保存资源并返回新的 opaque texture 句柄。
        self.textures.push(Some(OpenGlRhiTexture {
            native,
            framebuffer,
            extent: desc.extent,
            format: desc.format,
        }));
        Ok(TextureHandle::from_raw(self.textures.len() as u64))
    }

    // 创建 clamp sampler。
    pub(super) fn create_sampler(
        &mut self,
        gl: &glow::Context,
        desc: SamplerDesc,
    ) -> Result<SamplerHandle> {
        // 创建 sampler 对象。
        let native = unsafe {
            gl.create_sampler()
                .map_err(|error| gl_error("create RHI sampler", error))?
        };
        // 让 sampler 的过滤与 texture 的边界策略可被 draw 复用。
        unsafe {
            let filter = if desc.linear {
                glow::LINEAR as i32
            } else {
                glow::NEAREST as i32
            };
            gl.sampler_parameter_i32(native, glow::TEXTURE_MIN_FILTER, filter);
            gl.sampler_parameter_i32(native, glow::TEXTURE_MAG_FILTER, filter);
            gl.sampler_parameter_i32(native, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
            gl.sampler_parameter_i32(native, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);
        }
        // 记录 sampler 资源。
        self.samplers.push(Some(OpenGlRhiSampler { native }));
        // 返回从一开始递增的 sampler 句柄。
        Ok(SamplerHandle::from_raw(self.samplers.len() as u64))
    }

    // 编译并登记一个固定 pipeline。
    pub(super) fn create_pipeline(
        &mut self,
        gl: &glow::Context,
        desc: PipelineDesc,
    ) -> Result<PipelineHandle> {
        // 为 key 选择已经固定的 GLES 3.0 shader ABI（Wayland EGL 需要去掉 Y 翻转）。
        let (vertex, fragment) = shader_sources(desc.key, self.flip_y)
            .ok_or_else(|| rhi_not_implemented("OpenGL RHI pipeline key"))?;
        // 编译 program；shader 失败时不登记半成品资源。
        let program = unsafe { compile_program(gl, &vertex, fragment, "RHI pipeline")? };
        // 保存 key 和 program 的 owner-thread 生命周期。
        self.pipelines.push(Some(OpenGlRhiPipeline {
            key: desc.key,
            program,
        }));
        // 返回新的 pipeline 句柄。
        Ok(PipelineHandle::from_raw(self.pipelines.len() as u64))
    }

    // 销毁 buffer 资源。
    pub(super) fn destroy_buffer(
        &mut self,
        gl: &glow::Context,
        handle: BufferHandle,
    ) -> Result<()> {
        // 取得目标槽位。
        let index = Self::index(handle.raw(), "buffer")?;
        let Some(slot) = self.buffers.get_mut(index) else {
            return Err(rhi_invalid("OpenGL RHI buffer handle is stale"));
        };
        // 只能销毁仍然存在的资源。
        let Some(buffer) = slot.take() else {
            return Err(rhi_invalid("OpenGL RHI buffer was already destroyed"));
        };
        // 删除底层对象。
        unsafe {
            gl.delete_buffer(buffer.native);
        }
        // 返回统一成功结果。
        Ok(())
    }

    // 销毁 texture 资源及其 render target framebuffer。
    pub(super) fn destroy_texture(
        &mut self,
        gl: &glow::Context,
        handle: TextureHandle,
    ) -> Result<()> {
        // 当前 pass 不能销毁正在绑定的目标。
        if self
            .active_target
            .is_some_and(|target| target.raw() == handle.raw())
        {
            return Err(rhi_invalid("OpenGL RHI cannot destroy active target"));
        }
        // 取得目标槽位。
        let index = Self::index(handle.raw(), "texture")?;
        let Some(slot) = self.textures.get_mut(index) else {
            return Err(rhi_invalid("OpenGL RHI texture handle is stale"));
        };
        // 只能销毁仍然存在的资源。
        let Some(texture) = slot.take() else {
            return Err(rhi_invalid("OpenGL RHI texture was already destroyed"));
        };
        // 删除 framebuffer 和 texture 对象。
        unsafe {
            if let Some(framebuffer) = texture.framebuffer {
                gl.delete_framebuffer(framebuffer);
            }
            gl.delete_texture(texture.native);
        }
        // 清理可能残留的采样绑定。
        if self.bound_texture == Some(handle) {
            self.bound_texture = None;
        }
        // 返回统一成功结果。
        Ok(())
    }

    // 销毁 sampler 资源。
    pub(super) fn destroy_sampler(
        &mut self,
        gl: &glow::Context,
        handle: SamplerHandle,
    ) -> Result<()> {
        // 取得目标槽位。
        let index = Self::index(handle.raw(), "sampler")?;
        let Some(slot) = self.samplers.get_mut(index) else {
            return Err(rhi_invalid("OpenGL RHI sampler handle is stale"));
        };
        // 只能销毁仍然存在的资源。
        let Some(sampler) = slot.take() else {
            return Err(rhi_invalid("OpenGL RHI sampler was already destroyed"));
        };
        // 删除底层 sampler 对象。
        unsafe {
            gl.delete_sampler(sampler.native);
        }
        // 清理可能残留的采样绑定。
        if self.bound_sampler == Some(handle) {
            self.bound_sampler = None;
        }
        // 返回统一成功结果。
        Ok(())
    }

    // 销毁 pipeline 资源。
    pub(super) fn destroy_pipeline(
        &mut self,
        gl: &glow::Context,
        handle: PipelineHandle,
    ) -> Result<()> {
        // 取得目标槽位。
        let index = Self::index(handle.raw(), "pipeline")?;
        let Some(slot) = self.pipelines.get_mut(index) else {
            return Err(rhi_invalid("OpenGL RHI pipeline handle is stale"));
        };
        // 只能销毁仍然存在的资源。
        let Some(pipeline) = slot.take() else {
            return Err(rhi_invalid("OpenGL RHI pipeline was already destroyed"));
        };
        // 删除底层 program。
        unsafe {
            gl.delete_program(pipeline.program);
        }
        // 返回统一成功结果。
        Ok(())
    }

    // 开始一个 RHI render pass 并绑定目标 framebuffer。
    pub(super) fn begin_render_pass(
        &mut self,
        gl: &glow::Context,
        target: RenderTargetHandle,
        load: LoadAction,
        surface_extent: RhiExtent,
    ) -> Result<()> {
        // 禁止 pass 嵌套，确保 FramePlan 顺序有唯一 owner。
        if self.pass_open {
            return Err(rhi_invalid("OpenGL RHI render pass is already open"));
        }
        // 解析 surface 或离屏 texture target。
        let (framebuffer, extent) = if target.raw() == RHI_SURFACE_TARGET_RAW {
            (None, surface_extent)
        } else {
            let texture = self.texture(TextureHandle::from_raw(target.raw()))?;
            let framebuffer = texture
                .framebuffer
                .ok_or_else(|| rhi_invalid("OpenGL RHI target texture is not renderable"))?;
            (Some(framebuffer), texture.extent)
        };
        // target 的 extent 必须为正。
        if !extent.is_positive() {
            return Err(rhi_invalid("OpenGL RHI render target extent is invalid"));
        }
        // 绑定 framebuffer 并清理按 pass 指定的颜色。
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, framebuffer);
            if let LoadAction::Clear(RhiColor(color)) = load {
                gl.disable(glow::SCISSOR_TEST);
                gl.clear_color(color[0], color[1], color[2], color[3]);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
        }
        // 保存 pass 状态并清掉上一个 pass 的采样绑定。
        self.pass_open = true;
        self.active_target = Some(target);
        self.active_extent = Some(extent);
        // 每个 pass 从未启用 scissor 开始，避免跨 pass 泄漏裁剪状态。
        self.scissor = None;
        self.bound_texture = None;
        self.bound_sampler = None;
        // 返回统一成功结果。
        Ok(())
    }

    // 设置物理 viewport。
    pub(super) fn set_viewport(&self, gl: &glow::Context, viewport: RhiViewport) -> Result<()> {
        // 先验证跨 adapter 的 viewport 合约。
        if !self.pass_open || !viewport.is_valid() {
            return Err(rhi_invalid(
                "OpenGL RHI viewport is invalid or pass is closed",
            ));
        }
        // OpenGL viewport 使用自身坐标系；行序差异已由 shader 编译期处理（flip_y）。
        unsafe {
            gl.viewport(
                0,
                0,
                viewport.width.round() as i32,
                viewport.height.round() as i32,
            );
        }
        // 返回统一成功结果。
        Ok(())
    }

    // 设置或关闭左上原点 scissor。
    pub(super) fn set_scissor(
        &mut self,
        gl: &glow::Context,
        scissor: Option<RhiScissor>,
    ) -> Result<()> {
        // scissor 只能在 active pass 内设置。
        let extent = self
            .active_extent
            .ok_or_else(|| rhi_invalid("OpenGL RHI scissor has no active target"))?;
        // 按 RHI 的左上原点 ABI 校验并转换坐标。
        unsafe {
            if let Some(scissor) = scissor {
                if !scissor.is_valid()
                    || scissor.x.saturating_add(scissor.width) > extent.width as i32
                    || scissor.y.saturating_add(scissor.height) > extent.height as i32
                {
                    return Err(rhi_invalid("OpenGL RHI scissor is outside target"));
                }
                gl.enable(glow::SCISSOR_TEST);
                if self.flip_y {
                    // Wayland EGL：shader 不翻转后，UI 顶部映射到 GL 帧缓冲第 0 行
                    //（内存第 0 行 = 窗口顶部），RHI 左上原点坐标可直接使用。
                    gl.scissor(scissor.x, scissor.y, scissor.width, scissor.height);
                } else {
                    // WGL bottom-up DIB：UI 顶部映射到 GL 帧缓冲顶部行，
                    // 需把左上原点坐标换算为 GL 左下原点。
                    gl.scissor(
                        scissor.x,
                        extent.height as i32 - scissor.y - scissor.height,
                        scissor.width,
                        scissor.height,
                    );
                }
            } else {
                gl.disable(glow::SCISSOR_TEST);
            }
        }
        // 只有原生状态设置成功后才更新可恢复的 RHI 状态。
        self.scissor = scissor;
        // 返回统一成功结果。
        Ok(())
    }

    // 记录 texture/sampler 绑定，实际 GL 绑定在 draw 时完成。
    pub(super) fn bind_texture(
        &mut self,
        slot: u32,
        texture: TextureHandle,
        sampler: SamplerHandle,
    ) -> Result<()> {
        // 当前跨 adapter ABI 只开放 t0/s0。
        if !self.pass_open || slot != 0 {
            return Err(rhi_invalid("OpenGL RHI only supports texture slot zero"));
        }
        // 先验证资源仍然存在。
        self.texture(texture)?;
        self.sampler(sampler)?;
        // 禁止当前 render target 同时作为 sampled source，避免反馈环。
        if self
            .active_target
            .is_some_and(|target| target.raw() == texture.raw())
        {
            return Err(rhi_invalid("OpenGL RHI texture feedback loop is invalid"));
        }
        // 保存绑定身份。
        self.bound_texture = Some(texture);
        self.bound_sampler = Some(sampler);
        // 返回统一成功结果。
        Ok(())
    }

    // 结束当前 pass 并解除 framebuffer 绑定。
    pub(super) fn end_render_pass(&mut self, gl: &glow::Context) -> Result<()> {
        // 禁止在没有 pass 时结束。
        if !self.pass_open {
            return Err(rhi_invalid("OpenGL RHI render pass is not open"));
        }
        // 解除 texture/sampler 和 framebuffer 状态。
        unsafe {
            // 结束 pass 时显式关闭 scissor，避免下一 target 继承旧裁剪。
            gl.disable(glow::SCISSOR_TEST);
            gl.bind_sampler(0, None);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.bind_vertex_array(None);
        }
        // 清理 pass 状态。
        self.pass_open = false;
        self.active_target = None;
        self.active_extent = None;
        // 清理 pass 级 scissor，避免下一 pass 继承旧裁剪。
        self.scissor = None;
        self.bound_texture = None;
        self.bound_sampler = None;
        // 返回统一成功结果。
        Ok(())
    }

    // 执行一次 source texture 到 destination texture 的像素复制。
    pub(super) fn copy_texture(&mut self, gl: &glow::Context, copy: TextureCopy) -> Result<()> {
        // copy 必须发生在 pass 外。
        if self.pass_open {
            return Err(rhi_invalid(
                "OpenGL RHI texture copy is inside a render pass",
            ));
        }
        // 读取源和目标的描述，均必须为颜色 render target。
        let source = self.texture(copy.source)?;
        let destination = self.texture(copy.destination)?;
        if source.framebuffer.is_none() || destination.format == TextureFormat::R8Unorm {
            return Err(rhi_invalid(
                "OpenGL RHI texture copy requires color textures",
            ));
        }
        // 校验复制范围在两个 texture 内。
        let source_right = copy.source_x.saturating_add(copy.width);
        let source_bottom = copy.source_y.saturating_add(copy.height);
        let destination_right = copy.destination_x.saturating_add(copy.width);
        let destination_bottom = copy.destination_y.saturating_add(copy.height);
        if source_right > source.extent.width
            || source_bottom > source.extent.height
            || destination_right > destination.extent.width
            || destination_bottom > destination.extent.height
        {
            return Err(rhi_invalid("OpenGL RHI texture copy is outside extent"));
        }
        // 复制前绑定源 framebuffer 作为 READ_FRAMEBUFFER，并绑定目标 texture。
        unsafe {
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, source.framebuffer);
            gl.bind_texture(glow::TEXTURE_2D, Some(destination.native));
            gl.copy_tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                copy.destination_x as i32,
                destination.extent.height as i32 - destination_bottom as i32,
                copy.source_x as i32,
                source.extent.height as i32 - source_bottom as i32,
                copy.width as i32,
                copy.height as i32,
            );
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
        }
        // 返回统一成功结果。
        Ok(())
    }

    // 提交当前 owner-thread GL 命令并生成提交身份。
    pub(super) fn submit(&mut self, gl: &glow::Context) -> Result<SubmissionHandle> {
        // pass 必须已经关闭才能提交。
        if self.pass_open {
            return Err(rhi_invalid("OpenGL RHI submit has an open render pass"));
        }
        // 令驱动在最终 swap 前观察到当前命令序列。
        unsafe {
            gl.flush();
        }
        // 生成不透明提交序号并避免零值回绕。
        let raw = self.next_submission.max(1);
        self.next_submission = self.next_submission.saturating_add(1).max(1);
        let submission = SubmissionHandle::from_raw(raw);
        self.last_submission = Some(submission);
        // 返回提交身份。
        Ok(submission)
    }

    // 校验 surface present 使用的是最近一次成功 submit。
    pub(super) fn validate_submission(&self, submission: SubmissionHandle) -> Result<()> {
        // 零值和迟到提交都不能触发原生交换。
        if submission.raw() == 0 || self.last_submission != Some(submission) {
            return Err(rhi_invalid("OpenGL RHI present submission is stale"));
        }
        // 返回统一成功结果。
        Ok(())
    }

    // 释放所有 RHI 资源；调用方必须保证 GL context current。
    pub(super) fn release(&mut self, gl: &glow::Context) {
        // 按资源表逆序释放底层对象，避免依赖关系被提前拆除。
        for slot in self.pipelines.iter_mut().rev() {
            if let Some(pipeline) = slot.take() {
                unsafe {
                    gl.delete_program(pipeline.program);
                }
            }
        }
        // 先删除 texture framebuffer，再删除 texture。
        for slot in self.textures.iter_mut().rev() {
            if let Some(texture) = slot.take() {
                unsafe {
                    if let Some(framebuffer) = texture.framebuffer {
                        gl.delete_framebuffer(framebuffer);
                    }
                    gl.delete_texture(texture.native);
                }
            }
        }
        // 删除 sampler 资源。
        for slot in self.samplers.iter_mut().rev() {
            if let Some(sampler) = slot.take() {
                unsafe {
                    gl.delete_sampler(sampler.native);
                }
            }
        }
        // 删除 buffer 资源和共享 VAO。
        for slot in self.buffers.iter_mut().rev() {
            if let Some(buffer) = slot.take() {
                unsafe {
                    gl.delete_buffer(buffer.native);
                }
            }
        }
        unsafe {
            gl.delete_vertex_array(self.vao);
        }
        // 清理所有逻辑状态。
        self.pass_open = false;
        self.active_target = None;
        self.active_extent = None;
        // 资源释放时同步清理 scissor 的逻辑镜像。
        self.scissor = None;
        self.bound_texture = None;
        self.bound_sampler = None;
        self.last_submission = None;
    }
}

// 生成 OpenGL RHI 的参数错误。
fn rhi_invalid(message: impl Into<String>) -> Error {
    // 统一使用 InvalidArgument，保持 adapter 错误分类稳定。
    Error::new(Errc::InvalidArgument, message)
}

// 生成 OpenGL RHI 的未实现错误。
fn rhi_not_implemented(operation: &'static str) -> Error {
    // 让 bootstrap 和诊断能区分缺失 shader key 与平台故障。
    Error::new(
        Errc::NotImplemented,
        format!("OpenGL RHI operation is not implemented: {operation}"),
    )
}
