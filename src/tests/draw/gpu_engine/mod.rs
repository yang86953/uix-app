use crate::draw::gpu_engine::*;
use crate::tests::common::*;
use std::ffi::c_void;

struct FactoryInitializedNativeContext {
    resize_calls: Rc<Cell<usize>>,
    width: i32,
    height: i32,
}

impl IGraphicsContext for FactoryInitializedNativeContext {
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
        _native_window: *mut c_void,
        _width: i32,
        _height: i32,
    ) -> crate::core::Result<()> {
        panic!("factory-initialized context must not be initialized by GpuEngine")
    }

    fn resize(&mut self, width: i32, height: i32) -> crate::core::Result<()> {
        self.resize_calls.set(self.resize_calls.get() + 1);
        self.width = width;
        self.height = height;
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
        _x: i32,
        _y: i32,
        _width: i32,
        _height: i32,
    ) -> crate::core::Result<Vec<u32>> {
        Ok(Vec::new())
    }

    fn clear_render_target(
        &mut self,
        _r: f32,
        _g: f32,
        _b: f32,
        _a: f32,
    ) -> crate::core::Result<()> {
        Ok(())
    }

    fn blit_soft_fallback_tile(
        &mut self,
        _pixels: &[u32],
        _tile: SoftFallbackTile,
    ) -> crate::core::Result<()> {
        Ok(())
    }

    fn present(&mut self, _frame: &PresentFrame<'_>) -> crate::core::Result<()> {
        Ok(())
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }
}

#[test]
fn gpu_native_engine_uses_factory_drawable_without_initial_resize() {
    let resize_calls = Rc::new(Cell::new(0));
    let context = FactoryInitializedNativeContext {
        resize_calls: Rc::clone(&resize_calls),
        width: 7,
        height: 5,
    };
    let mut engine = GpuEngine::new(Box::new(context)).expect("GpuNative engine");

    engine.initialize(640, 480).expect("engine initialization");

    assert_eq!(resize_calls.get(), 0);
    assert_eq!(
        (engine.session().width(), engine.session().height()),
        (7, 5)
    );

    engine.resize(8, 6).expect("subsequent native resize");
    assert_eq!(resize_calls.get(), 1);
    assert_eq!(
        (engine.session().width(), engine.session().height()),
        (8, 6)
    );
}

#[test]
fn gpu_engine_rejects_cpu_raster_without_allocating_soft_resources() {
    let resize_calls = Rc::new(Cell::new(0));
    let context = FactoryInitializedNativeContext {
        resize_calls,
        width: 64,
        height: 64,
    };
    let mut engine = GpuEngine::new(Box::new(context)).expect("GpuNative engine");
    engine.initialize(64, 64).expect("engine initialization");
    engine
        .canvas_2d()
        .fill_ellipse(Rect::new(1.0, 1.0, 8.0, 8.0), Color::white());
    let error = engine
        .session_mut()
        .backend_mut()
        .present(&DamageRegion::full())
        .expect_err("GPU-only engine must reject CPU raster fallback");

    assert_eq!(error.code(), Errc::NotImplemented);
    assert_eq!(engine.idle_resource_deadline(), None);
    assert!(engine
        .session()
        .native_gpu_backend()
        .is_some_and(|backend| backend.surface.canvas.soft_fallback.is_none()));
}
