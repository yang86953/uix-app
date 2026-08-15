use super::*;

// 呈现器仅向 macOS Platform System 根模块开放构造与持有。
pub(super) struct MacosPresenter {
    layer: cocoa::Id,
    width: i32,
    height: i32,
}

impl MacosPresenter {
    // 由 macOS 窗口工厂为新 CAMetalLayer 构造呈现器。
    pub(super) fn new(layer: cocoa::Id, width: i32, height: i32) -> Self {
        Self {
            layer,
            width,
            height,
        }
    }
}

impl IPresenter for MacosPresenter {
    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        _damage: PresentDamage,
    ) -> crate::core::Result<(), Error> {
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        validate_pixel_buffer(pixels, width, height)?;
        if width != self.width || height != self.height {
            self.resize(width, height)?;
        }
        // SAFETY: pixels is a live Rust slice for the duration of this call, and
        // set_layer_pixels copies it into CFData/CGImage before returning.
        unsafe {
            present_layer_pixels(self.layer, pixels, width, height, _damage)?;
        }
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> crate::core::Result<(), Error> {
        if width > 0 && height > 0 {
            self.width = width;
            self.height = height;
        }
        Ok(())
    }
}
