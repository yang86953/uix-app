use super::*;

// 在进入驱动前校验左上原点回读区域完整落在当前 drawable 内。
fn validate_readback_region(
    // 接收左上原点横坐标。
    x: i32,
    // 接收左上原点纵坐标。
    y: i32,
    // 接收正数区域宽度。
    width: i32,
    // 接收正数区域高度。
    height: i32,
    // 接收当前 drawable 宽度。
    drawable_width: i32,
    // 接收当前 drawable 高度。
    drawable_height: i32,
) -> Result<()> {
    // 用检查式加法拒绝横向边界溢出。
    let right = x
        // 计算区域右侧开边界。
        .checked_add(width)
        // 把整数溢出保持为调用方可修正的 typed failure。
        .ok_or_else(|| Error::new(Errc::InvalidArgument, "OpenGL readback x overflows"))?;
    // 用检查式加法拒绝纵向边界溢出。
    let bottom = y
        // 计算区域下侧开边界。
        .checked_add(height)
        // 把整数溢出保持为调用方可修正的 typed failure。
        .ok_or_else(|| Error::new(Errc::InvalidArgument, "OpenGL readback y overflows"))?;
    // 只允许完整落在 drawable 内的正尺寸区域进入 OpenGL。
    if x < 0
        // 拒绝负的逻辑顶部坐标。
        || y < 0
        // 拒绝超出 drawable 右边界的区域。
        || right > drawable_width
        // 拒绝超出 drawable 下边界的区域。
        || bottom > drawable_height
    {
        // 返回稳定的参数错误，不把裁切决定交给驱动。
        return Err(Error::new(
            // 使用统一的无效参数分类。
            Errc::InvalidArgument,
            // 保留可检索的 OpenGL 回读范围诊断。
            "OpenGL readback region is out of range",
        ));
    }
    // 区域已经满足当前 surface 契约。
    Ok(())
}

// 把左上原点区域的顶部坐标映射到当前 OpenGL surface 的读回坐标。
fn gl_readback_y_from_top(
    // 接收当前 drawable 高度。
    drawable_height: i32,
    // 接收逻辑区域顶部坐标。
    top: i32,
    // 接收逻辑区域高度。
    height: i32,
) -> i32 {
    // EGL 与 WGL window surface 都遵循 OpenGL 左下原点回读坐标。
    drawable_height - top - height
}

// 原地反转紧密排列的像素行，同时保持每行内部的左右顺序。
fn reverse_readback_rows(pixels: &mut [u32], row_width: usize) {
    // 空行宽不应来自正尺寸回读，但仍保持辅助函数安全返回。
    if row_width == 0 {
        // 避免后续除零并把空输入视为无操作。
        return;
    }
    // 回读载荷必须只包含完整的紧密像素行。
    debug_assert_eq!(pixels.len() % row_width, 0);
    // 计算需要参与对称交换的总行数。
    let row_count = pixels.len() / row_width;
    // 只遍历上半部分，每对行交换一次。
    for top_row in 0..row_count / 2 {
        // 找到与当前顶部行对称的底部行。
        let bottom_row = row_count - 1 - top_row;
        // 逐列交换同一横坐标的像素。
        for column in 0..row_width {
            // 计算顶部像素在线性缓冲区中的索引。
            let top_index = top_row * row_width + column;
            // 计算底部像素在线性缓冲区中的索引。
            let bottom_index = bottom_row * row_width + column;
            // 原地交换两个像素，避免再分配一份全帧缓冲。
            pixels.swap(top_index, bottom_index);
        }
    }
}

// 把 glReadPixels 的 RGBA 字节载荷规范化为 UIX 统一的 0xAARRGGBB 像素。
fn normalize_rgba_readback_pixels(pixels: &mut [u32]) {
    // 逐像素解码原生内存字节，避免依赖主机 u32 的通道解释。
    for pixel in pixels {
        // 固定 RGBA/UNSIGNED_BYTE 请求依次写入 R、G、B、A 四个字节。
        let [red, green, blue, alpha] = pixel.to_ne_bytes();
        // 以数值位移构造跨 Adapter 统一的 AARRGGBB packed 值。
        *pixel = ((alpha as u32) << 24)
            // 红色进入 16 到 23 位。
            | ((red as u32) << 16)
            // 绿色进入 8 到 15 位。
            | ((green as u32) << 8)
            // 蓝色进入最低八位。
            | blue as u32;
    }
}

