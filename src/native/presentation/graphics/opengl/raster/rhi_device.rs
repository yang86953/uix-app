//! OpenGL ES 3.0 薄 RHI 的资源、pass 与提交状态。

// 引入父 raster 模块的 OpenGL 错误辅助。
use super::gl_error;
// 引入 glow 的上下文扩展方法。
use glow::HasContext as _;

// 引入统一错误、结果和 RHI 原语。
use crate::core::error::{Errc, Error, Result};
// 引入 Surface 提供的跨呈现像素保留语义。
use crate::core::PresentCoherency;
use crate::platform::presentation::rhi::{
    BufferDesc, BufferHandle, DrawPacket, DrawRasterState, LoadAction, PipelineBinding,
    PipelineColorWriteMask, PipelineDesc, PipelineDitherState, PipelineKind, RenderTargetHandle,
    RhiBufferResource, RhiBufferResourceTable, RhiBufferUpload, RhiBufferUploadPreflight, RhiColor,
    RhiColorClearContract, RhiExtent, RhiPassState, RhiPipelineResourceTable,
    RhiPresentTransaction, RhiResourceTable, RhiScissor, RhiSubmissionSequence, RhiTextureResource,
    RhiTextureResourceTable, RhiTextureUpload, SampledTextureBinding, SamplerAddressMode,
    SamplerDesc, SamplerFilter, SamplerHandle, SamplerMipMode, SubmissionHandle, SurfaceToken,
    TextureCopy, TextureDesc, TextureFormat, TextureHandle, TextureMove, UIX_COLOR_CLEAR_CONTRACT,
    ValidatedRhiPresent,
};

// 将 retained 区域移动拆出，保持资源设备文件低于行数上限。
#[path = "rhi_device_copy.rs"]
mod rhi_device_copy;

// 保留一份 OpenGL ES 资源的通用描述和 CPU 更新镜像。
struct OpenGlRhiBuffer {
    // 保存底层 GL buffer 对象。
    native: glow::Buffer,
    // 保存已经通过共同门禁的完整 Buffer 描述。
    desc: BufferDesc,
    // 保存最近一次上传的数据，uniform 通过它解码为 GL uniforms。
    data: Vec<u8>,
}

// 让 OpenGL Buffer 资源向共享 Draw 预检提供创建时冻结的描述。
impl RhiBufferResource for OpenGlRhiBuffer {
    // 返回唯一共享 Buffer 描述，不重新解释用途或步长。
    fn desc(&self) -> BufferDesc {
        // 复制资源创建时保存的不可变描述。
        self.desc
    }
}

// 保存 OpenGL ES texture 及其可选 render target framebuffer。
struct OpenGlRhiTexture {
    // 保存底层 GL texture 对象。
    native: glow::Texture,
    // 保存颜色 texture 对应的 framebuffer，R8 coverage 不创建它。
    framebuffer: Option<glow::Framebuffer>,
    // 保存已经通过共同门禁的完整纹理描述。
    desc: TextureDesc,
}

// 让 OpenGL texture 资源向共享目标 resolver 提供创建时冻结的描述。
impl RhiTextureResource for OpenGlRhiTexture {
    // 返回唯一共享纹理描述，不重新解释原生格式能力。
    fn desc(&self) -> TextureDesc {
        // 复制资源创建时保存的不可变描述。
        self.desc
    }
}

// 保存编译后的 GL program。
struct OpenGlRhiPipeline {
    // 保存 OpenGL ES program 对象。
    program: glow::Program,
}

// 保存固定 clamp sampler。
struct OpenGlRhiSampler {
    // 保存 OpenGL ES sampler 对象。
    native: glow::Sampler,
    // 保存共享 pipeline draw 门禁需要的 API 无关过滤事实。
    desc: SamplerDesc,
}

// 返回左上逻辑坐标映射到当前 OpenGL 目标所需的 NDC Y 符号。
fn target_y_sign(target: RenderTargetHandle) -> f32 {
    // 原生 surface 的顶部位于 GL framebuffer 高 Y，texture 的 top-left 行位于低 Y。
    if target.is_surface() { -1.0 } else { 1.0 }
}

