//! OpenGL ES 薄 RHI 的整块与子区域 texture 上传。

// 引入父资源表中的 OpenGL RHI 类型和错误辅助。
use super::*;

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
        if !extent.is_positive() || end_x > texture.extent.width || end_y > texture.extent.height {
            return Err(rhi_invalid("OpenGL RHI texture region is out of range"));
        }
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
        // 使用紧密 row pitch 把子区域上传到已登记的 GL texture。
        // SAFETY: texture.native 存活且区域已在上方按 extent 与数据长度校验；data 切片为有效内存且紧密排列（UNPACK_ALIGNMENT=1）；context 保持 current。
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(texture.native));
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                destination_x as i32,
                destination_y as i32,
                extent.width as i32,
                extent.height as i32,
                upload_format,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(data)),
            );
            gl.bind_texture(glow::TEXTURE_2D, None);
        }
        // 返回统一成功结果。
        Ok(())
    }
}
