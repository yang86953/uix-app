//! Canvas2D `pixels_mut` 测试关联项宏定义（自源文件移入）。
//! trait 声明与四处 impl 的方法体在此定义，源码仅保留 cfg(test) 宏调用，
//! 展开后的形状与借用语义与原内联一致；经 #[path] 挂载+宏路径 use 引用，
//! 不进发布包。

/// Canvas2D trait 的测试构建可变像素声明。
macro_rules! canvas2d_pixels_mut_decl {
    () => {
        #[cfg(test)]
        /// 在测试构建中借用当前表面的可变像素。
        fn pixels_mut(&mut self) -> &mut [u32];
    };
}
pub(crate) use canvas2d_pixels_mut_decl;


macro_rules! noop_canvas2d_pixels_mut_impl {
    () => {
#[cfg(test)]
    fn pixels_mut(&mut self) -> &mut [u32] {
        &mut []
    }
    };
}
pub(crate) use noop_canvas2d_pixels_mut_impl;


macro_rules! gpu_canvas2d_pixels_mut_impl {
    () => {
#[cfg(test)]
    fn pixels_mut(&mut self) -> &mut [u32] {
        if self.gpu_only {
            self.reject_unsupported("direct CPU pixel access in GPU-only mode");
            return &mut self.rejected_pixels;
        }
        // 测试直写像素也必须先建立与当前 blend 一致的 soft 段。
        self.prepare_soft_segment(self.blend_mode);
        self.mark_soft();
        self.soft_uses_destination_blend |= matches!(self.blend_mode, BlendMode::Additive);
        self.ensure_soft().pixels_mut()
    }
    };
}
pub(crate) use gpu_canvas2d_pixels_mut_impl;


macro_rules! recorder_canvas2d_pixels_mut_impl {
    () => {
#[cfg(test)]
    fn pixels_mut(&mut self) -> &mut [u32] {
        if let Err(error) = self.ensure_scratch() {
            self.remember_error(error);
            return self.scratch.pixels_mut();
        }
        self.scratch_dirty = true;
        // 测试直接写像素时沿用当前 blend 的最终合成事实。
        self.scratch_additive = self.blend_mode == BlendMode::Additive;
        self.scratch.pixels_mut()
    }
    };
}
pub(crate) use recorder_canvas2d_pixels_mut_impl;


macro_rules! shared_rasterizer_pixels_mut_impl {
    () => {
#[cfg(test)]
    fn pixels_mut(&mut self) -> &mut [u32] {
        self.surface.pixels_mut()
    }
    };
}
pub(crate) use shared_rasterizer_pixels_mut_impl;