// 复用主 raster 类型承载状态恢复与 readback 操作。
impl OpenGlRasterPipeline {
    pub(crate) fn bind_swapchain_target(&mut self) {
        self.current = self.swapchain;
        self.bind_current_framebuffer();
        self.restore_full_viewport();
    }

    pub(crate) fn read_pixels(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<Vec<u32>> {
        if width <= 0 || height <= 0 {
            return Ok(Vec::new());
        }
        // 在调用驱动前锁定完整的左上原点区域边界。
        validate_readback_region(
            // 校验逻辑横坐标。
            x,
            // 校验逻辑纵坐标。
            y,
            // 校验请求宽度。
            width,
            // 校验请求高度。
            height,
            // 使用当前 target 的真实 drawable 宽度。
            self.current.drawable_width,
            // 使用当前 target 的真实 drawable 高度。
            self.current.drawable_height,
        )?;
        // 把公共左上原点纵坐标转换为当前 GL surface 的区域坐标。
        let read_y = gl_readback_y_from_top(
            // 使用当前 surface 高度完成 OpenGL 换算。
            self.current.drawable_height,
            // 传入已经校验的逻辑顶部坐标。
            y,
            // 传入已经校验的区域高度。
            height,
        );
        let length = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| Error::new(Errc::InvalidArgument, "OpenGL readback extent overflows"))?;
        // 检查传给 glow 的字节切片长度不会溢出。
        let byte_length = length
            // 每个 RGBA8 像素占用一个 u32。
            .checked_mul(std::mem::size_of::<u32>())
            // 保持载荷长度溢出的 typed 参数错误。
            .ok_or_else(|| {
                // 返回可诊断的读回缓冲区溢出错误。
                Error::new(Errc::InvalidArgument, "OpenGL readback payload overflows")
            })?;
        let mut pixels = vec![0u32; length];
        // SAFETY: read_pixels 写入的像素缓冲由本函数刚创建且容量已按长×宽×4 验证，from_raw_parts_mut 的指针为同一切片对齐后的有效内存；矩形已在上方按 drawable 范围校验；context 保持 current。
        unsafe {
            self.gl().read_pixels(
                x,
                // 使用已经适配 surface 行序的驱动纵坐标。
                read_y,
                width,
                height,
                // 固定请求 RGBA 字节顺序，避免实现偏好改变公共像素语义。
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(std::slice::from_raw_parts_mut(
                    pixels.as_mut_ptr() as *mut u8,
                    // 传入已经检查过的完整 RGBA8 载荷字节数。
                    byte_length,
                ))),
            );
            let error = self.gl().get_error();
            if error != glow::NO_ERROR {
                return Err(Error::new(
                    Errc::PlatformError,
                    format!("OpenGL read_pixels failed with error {error:#X}"),
                ));
            }
        }
        // 先把 RGBA 字节通道规范化为与 D3D11/CPU 一致的 0xAARRGGBB。
        normalize_rgba_readback_pixels(&mut pixels);
        // OpenGL read_pixels 从低 Y 到高 Y 返回，统一反转为 top-left 行序。
        reverse_readback_rows(&mut pixels, width as usize);
        // EGL 与 WGL 都已在 adapter 内规范化为同一输出契约。
        Ok(pixels)
    }

    pub(crate) fn release(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        // 先释放 FramePlan/RHI 资源，再恢复默认 framebuffer。
        self.rhi_release();
        self.bind_swapchain_target();
    }

    pub(super) fn bind_current_framebuffer(&self) {
        // SAFETY: current.framebuffer 为 swapchain 目标或存活 texture 的 framebuffer，句柄有效；调用时 context current。
        unsafe {
            self.gl()
                .bind_framebuffer(glow::FRAMEBUFFER, self.current.framebuffer);
        }
    }

    pub(super) fn restore_full_viewport(&self) {
        // viewport 始终使用 GL 自身坐标系；行序差异已由 shader 编译期处理。
        // SAFETY: drawable 尺寸为驱动提供的非负像素值；disable 不依赖句柄；context 保持 current。
        unsafe {
            self.gl().viewport(
                0,
                0,
                self.current.drawable_width,
                self.current.drawable_height,
            );
            self.gl().disable(glow::SCISSOR_TEST);
        }
    }
}
