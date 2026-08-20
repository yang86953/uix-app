//! OpenGL ES 薄 RHI 的整块与子区域 texture 上传。

// 引入借用或拥有的上传载荷，避免 RGBA 路径产生额外分配。
use std::borrow::Cow;

// 引入父资源表中的 OpenGL RHI 类型和错误辅助。
use super::*;

// 把 RHI 定义的像素字节布局规范化为 OpenGL RGBA8 存储布局。
fn normalize_upload_payload(format: TextureFormat, data: &[u8]) -> Cow<'_, [u8]> {
    // 只有 BGRA 输入需要在 Adapter 边界重排红蓝通道。
    if format != TextureFormat::Bgra8Unorm {
        // RGBA 与单通道载荷可直接借用调用方内存。
        return Cow::Borrowed(data);
    }
    // 上层长度门禁保证 BGRA 载荷只包含完整四字节像素。
    debug_assert_eq!(data.len() % 4, 0);
    // 复制一份只属于当前驱动调用的 RGBA 上传载荷。
    let mut rgba = data.to_vec();
    // 逐像素交换红蓝字节，绿色与 alpha 保持原位。
    for pixel in rgba.chunks_exact_mut(4) {
        // BGRA 的 B/G/R/A 转换为 OpenGL 存储使用的 R/G/B/A。
        pixel.swap(0, 2);
    }
    // 返回拥有的规范载荷，生命周期覆盖同步上传调用。
    Cow::Owned(rgba)
}

// 为 OpenGL RHI device 提供紧密排列的 texture 上传。
impl OpenGlRhiDevice {
    // 上传已经绑定目标身份、区域与紧密像素载荷的纹理命令。
    pub(crate) fn update_texture(
        &mut self,
        gl: &glow::Context,
        upload: RhiTextureUpload<'_>,
    ) -> Result<()> {
        // 先解析目标身份，空载荷也不能绕过陈旧句柄门禁。
        let texture = self.texture(upload.texture())?;
        // 由共享 Transfer Component 统一验证描述、区域、载荷和行跨度。
        let validated = upload.validate(texture.desc)?;
        // 共享门禁返回唯一可机械投影的原点、尺寸与远端边界。
        let bounds = validated.bounds();
        // 读取共享区域已经证明安全的 OpenGL 原点与尺寸。
        let ((native_x, native_y), (native_width, native_height)) =
            bounds.native_origin_and_size_i32();
        // 取得格式对应的外部 GL 通道枚举。
        let (_, upload_format) = Self::texture_format(texture.desc.format());
        // 在原生 API 边界消除 BGRA CPU 布局与 RGBA8 GL 存储之间的差异。
        let upload_data = normalize_upload_payload(texture.desc.format(), validated.data());
        // 使用紧密 row pitch 把子区域上传到已登记的 GL texture。
        // SAFETY: texture.native 存活且区域已在上方按 extent 与数据长度校验；data 切片为有效内存且紧密排列（UNPACK_ALIGNMENT=1）；context 保持 current。
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(texture.native));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                native_x,
                native_y,
                native_width,
                native_height,
                upload_format,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(upload_data.as_ref())),
            );
            gl.bind_texture(glow::TEXTURE_2D, None);
        }
        // 返回统一成功结果。
        Ok(())
    }
}

// 验证 OpenGL Adapter 独占的 CPU 上传通道规范化。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../../tests/unit/native/presentation/graphics/opengl/raster/rhi_device_upload__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