// 持有 OpenGL ES RHI 的资源表、pass 状态和 VAO。
pub(super) struct OpenGlRhiDevice {
    // 保存按共享 Buffer 语义管理的原生资源表。
    buffers: RhiBufferResourceTable<OpenGlRhiBuffer>,
    // 保存按共享 texture 语义管理的原生 texture 资源表。
    textures: RhiTextureResourceTable<OpenGlRhiTexture>,
    // 保存必须活到下一次原生 submit 之后才能删除的临时移动纹理。
    texture_move_scratch_after_submit: Vec<TextureHandle>,
    // 保存按共享 pipeline 语义索引的原生 program 资源表。
    pipelines: RhiPipelineResourceTable<OpenGlRhiPipeline>,
    // 保存按一开始从 1 分配的 sampler 句柄索引的资源表。
    samplers: RhiResourceTable<SamplerHandle, OpenGlRhiSampler>,
    // 保存所有 RHI draw 共用的 VAO。
    vao: glow::VertexArray,
    // 保存两个 Adapter 共用的 pass 生命周期、目标与物理范围事实。
    pass: RhiPassState,
    // 保存共享的提交身份状态机，禁止 OpenGL 自行解释 submit/present 关联。
    submission_sequence: RhiSubmissionSequence,
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
// 将共享顶点布局的原生映射拆成独立 Adapter Component。
#[path = "rhi_device_vertex_layout.rs"]
mod vertex_layout;

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

// 将资源句柄校验与 framebuffer 创建下沉到同一 RHI 设备 Module。
#[path = "rhi_device_resources.rs"]
mod resources;

// 为 OpenGL ES RHI 提供初始资源表和 VAO。
impl OpenGlRhiDevice {
    // 在已经 current 的 GLES 3 context 中创建共享 VAO。
    pub(super) fn new(gl: &glow::Context) -> Result<Self> {
        // 创建 VAO 失败必须阻止 context 进入 RHI 路径。
        // SAFETY: 调用时 GL context 已由调用方设为 current；create_vertex_array 不接收指针参数，失败走错误返回而非 UB。
        let vao = unsafe {
            gl.create_vertex_array()
                .map_err(|error| gl_error("create RHI vertex array", error))?
        };
        // 返回没有打开 pass 的 owner-thread 状态。
        Ok(Self {
            // 由共享表统一管理 Buffer 句柄与冻结描述。
            buffers: RhiBufferResourceTable::new(),
            // 由共享表统一管理 texture 句柄与冻结描述。
            textures: RhiTextureResourceTable::new(),
            // 新设备尚未产生等待提交的临时移动资源。
            texture_move_scratch_after_submit: Vec::new(),
            // 由共享表统一保存 pipeline 句柄与 kind。
            pipelines: RhiPipelineResourceTable::new(),
            samplers: RhiResourceTable::new(),
            vao,
            // 使用 API 无关状态机初始化 pass 生命周期。
            pass: RhiPassState::new(),
            // 使用 API 无关状态机初始化提交序列。
            submission_sequence: RhiSubmissionSequence::new(),
            // 默认不安排测试设备丢失。
            #[cfg(feature = "test-harness")]
            device_lost_for_test: false,
            // 默认不安排测试 surface 丢失。
            #[cfg(feature = "test-harness")]
            surface_lost_for_test: false,
        })
    }

    // 从共享 texture 资源表提升一个已证明可渲染的目标身份。
    pub(super) fn resolve_render_target(
        // 只读借用 OpenGL 设备，不触碰 native GL 状态。
        &self,
        // 接收 GraphicsDevice 创建返回的 texture 句柄。
        texture: TextureHandle,
    ) -> Result<RenderTargetHandle> {
        // 共享表依据真实 texture 描述执行唯一能力门禁。
        self.textures.resolve_render_target(texture)
    }

    // 只读预检 DrawPacket 的真实 Buffer 与条件采样资源。
    pub(super) fn preflight_draw_resources(&self, packet: DrawPacket) -> Result<()> {
        // 先由共享 pipeline 表验证句柄存活及真实 kind 语义。
        self.pipelines.get(packet.pipeline())?;
        // 再由共享 Buffer 表验证固定资源角色和范围关系。
        self.buffers.validate_draw(packet)?;
        // 只有当前 packet 明确携带采样资源时才解析纹理与 sampler。
        if let Some(binding) = packet.sampling().sampled_texture() {
            // 条件资源必须由真实描述满足绑定时冻结的采样语义。
            self.validate_sampled_resources(binding)?;
        }
        // 当前 DrawPacket 的全部真实资源均已通过只读预检。
        Ok(())
    }

