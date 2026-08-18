//! OpenGL RHI 设备的资源句柄校验、查询与 framebuffer 创建辅助。

// 复用父 Module 唯一拥有的设备、资源槽位和错误契约。
use super::*;

// 为 OpenGL RHI 设备补充资源表内部操作，不建立第二份资源 owner。
impl OpenGlRhiDevice {
    // 把从 1 开始的 opaque handle 转换为资源表索引。
    pub(super) fn index(raw: u64, kind: &'static str) -> Result<usize> {
        // 零句柄不能代表已经创建的资源。
        raw.checked_sub(1)
            .map(|value| value as usize)
            .ok_or_else(|| rhi_invalid(format!("OpenGL RHI {kind} handle is null")))
    }

    // 读取一个已经存在的 buffer。
    pub(super) fn buffer(&self, handle: BufferHandle) -> Result<&OpenGlRhiBuffer> {
        // 先校验句柄的零值和索引转换。
        let index = Self::index(handle.raw(), "buffer")?;
        // 拒绝越界或已经销毁的槽位。
        self.buffers
            .get(index)
            .and_then(Option::as_ref)
            .ok_or_else(|| rhi_invalid("OpenGL RHI buffer handle is stale"))
    }

    // 读取一个可变 buffer。
    pub(super) fn buffer_mut(&mut self, handle: BufferHandle) -> Result<&mut OpenGlRhiBuffer> {
        // 先校验句柄的零值和索引转换。
        let index = Self::index(handle.raw(), "buffer")?;
        // 拒绝越界或已经销毁的槽位。
        self.buffers
            .get_mut(index)
            .and_then(Option::as_mut)
            .ok_or_else(|| rhi_invalid("OpenGL RHI buffer handle is stale"))
    }

    // 读取一个已经存在的 texture。
    pub(super) fn texture(&self, handle: TextureHandle) -> Result<&OpenGlRhiTexture> {
        // 先校验句柄的零值和索引转换。
        let index = Self::index(handle.raw(), "texture")?;
        // 拒绝越界或已经销毁的槽位。
        self.textures
            .get(index)
            .and_then(Option::as_ref)
            .ok_or_else(|| rhi_invalid("OpenGL RHI texture handle is stale"))
    }

    // 读取一个已经存在的 pipeline。
    pub(super) fn pipeline(&self, handle: PipelineHandle) -> Result<&OpenGlRhiPipeline> {
        // 先校验句柄的零值和索引转换。
        let index = Self::index(handle.raw(), "pipeline")?;
        // 拒绝越界或已经销毁的槽位。
        self.pipelines
            .get(index)
            .and_then(Option::as_ref)
            .ok_or_else(|| rhi_invalid("OpenGL RHI pipeline handle is stale"))
    }

    // 读取一个已经存在的 sampler。
    pub(super) fn sampler(&self, handle: SamplerHandle) -> Result<&OpenGlRhiSampler> {
        // 先校验句柄的零值和索引转换。
        let index = Self::index(handle.raw(), "sampler")?;
        // 拒绝越界或已经销毁的槽位。
        self.samplers
            .get(index)
            .and_then(Option::as_ref)
            .ok_or_else(|| rhi_invalid("OpenGL RHI sampler handle is stale"))
    }

    // 将通用 texture format 转换为 GLES 3.0 的内部格式和上传格式。
    pub(super) fn texture_format(format: TextureFormat) -> (i32, u32) {
        // 颜色 texture 统一用 RGBA8 存储，BGRA 上传载荷在 Adapter 边界完成规范化。
        match format {
            // BGRA payload 会先转换为 RGBA，再按四字节紧密排列上传。
            TextureFormat::Bgra8Unorm => (glow::RGBA8 as i32, glow::RGBA),
            // RGBA MSDF 和离屏颜色 texture 保持通道原序。
            TextureFormat::Rgba8Unorm => (glow::RGBA8 as i32, glow::RGBA),
            // coverage 使用 GLES 3.0 的单通道 R8 纹理。
            TextureFormat::R8Unorm => (glow::R8 as i32, glow::RED),
        }
    }

    // 为 render target texture 创建并检查颜色 framebuffer。
    pub(super) fn create_framebuffer(
        gl: &glow::Context,
        texture: glow::Texture,
    ) -> Result<glow::Framebuffer> {
        // 创建 framebuffer 对象。
        // SAFETY: 本函数前置条件为 context current；create_framebuffer 无指针参数，失败走错误返回。
        let framebuffer = unsafe {
            gl.create_framebuffer()
                .map_err(|error| gl_error("create RHI framebuffer", error))?
        };
        // 把 texture 接到颜色附件并检查完整性。
        // SAFETY: framebuffer 与 texture 均由本设备刚创建且未销毁，句柄存活；context 保持 current；组合正确性由随后的完整性检查兜底。
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
        // SAFETY: 上一步刚绑定的 framebuffer 此时仍处于绑定状态且存活，查询为只读操作；context 保持 current。
        let complete =
            unsafe { gl.check_framebuffer_status(glow::FRAMEBUFFER) } == glow::FRAMEBUFFER_COMPLETE;
        // 不让资源表保留不完整的 framebuffer。
        // SAFETY: 解绑不依赖任何对象句柄，仅将当前 GL 状态恢复为无 framebuffer；context 保持 current。
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        }
        // 不完整 framebuffer 不得进入资源表。
        if !complete {
            // 释放不完整 framebuffer 并返回平台错误。
            // SAFETY: framebuffer 由本函数刚创建、未被其他位置引用，删除后不再使用，仅在此销毁一次；context 保持 current。
            unsafe {
                gl.delete_framebuffer(framebuffer);
            }
            // 把失败归类为平台图形错误。
            return Err(Error::new(
                // 使用统一的平台错误码。
                Errc::PlatformError,
                // 保留稳定的 OpenGL RHI 诊断文本。
                "OpenGL RHI framebuffer is incomplete",
            ));
        }
        // 返回已经检查过的颜色 framebuffer。
        Ok(framebuffer)
    }
}
