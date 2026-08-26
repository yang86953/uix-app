//! OpenGL ES 薄 RHI 的整块与子区域 texture 上传。

// 引入借用或拥有的上传载荷，避免 RGBA 路径产生额外分配。
use std::borrow::Cow;

// 引入父资源表中的 OpenGL RHI 类型和错误辅助。
use super::*;

// 标量参考实现同时承担非 x86_64 或缺少 SIMD 特性的跨架构后备。
fn swap_bgra_to_rgba_scalar(data: &mut [u8]) {
    // 只处理完整像素；上层 RHI 门禁保证生产载荷没有残缺尾字节。
    for pixel in data.chunks_exact_mut(4) {
        // BGRA 的 B/G/R/A 转换为 GLES 存储使用的 R/G/B/A。
        pixel.swap(0, 2);
    }
}

// 在 x86_64 上以运行时特性检测选择最快可用实现，其余架构固定回退标量。
fn swap_bgra_to_rgba(data: &mut [u8]) {
    #[cfg(target_arch = "x86_64")]
    {
        // AVX2 快路径只在当前 CPU 明确报告支持后调用，禁止执行非法指令。
        if std::arch::is_x86_feature_detected!("avx2") {
            // SAFETY: 运行时检测已证明 AVX2 可用；函数内部只做切片范围内非对齐加载与存储。
            unsafe { swap_bgra_to_rgba_avx2(data) };
            return;
        }
        // 较老 x86_64 CPU 可在显式 SSSE3 能力下使用 128-bit shuffle。
        if std::arch::is_x86_feature_detected!("ssse3") {
            // SAFETY: 运行时检测已证明 SSSE3 可用；函数内部只访问完整 16-byte 块。
            unsafe { swap_bgra_to_rgba_ssse3(data) };
            return;
        }
    }
    // 不支持 SIMD 的 CPU 与所有非 x86_64 架构使用等价逐字节实现。
    swap_bgra_to_rgba_scalar(data);
}

// 以 256-bit lane 内字节 shuffle 一次转换八个完整像素。
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline(never)]
unsafe fn swap_bgra_to_rgba_avx2(data: &mut [u8]) {
    use std::arch::x86_64::{__m256i, _mm256_storeu_si256};
    use std::arch::x86_64::{_mm256_loadu_si256, _mm256_setr_epi8, _mm256_shuffle_epi8};

    // 两个 128-bit lane 使用相同 B/G/R/A → R/G/B/A shuffle 索引。
    let shuffle = _mm256_setr_epi8(
        2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15, 2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11,
        14, 13, 12, 15,
    );
    let vector_bytes = data.len() / 32 * 32;
    let mut offset = 0;
    while offset < vector_bytes {
        // SAFETY: offset 每次推进 32 且小于 vector_bytes，loadu 覆盖范围始终位于切片内并允许非对齐地址。
        let pixels = unsafe { _mm256_loadu_si256(data.as_ptr().add(offset).cast::<__m256i>()) };
        let rgba = _mm256_shuffle_epi8(pixels, shuffle);
        // SAFETY: 与上方相同的范围证明覆盖完整 32-byte 块；storeu 不要求地址对齐。
        unsafe {
            _mm256_storeu_si256(data.as_mut_ptr().add(offset).cast::<__m256i>(), rgba);
        }
        offset += 32;
    }
    // 末尾不足 32 字节的完整像素继续走同一标量参考语义。
    swap_bgra_to_rgba_scalar(&mut data[vector_bytes..]);
}

// 以 128-bit lane 内字节 shuffle 一次转换四个完整像素。
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
#[inline(never)]
unsafe fn swap_bgra_to_rgba_ssse3(data: &mut [u8]) {
    use std::arch::x86_64::{__m128i, _mm_storeu_si128};
    use std::arch::x86_64::{_mm_loadu_si128, _mm_setr_epi8, _mm_shuffle_epi8};

    // 每个四字节像素只交换 B/R，绿色与 alpha 的字节索引保持不变。
    let shuffle = _mm_setr_epi8(2, 1, 0, 3, 6, 5, 4, 7, 10, 9, 8, 11, 14, 13, 12, 15);
    let vector_bytes = data.len() / 16 * 16;
    let mut offset = 0;
    while offset < vector_bytes {
        // SAFETY: offset 每次推进 16 且小于 vector_bytes，loadu 只读取切片内完整块并允许非对齐地址。
        let pixels = unsafe { _mm_loadu_si128(data.as_ptr().add(offset).cast::<__m128i>()) };
        let rgba = _mm_shuffle_epi8(pixels, shuffle);
        // SAFETY: 目标块与刚读取的范围相同且完整位于唯一可变切片内，storeu 不要求地址对齐。
        unsafe {
            _mm_storeu_si128(data.as_mut_ptr().add(offset).cast::<__m128i>(), rgba);
        }
        offset += 16;
    }
    // 末尾不足 16 字节的完整像素继续走同一标量参考语义。
    swap_bgra_to_rgba_scalar(&mut data[vector_bytes..]);
}

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
    // 运行时选择 AVX2、SSSE3 或跨架构标量后备，输出字节语义完全一致。
    swap_bgra_to_rgba(&mut rgba);
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