    // 只读预检普通 texture copy 的资源与传输关系。
    pub(super) fn preflight_texture_copy(&self, copy: TextureCopy) -> Result<()> {
        // 共享 texture 表验证真实描述，不访问 OpenGL context。
        self.textures.validate_copy(copy)
    }

    // 只读预检 texture move 的资源与传输关系。
    pub(super) fn preflight_texture_move(&self, movement: TextureMove) -> Result<()> {
        // 共享 texture 表验证真实描述，不访问 OpenGL context。
        self.textures.validate_move(movement)
    }

    // 只读预检 sampled texture 与 sampler 的真实资源语义。
    fn validate_sampled_resources(
        // 只读借用 OpenGL 设备，不触碰 pass 或 native 状态。
        &self,
        // 接收不可拆分的共享 sampled 绑定事实。
        binding: SampledTextureBinding,
    ) -> Result<()> {
        // 解析真实 texture 资源并结束资源表借用。
        let texture = self.texture(binding.texture())?;
        // 解析真实 sampler 资源并结束资源表借用。
        let sampler = self.sampler(binding.sampler())?;
        // 复制共享纹理格式，供绑定契约统一校验。
        let format = texture.desc.format();
        // 复制 sampler 描述，供绑定契约统一校验。
        let sampler_desc = sampler.desc;
        // 只执行资源语义预检，不建立任何 pass-local 绑定状态。
        binding.validate_resources(format, sampler_desc)
    }

    // 只读预检 Buffer 上传的真实资源身份与载荷契约。
    pub(super) fn preflight_buffer_upload(
        // 只读借用 OpenGL 设备，不触碰 GL context 或 native 状态。
        &self,
        // 接收共享层冻结的 Buffer 上传预检事实。
        upload: RhiBufferUploadPreflight,
    ) -> Result<()> {
        // 由共享 Buffer 资源表解析真实句柄并统一验证用途、元素 ABI、容量与范围。
        self.buffers.validate_upload(upload)
    }

