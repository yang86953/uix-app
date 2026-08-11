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
    // 标记 GL 第零行是否就是窗口顶部。
    surface_rows_start_at_top: bool,
) -> i32 {
    // Wayland EGL 已把 GL 第零行呈现为窗口顶部，可以直接使用逻辑坐标。
    if surface_rows_start_at_top {
        // 保持 top-left surface 的区域坐标不变。
        top
    } else {
        // WGL bottom-up surface 需要与 scissor 相同的垂直坐标换算。
        drawable_height - top - height
    }
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
        // 复用构造期已经冻结的唯一平台 surface 行序事实。
        let surface_rows_start_at_top = self.rhi.surface_rows_start_at_top();
        // 把公共左上原点纵坐标转换为当前 GL surface 的区域坐标。
        let read_y = gl_readback_y_from_top(
            // 使用当前 target 高度完成 WGL 换算。
            self.current.drawable_height,
            // 传入已经校验的逻辑顶部坐标。
            y,
            // 传入已经校验的区域高度。
            height,
            // EGL 直用坐标，WGL 执行垂直换算。
            surface_rows_start_at_top,
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
        unsafe {
            self.gl().read_pixels(
                x,
                // 使用已经适配 surface 行序的驱动纵坐标。
                read_y,
                width,
                height,
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
        // WGL 的 GL 第零行位于窗口底部，返回前必须恢复 top-left 行序。
        if !surface_rows_start_at_top {
            // 只反转行，不改变 BGRA/RGBA 像素内部或行内左右顺序。
            reverse_readback_rows(&mut pixels, width as usize);
        }
        // EGL 已是 top-left；WGL 已在上方规范化为同一输出契约。
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
        unsafe {
            self.gl()
                .bind_framebuffer(glow::FRAMEBUFFER, self.current.framebuffer);
        }
    }

    pub(super) fn restore_full_viewport(&self) {
        // viewport 始终使用 GL 自身坐标系；行序差异已由 shader 编译期处理。
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

// 验证 OpenGL surface readback 的纯坐标与行序适配。
#[cfg(test)]
mod readback_contract_tests {
    // 引入同一私有 adapter 的待测辅助函数。
    use super::{gl_readback_y_from_top, reverse_readback_rows, validate_readback_region};
    // 引入稳定错误分类。
    use crate::core::Errc;

    // 锁定 EGL 与 WGL 对同一左上原点区域使用不同 GL y 坐标。
    #[test]
    fn maps_top_left_region_to_platform_surface_origin() {
        // EGL 第零行就是窗口顶部，逻辑 y 应保持不变。
        assert_eq!(gl_readback_y_from_top(8, 2, 3, true), 2);
        // WGL 第零行位于窗口底部，逻辑区域应换算到第三行。
        assert_eq!(gl_readback_y_from_top(8, 2, 3, false), 3);
    }

    // 锁定垂直翻转只改变行顺序，不改变单行像素顺序。
    #[test]
    fn reverses_readback_rows_without_reversing_columns() {
        // 构造三行、每行两个像素的 bottom-up 载荷。
        let mut pixels = [1u32, 2, 3, 4, 5, 6];
        // 把 bottom-up 行序原地规范化为 top-left。
        reverse_readback_rows(&mut pixels, 2);
        // 最底行应移到末尾，同时每行左右像素保持原顺序。
        assert_eq!(pixels, [5, 6, 3, 4, 1, 2]);
    }

    // 锁定越界区域在调用 OpenGL 前返回 typed 参数错误。
    #[test]
    fn rejects_readback_region_outside_drawable() {
        // 构造纵向越过八行 drawable 的区域。
        let error = validate_readback_region(0, 7, 4, 2, 8, 8)
            // 越界输入必须失败。
            .expect_err("out-of-range readback must fail before the driver call");
        // 失败必须保持为调用方可修正的无效参数分类。
        assert_eq!(error.code(), Errc::InvalidArgument);
    }
}
