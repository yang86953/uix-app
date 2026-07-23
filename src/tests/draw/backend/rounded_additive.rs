use crate::draw::backend::contract::RenderBackend;
use crate::draw::backend::gpu::GpuBackend;
use crate::draw::command::{EncodedFrameExecution, FrameRadius, FrameRasterOp};
use crate::draw::geometry::types::Radius;
use crate::tests::common::*;

struct RoundedAdditiveContext {
    width: i32,
    height: i32,
    pixels: Rc<RefCell<Vec<u32>>>,
}

impl IGraphicsContext for RoundedAdditiveContext {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(
            GraphicsBackend::D3d11,
            PresentCoherency::FullOnly,
            1.0,
        )
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        NativeRasterCaps::d3d11_full()
    }

    fn initialize(
        &mut self,
        _native_window: *mut std::ffi::c_void,
        width: i32,
        height: i32,
    ) -> crate::core::Result<()> {
        self.resize(width, height)
    }

    fn resize(&mut self, width: i32, height: i32) -> crate::core::Result<()> {
        self.width = width.max(1);
        self.height = height.max(1);
        self.pixels
            .borrow_mut()
            .resize(self.width as usize * self.height as usize, 0);
        Ok(())
    }

    fn make_current(&mut self) -> crate::core::Result<()> {
        Ok(())
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> crate::core::Result<()> {
        Ok(())
    }

    fn try_shutdown(&mut self) -> crate::core::Result<()> {
        Ok(())
    }

    fn read_pixels(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> crate::core::Result<Vec<u32>> {
        let pixels = self.pixels.borrow();
        let mut result = Vec::with_capacity(width as usize * height as usize);
        for row in y..y + height {
            let start = (row * self.width + x) as usize;
            result.extend_from_slice(&pixels[start..start + width as usize]);
        }
        Ok(result)
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn clear_render_target(&mut self, r: f32, g: f32, b: f32, a: f32) -> crate::core::Result<()> {
        let color = Color::from_rgba(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
            (a * 255.0) as u8,
        );
        self.pixels.borrow_mut().fill(color.premultiplied());
        Ok(())
    }

    fn upload_surface_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
    ) -> crate::core::Result<()> {
        assert_eq!((width, height), (self.width, self.height));
        assert_eq!(pixels.len(), width as usize * height as usize);
        self.pixels.borrow_mut().copy_from_slice(pixels);
        Ok(())
    }
}

#[test]
fn rounded_additive_uses_readback_reference_apply_and_replace_upload() {
    let pixels = Rc::new(RefCell::new(Vec::new()));
    let context = RoundedAdditiveContext {
        width: 1,
        height: 1,
        pixels: Rc::clone(&pixels),
    };
    let mut backend = GpuBackend::new(Box::new(context)).expect("backend");
    backend.resize(9, 7).expect("resize");

    let mut encoder = FrameEncoder::new(9, 7).expect("encoder");
    encoder.clear(Color::from_rgba(12, 24, 36, 255));
    encoder.native(FrameRasterOp::FillRoundedRectAdditive {
        rect: FrameRect::new(1, 1, 7, 5),
        color: Color::from_rgba(80, 40, 120, 160),
        radius: FrameRadius::new(Radius::uniform(2.5)).expect("valid radius"),
    });

    assert_eq!(
        backend
            .try_execute_encoded_frame(&encoder)
            .expect("execute rounded Additive frame"),
        EncodedFrameExecution::Executed
    );
    assert_eq!(
        pixels.borrow().as_slice(),
        encoder.render_reference().pixels()
    );
}