    // 创建动态 buffer 并登记其 CPU 镜像。
    pub(super) fn create_buffer(
        &mut self,
        gl: &glow::Context,
        desc: BufferDesc,
    ) -> Result<BufferHandle> {
        // 先通过两个 Adapter 共用的容量、步长与 Uniform ABI 门禁。
        let native_desc = desc.validate()?;
        // 创建底层 buffer。
        // SAFETY: 本设备方法约定调用时 GL context current；create_buffer 无指针参数，失败走错误返回。
        let native = unsafe {
            gl.create_buffer()
                .map_err(|error| gl_error("create RHI buffer", error))?
        };
        // 为所有通用 buffer 分配固定字节容量；uniform 由 CPU 镜像解码。
        // SAFETY: native 刚创建且存活；容量已在上方验证非零且 ≤ i32::MAX；绑定与解绑在同步调用内成对完成；context 保持 current。
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(native));
            gl.buffer_data_size(
                glow::ARRAY_BUFFER,
                native_desc.size_bytes_i32(),
                glow::DYNAMIC_DRAW,
            );
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }
        // 由共享资源表原子登记资源事实和初始化为零的 CPU 镜像。
        Ok(self.buffers.insert(OpenGlRhiBuffer {
            native,
            // Adapter 只保存唯一共享描述，不再复制三个可漂移字段。
            desc,
            // CPU 镜像与已经验证的资源容量精确一致。
            data: vec![0; desc.size_bytes()],
        }))
    }

    // 更新 Buffer 从零开始的已验证元素前缀，并同步 CPU Uniform 镜像。
    pub(super) fn update_buffer(
        &mut self,
        gl: &glow::Context,
        upload: RhiBufferUpload<'_>,
    ) -> Result<()> {
        // 先解析目标身份，空载荷也不能绕过陈旧句柄门禁。
        let buffer = self.buffer_mut(upload.buffer())?;
        // 由共享 Component 验证前缀范围、元素边界和 Uniform 完整替换。
        let validated = upload.validate(buffer.desc)?;
        // 原生调用只消费已经验证的不可变字节。
        let data = validated.data();
        // 写入 CPU 镜像的同一前缀，供随后 draw 的 Uniform 解码使用。
        buffer.data[..data.len()].copy_from_slice(data);
        // 上传同一范围到 GLES buffer。
        // SAFETY: buffer.native 存活；共享上传门禁已验证前缀不越过资源容量；data 切片指向的有效内存贯穿整个同步调用；context 保持 current。
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer.native));
            gl.buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, data);
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
        // 取得两个 Adapter 共用的有符号原生尺寸。
        let (native_width, native_height) = desc.validate()?.size_i32();
        // 取得只包含原生枚举映射的 GL 内部格式与上传格式。
        let (internal, upload_format) = Self::texture_format(desc.format());
        // 创建底层 texture。
        // SAFETY: 调用时 GL context current；create_texture 无指针参数，失败走错误返回。
        let native = unsafe {
            gl.create_texture()
                .map_err(|error| gl_error("create RHI texture", error))?
        };
        // 分配紧密排列的二维 texture 存储并设置 clamp sampler 默认值。
        // SAFETY: native 刚创建且存活；extent 已投影为共同有符号正尺寸；PixelUnpackData::Slice(None) 不传递 CPU 指针；其余均为有效 GLES3 常量；context 保持 current。
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(native));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                internal,
                native_width,
                native_height,
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
        let framebuffer = if desc.format().supports_render_target() {
            // framebuffer 是 texture 登记前最后一个可能失败的原生创建步骤。
            let framebuffer = match Self::create_framebuffer(gl, native) {
                // 成功对象将与 texture 一起移交资源表。
                Ok(framebuffer) => framebuffer,
                // 失败时 texture 尚无资源表 owner，必须在返回前显式回收。
                Err(error) => {
                    // SAFETY: native 由本函数刚创建且尚未登记；framebuffer helper 已只回收自己的对象；context 保持 current。
                    unsafe {
                        // 删除本创建事务唯一拥有的未登记 texture。
                        gl.delete_texture(native);
                    }
                    // 保留 framebuffer 创建或完整性检查的原始错误。
                    return Err(error);
                }
            };
            // 成功 framebuffer 与 texture 作为一个颜色目标资源共同登记。
            Some(framebuffer)
        } else {
            None
        };
        // 由共享 texture 表登记资源并返回新的 opaque texture 句柄。
        Ok(self.textures.insert(OpenGlRhiTexture {
            native,
            framebuffer,
            // Adapter 只保存唯一共享描述，不再复制尺寸和格式字段。
            desc,
        }))
    }

    // 创建 clamp sampler。
    pub(super) fn create_sampler(
        &mut self,
        gl: &glow::Context,
        desc: SamplerDesc,
    ) -> Result<SamplerHandle> {
        // 创建 sampler 对象。
        // SAFETY: 调用时 GL context current；create_sampler 无指针参数，失败走错误返回。
        let native = unsafe {
            gl.create_sampler()
                .map_err(|error| gl_error("create RHI sampler", error))?
        };
        // 让 sampler 的过滤与 texture 的边界策略可被 draw 复用。
        // SAFETY: native 刚创建且存活；参数均为有效 GL 枚举且无指针；context 保持 current。
        unsafe {
            // 穷尽映射共享 min/mag 过滤语义。
            let filter = match desc.filter() {
                // 最近点采样直接映射为 GL_NEAREST。
                SamplerFilter::Nearest => glow::NEAREST as i32,
                // 线性采样直接映射为 GL_LINEAR。
                SamplerFilter::Linear => glow::LINEAR as i32,
            };
            // 穷尽映射共享二维地址语义。
            let address_mode = match desc.address_mode() {
                // 当前唯一地址模式机械映射为 GL_CLAMP_TO_EDGE。
                SamplerAddressMode::ClampToEdge => glow::CLAMP_TO_EDGE as i32,
            };
            // 穷尽映射共享单级 mip 语义。
            let (min_lod, max_lod) = match desc.mip_mode() {
                // 第零级是当前纹理资源唯一允许的 LOD。
                SamplerMipMode::SingleLevel => (0.0, 0.0),
            };
            // 把共享过滤语义应用到缩小路径。
            gl.sampler_parameter_i32(native, glow::TEXTURE_MIN_FILTER, filter);
            // 把同一共享过滤语义应用到放大路径。
            gl.sampler_parameter_i32(native, glow::TEXTURE_MAG_FILTER, filter);
            // U 轴只消费共享地址模式投影。
            gl.sampler_parameter_i32(native, glow::TEXTURE_WRAP_S, address_mode);
            // V 轴只消费同一共享地址模式投影。
            gl.sampler_parameter_i32(native, glow::TEXTURE_WRAP_T, address_mode);
            // 显式冻结允许的最小 LOD，避免依赖驱动默认值。
            gl.sampler_parameter_f32(native, glow::TEXTURE_MIN_LOD, min_lod);
            // 显式冻结允许的最大 LOD，禁止未来纹理层级引入隐式差异。
            gl.sampler_parameter_f32(native, glow::TEXTURE_MAX_LOD, max_lod);
        }
        // 由共享资源表登记 sampler 资源。
        Ok(self.samplers.insert(OpenGlRhiSampler {
            // 保持原生 sampler 的 owner-thread 生命周期。
            native,
            // 保留创建描述供共享 PipelineSampling 在 draw 前核对。
            desc,
        }))
    }

    // 编译并登记一个固定 pipeline。
    pub(super) fn create_pipeline(
        &mut self,
        gl: &glow::Context,
        desc: PipelineDesc,
    ) -> Result<PipelineBinding> {
        // 为封闭语义选择不再按平台改写的固定 GLES 3.0 shader ABI。
        let (vertex, fragment) = shader_sources(desc.kind);
        // 编译 program；shader 失败时不登记半成品资源。
        // SAFETY: 编译期间 context 保持 current（compile_program 自身的前置条件），shader 源码为存活且静态的生命周期字符串。
        let program = unsafe { compile_program(gl, vertex, fragment, "RHI pipeline")? };
        // 由共享资源表保存 program 并签发不可拆 pipeline 身份。
        let binding = self
            .pipelines
            .insert(desc.kind, OpenGlRhiPipeline { program });
        // 返回与资源表真实语义一致的完整绑定。
        Ok(binding)
    }

    // 销毁 buffer 资源。
    pub(super) fn destroy_buffer(
        &mut self,
        gl: &glow::Context,
        handle: BufferHandle,
    ) -> Result<()> {
        // 由共享资源表检查式取出仍然存活的 buffer。
        let buffer = self.buffers.take(handle)?;
        // 删除底层对象。
        // SAFETY: buffer 刚经 slot.take() 取出、仍存活且此后不再被引用，只在销毁路径删除一次；context 保持 current。
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
        // 由共享 pass 状态统一拒绝当前活动 render target。
        self.pass.validate_texture_destroy(handle)?;
        // 由共享资源表检查式取出仍然存活的 texture。
        let texture = self.textures.take(handle)?;
        // 删除 framebuffer 和 texture 对象。
        // SAFETY: 上方已确认该 texture 不是当前 pass 的活动 target；texture 刚被取出、仍存活且此后不再引用；先删 framebuffer 再删 texture 顺序安全；context 保持 current。
        unsafe {
            if let Some(framebuffer) = texture.framebuffer {
                gl.delete_framebuffer(framebuffer);
            }
            gl.delete_texture(texture.native);
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
        // 由共享资源表检查式取出仍然存活的 sampler。
        let sampler = self.samplers.take(handle)?;
        // 删除底层 sampler 对象。
        // SAFETY: sampler 刚被取出、仍存活且此后不再引用，只在销毁路径删除一次；context 保持 current。
        unsafe {
            gl.delete_sampler(sampler.native);
        }
        // 返回统一成功结果。
        Ok(())
    }

    // 销毁 pipeline 资源。
    pub(super) fn destroy_pipeline(
        &mut self,
        gl: &glow::Context,
        binding: PipelineBinding,
    ) -> Result<()> {
        // 由共享资源表检查完整绑定后取出仍然存活的 pipeline。
        let pipeline = self.pipelines.take(binding)?;
        // 删除底层 program。
        // SAFETY: pipeline 刚被取出、仍存活且此后不再引用，只在销毁路径删除一次；context 保持 current。
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
        self.pass.require_closed()?;
        // 解析 surface 或离屏 texture target。
        let (framebuffer, extent) = match target.texture() {
            // Surface 目标直接写入当前默认 framebuffer。
            None => (None, surface_extent),
            // Texture 目标从资源表解析其 framebuffer 与物理范围。
            Some(texture_handle) => {
                // 查询同一类型化 texture 身份，不执行裸数值重建。
                let texture = self.texture(texture_handle)?;
                // 已签发的目标身份必须对应已创建的颜色 framebuffer。
                let framebuffer = texture
                    // 防御原生资源状态与共享目标身份不一致。
                    .framebuffer
                    // 将 Adapter 资源不完整归类为平台错误。
                    .ok_or_else(|| {
                        Error::new(
                            Errc::PlatformError,
                            "OpenGL RHI target texture resource is incomplete",
                        )
                    })?;
                // 返回存活原生 framebuffer 与共享描述范围。
                (Some(framebuffer), texture.desc.extent())
            }
        };
        // 由共享状态机统一验证目标范围、清屏颜色并建立 pass 事实。
        self.pass.begin(target, extent, load)?;
        // 绑定 framebuffer 并清理按 pass 指定的颜色。
        // SAFETY: framebuffer 取自存活 texture（surface 时为 None，解绑合法）；RhiColor 为固定四元素数组，颜色参数有效；context 保持 current。
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, framebuffer);
            if let LoadAction::Clear(color) = load {
                // Adapter 只读取共享层已经验证并预乘的目标颜色。
                let [red, green, blue, alpha] = color.components();
                // SAFETY: 共享清理契约只包含可由当前 GL context 机械编码的封闭状态。
                clear::apply_color_clear_contract(gl, UIX_COLOR_CLEAR_CONTRACT);
                // 整目标清理必须覆盖上一个 pass 可能留下的裁剪区域。
                gl.disable(glow::SCISSOR_TEST);
                // 清屏直接写目标，不执行任何额外 alpha 转换。
                gl.clear_color(red, green, blue, alpha);
                gl.clear(glow::COLOR_BUFFER_BIT);
            }
        }
        // 返回统一成功结果。
        Ok(())
    }

    // 把当前 DrawPacket 独占的完整动态栅格状态机械编码到 OpenGL。
    pub(super) fn apply_draw_raster(
        // 只读借用设备，栅格状态不再保存为 pass 历史。
        &self,
        // 借用当前 owner-thread OpenGL context。
        gl: &glow::Context,
        // 接收已经与当前 Draw 原子绑定的栅格事实。
        raster: DrawRasterState,
    ) -> Result<()> {
        // 由共享状态机一次验证 viewport、scissor、pass 与目标关系。
        self.pass.validate_draw_raster(raster)?;
        // 只读投影当前 Draw 的 viewport。
        let viewport = raster.viewport();
        // 读取共享几何 Component 已验证的唯一有符号投影。
        let (width, height) = viewport
            // Adapter 不得自行截断浮点 viewport。
            .native_size_i32()
            // 防御绕过 pass 状态机的未来调用路径。
            .ok_or_else(|| rhi_invalid("OpenGL RHI viewport is invalid"))?;
        // OpenGL viewport 保持正尺寸，Y 方向由 draw 时的目标身份决定。
        // SAFETY: viewport 已由共享值对象验证为正整像素且可精确转换为 i32；无指针参数；context 保持 current。
        unsafe {
            gl.viewport(0, 0, width, height);
        }
        // 同一次 Draw 必须覆盖原生 scissor 状态，禁止继承前一命令。
        self.apply_scissor(gl, raster.scissor())?;
        // 返回统一成功结果。
        Ok(())
    }

    // 把已经由当前命令验证的左上原点 scissor 机械编码到 OpenGL。
    pub(super) fn apply_scissor(
        &self,
        gl: &glow::Context,
        scissor: Option<RhiScissor>,
    ) -> Result<()> {
        // 读取共享状态机冻结的活动目标身份。
        let target = self.pass.target()?;
        // 使用同一 pass 的物理 extent 完成原生坐标转换。
        let extent = self.pass.extent()?;
        // 仅对显式 scissor 计算原生坐标。
        if let Some(scissor) = scissor {
            // surface 使用底部原点，texture target 保持共享顶部原点。
            let native_y = if target.is_surface() {
                // 由共享几何 Component 完成 checked 目标高度翻转。
                scissor
                    // 消费同一 pass 的已验证 extent。
                    .bottom_origin_y(extent)
                    // 防御未来调用绕过共享状态机。
                    .ok_or_else(|| rhi_invalid("OpenGL RHI scissor origin is invalid"))?
            } else {
                // texture target 把逻辑顶部存到 v=0，可直接使用共享 Y。
                scissor.y
            };
            // SAFETY: scissor 坐标、尺寸和 native_y 都来自共享 checked 几何投影；context 保持 current。
            unsafe {
                gl.enable(glow::SCISSOR_TEST);
                // 机械编码共享矩形与目标方向投影。
                gl.scissor(scissor.x, native_y, scissor.width, scissor.height);
            }
        } else {
            // SAFETY: 关闭 scissor 不读取任何坐标或指针；context 保持 current。
            unsafe {
                gl.disable(glow::SCISSOR_TEST);
            }
        }
        // 返回统一成功结果。
        Ok(())
    }

    // 结束当前 pass 并解除 framebuffer 绑定。
    pub(super) fn end_render_pass(&mut self, gl: &glow::Context) -> Result<()> {
        // 禁止在没有 pass 时结束。
        self.pass.require_open()?;
        // 解除 texture/sampler 和 framebuffer 状态。
        // SAFETY: 全部为解绑/关闭操作，不引用任何已销毁对象；bind_sampler 的 0 号单元在 GLES3 中存在；context 保持 current。
        unsafe {
            // 结束 pass 时显式关闭 scissor，避免下一 target 继承旧裁剪。
            gl.disable(glow::SCISSOR_TEST);
            gl.bind_sampler(0, None);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.bind_vertex_array(None);
        }
        // 由共享状态机原子清除目标与物理范围事实。
        self.pass.end()?;
        // 返回统一成功结果。
        Ok(())
    }

    // 执行一次 source texture 到 destination texture 的像素复制。
    pub(super) fn copy_texture(&mut self, gl: &glow::Context, copy: TextureCopy) -> Result<()> {
        // copy 必须发生在 pass 外。
        self.pass.require_closed()?;
        // 复制源原生对象与 API 无关描述，避免把资源表借用带入原生命令。
        let (source_framebuffer, source_desc) = {
            // 读取源纹理事实。
            let source = self.texture(copy.source())?;
            // 返回原生 framebuffer 和共享资源描述。
            (
                // 颜色源纹理必须拥有 framebuffer。
                source.framebuffer,
                // 向共享门禁交付资源创建时保存的同一描述。
                source.desc,
            )
        };
        // 复制目标原生对象与 API 无关描述。
        let (destination_native, destination_desc) = {
            // 读取目标纹理事实。
            let destination = self.texture(copy.destination())?;
            // 返回原生 texture 和共享资源描述。
            (
                // 保存目标原生 texture。
                destination.native,
                // 向共享门禁交付资源创建时保存的同一描述。
                destination.desc,
            )
        };
        // 格式、非空、范围与资源关系全部由共享传输契约验证。
        let bounds = copy.validate_transfer(source_desc, destination_desc)?;
        // 把源区域投影为共享验证过的 OpenGL 原点与尺寸。
        let ((source_x, source_y), (width, height)) = bounds.source().native_origin_and_size_i32();
        // 目标区域使用同一投影，只消费其左上原点。
        let ((destination_x, destination_y), _) = bounds.destination().native_origin_and_size_i32();
        // OpenGL 颜色源必须有可读 framebuffer，这只是原生资源完整性事实。
        let source_framebuffer = source_framebuffer
            // 把缺失 framebuffer 转换成稳定参数错误。
            .ok_or_else(|| rhi_invalid("OpenGL RHI color texture has no framebuffer"))?;
        // 复制前绑定源 framebuffer 作为 READ_FRAMEBUFFER，并绑定目标 texture。
        // SAFETY: source/destination 为存活颜色 texture，source framebuffer 已确认存在；复制矩形由共享契约验证；context 保持 current。
        unsafe {
            // 绑定已验证源纹理的只读 framebuffer。
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(source_framebuffer));
            // 绑定已验证目标纹理。
            gl.bind_texture(glow::TEXTURE_2D, Some(destination_native));
            // 离屏纹理把逻辑顶部存于原生第零行，因此 copy 的两端都直接使用 top-left Y。
            gl.copy_tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                destination_x,
                destination_y,
                source_x,
                source_y,
                width,
                height,
            );
            // 清除目标纹理绑定，避免后续 pass 继承隐式状态。
            gl.bind_texture(glow::TEXTURE_2D, None);
            // 清除只读 framebuffer 绑定。
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
        }
        // 返回统一成功结果。
        Ok(())
    }

    // 提交当前 owner-thread GL 命令并生成提交身份。
    pub(super) fn submit(&mut self, gl: &glow::Context) -> Result<SubmissionHandle> {
        // pass 必须已经关闭才能提交。
        self.pass.require_closed()?;
        // 令驱动在最终 swap 前观察到当前命令序列。
        // SAFETY: flush 不接收对象或指针参数，只需 context current。
        unsafe {
            gl.flush();
        }
        // 原生命令已经进入驱动队列后再释放 TextureMove 临时资源，保证复制源生命周期。
        let scratch_textures = std::mem::take(&mut self.texture_move_scratch_after_submit);
        // 每个临时资源只在所属离屏计划成功到达 submit 后删除一次。
        for scratch in scratch_textures {
            // 检查式删除仍由当前 owner-thread 设备完成。
            self.destroy_texture(gl, scratch)?;
        }
        // 由共享状态机签发提交身份，OpenGL 不再维护私有序号规则。
        self.submission_sequence.issue()
    }

    // 通过共享门禁校验不可拆的 Surface 呈现事务。
    pub(super) fn validate_present(
        // 只读借用 Device 资源与提交状态。
        &self,
        // 接收 FramePlan 构造的完整呈现事务。
        transaction: RhiPresentTransaction,
        // 接收当前 drawable token。
        current_token: SurfaceToken,
        // 接收当前 OpenGL Surface 冻结的保留能力。
        present_coherency: PresentCoherency,
    ) -> Result<ValidatedRhiPresent> {
        // OpenGL 只提供动态事实，完整呈现规则由共享 RHI Component 解释。
        transaction.validate(
            // 传入当前 Surface 代际与 extent。
            current_token,
            // 传入 Surface capability 对窄呈现的承诺。
            present_coherency,
            // 传入同一组合 context 的 Device 提交序列。
            &self.submission_sequence,
        )
    }

    // 释放所有 RHI 资源；调用方必须保证 GL context current。
    pub(super) fn release(&mut self, gl: &glow::Context) {
        // 按资源表逆序释放底层对象，避免依赖关系被提前拆除。
        for pipeline in self.pipelines.drain_reverse() {
            // SAFETY: pipeline 刚被共享资源表取出、仍存活且此后不再引用；释放期间调用方保证 context current（见本函数文档）。
            unsafe {
                gl.delete_program(pipeline.program);
            }
        }
        // 先删除 texture framebuffer，再删除 texture。
        for texture in self.textures.drain_reverse() {
            // SAFETY: texture 刚被共享资源表取出、仍存活且此后不再引用；先删 framebuffer 再删 texture；释放期间 context 保持 current。
            unsafe {
                if let Some(framebuffer) = texture.framebuffer {
                    gl.delete_framebuffer(framebuffer);
                }
                gl.delete_texture(texture.native);
            }
        }
        // 删除 sampler 资源。
        for sampler in self.samplers.drain_reverse() {
            // SAFETY: sampler 刚被共享资源表取出、仍存活且此后不再引用；释放期间 context 保持 current。
            unsafe {
                gl.delete_sampler(sampler.native);
            }
        }
        // 删除 buffer 资源和共享 VAO。
        for buffer in self.buffers.drain_reverse() {
            // SAFETY: buffer 刚被共享资源表取出、仍存活且此后不再引用；释放期间 context 保持 current。
            unsafe {
                gl.delete_buffer(buffer.native);
            }
        }
        // SAFETY: vao 为本设备创建、仍存活且此后不再引用；释放期间 context 保持 current。
        unsafe {
            gl.delete_vertex_array(self.vao);
        }
        // 清理共享 pass 状态并切断所有原生资源身份。
        self.pass.reset();
        // 释放 context 时让所有尚未 present 的迟到提交失效。
        self.submission_sequence.invalidate();
    }
}

// 生成 OpenGL RHI 的参数错误。
fn rhi_invalid(message: impl Into<String>) -> Error {
    // 统一使用 InvalidArgument，保持 adapter 错误分类稳定。
    Error::new(Errc::InvalidArgument, message)
}

// 只向父 raster Module 暴露无参数测试入口，原生资源和视觉事实继续保持私有。
#[cfg(all(target_os = "linux", uix_gpu_parity_opengl))]
pub(super) fn run_gpu_parity_test() {
    gpu_parity_tests::run_gpu_parity_test();
}

// 把真实 GPU parity harness 实现统一存放在根 tests-src 目录。
#[cfg(all(target_os = "linux", uix_gpu_parity_opengl))]
#[path = "../../../../../../tests-src/native/gpu_parity/opengl_rhi_device.rs"]
mod gpu_parity_tests;
