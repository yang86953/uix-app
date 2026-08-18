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
    // 上传整张 texture，保持既有通用 RHI 入口的严格尺寸语义。
    pub(crate) fn update_texture(
        &mut self,
        gl: &glow::Context,
        handle: TextureHandle,
        extent: RhiExtent,
        data: &[u8],
    ) -> Result<()> {
        // 读取 texture 描述并要求整块上传与资源尺寸完全一致。
        let texture = self.texture(handle)?;
        if extent != texture.extent {
            return Err(rhi_invalid("OpenGL RHI texture update extent differs"));
        }
        // 复用子区域实现，避免两条上传路径产生不同的长度检查。
        self.update_texture_region(gl, handle, 0, 0, extent, data)
    }

    // 上传 texture 中任意合法的紧密排列子区域。
    pub(crate) fn update_texture_region(
        &mut self,
        gl: &glow::Context,
        handle: TextureHandle,
        destination_x: u32,
        destination_y: u32,
        extent: RhiExtent,
        data: &[u8],
    ) -> Result<()> {
        // 读取 texture 描述并校验目标矩形不越过资源边界。
        let texture = self.texture(handle)?;
        let end_x = destination_x
            .checked_add(extent.width)
            .ok_or_else(|| rhi_invalid("OpenGL RHI texture region x overflows"))?;
        let end_y = destination_y
            .checked_add(extent.height)
            .ok_or_else(|| rhi_invalid("OpenGL RHI texture region y overflows"))?;
        if !extent.is_valid() || end_x > texture.extent.width || end_y > texture.extent.height {
            return Err(rhi_invalid("OpenGL RHI texture region is out of range"));
        }
        // 读取共享几何 Component 已验证的子区域有符号尺寸。
        let (native_width, native_height) = extent
            // Adapter 不得自行强转共享 extent。
            .native_size_i32()
            // 理论上已由上方 is_valid 证明，仍保留稳定错误。
            .ok_or_else(|| rhi_invalid("OpenGL RHI texture region extent is invalid"))?;
        // 目标 X 必须能被 OpenGL GLint 无损接收。
        let native_x = i32::try_from(destination_x)
            // 越界坐标不得通过强转改变符号。
            .map_err(|_| rhi_invalid("OpenGL RHI texture region x is invalid"))?;
        // 目标 Y 必须服从相同有符号值域。
        let native_y = i32::try_from(destination_y)
            // 越界坐标不得通过强转改变符号。
            .map_err(|_| rhi_invalid("OpenGL RHI texture region y is invalid"))?;
        // 取得格式对应的外部 GL 通道和字节宽度。
        let (_, upload_format, bytes_per_pixel) = Self::texture_format(texture.format);
        let expected = (extent.width as usize)
            .checked_mul(extent.height as usize)
            .and_then(|value| value.checked_mul(bytes_per_pixel))
            .ok_or_else(|| rhi_invalid("OpenGL RHI texture region payload overflows"))?;
        if data.len() != expected {
            return Err(rhi_invalid(
                "OpenGL RHI texture region payload length is invalid",
            ));
        }
        // 在原生 API 边界消除 BGRA CPU 布局与 RGBA8 GL 存储之间的差异。
        let upload_data = normalize_upload_payload(texture.format, data);
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
mod tests {
    // 引入被测辅助函数。
    use super::normalize_upload_payload;
    // 引入共享纹理格式。
    use crate::native::present::rhi::TextureFormat;

    // 锁定 BGRA 载荷进入 RGBA8 存储前只交换红蓝通道。
    #[test]
    fn normalizes_bgra_payload_to_rgba_storage() {
        // 构造两个包含不同通道值的 BGRA 像素。
        let bgra = [0x33, 0x22, 0x11, 0x44, 0x77, 0x66, 0x55, 0x88];
        // 执行 Adapter 私有规范化。
        let rgba = normalize_upload_payload(TextureFormat::Bgra8Unorm, &bgra);
        // 每个像素只交换首尾颜色通道，alpha 不变。
        assert_eq!(
            rgba.as_ref(),
            [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]
        );
    }

    // 锁定已经是 RGBA 的载荷保持零复制与原序。
    #[test]
    fn borrows_rgba_payload_without_channel_changes() {
        // 构造一个 RGBA 像素。
        let rgba = [0x11, 0x22, 0x33, 0x44];
        // 执行无需转换的 Adapter 路径。
        let normalized = normalize_upload_payload(TextureFormat::Rgba8Unorm, &rgba);
        // RGBA 载荷必须保持原始字节顺序。
        assert_eq!(normalized.as_ref(), rgba);
        // 未转换路径必须继续借用调用方切片。
        assert!(matches!(normalized, std::borrow::Cow::Borrowed(_)));
    }
}
