use crate::tests::common::*;
use crate::draw::gpu_engine::*;
use std::ffi::c_void;

struct FactoryInitializedNativeContext {
    resize_calls: Rc<Cell<usize>>,
    width: i32,
    height: i32,
}

impl IGraphicsContext for FactoryInitializedNativeContext {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(GraphicsBackend::D3d11, false, 1.0)
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

#[cfg(feature = "opengles")]
#[test]
fn legacy_draw_module_reexports_native_shader_sources() {
    use crate::native::graphics::opengl::shaders;

    assert_eq!(RECT_VERT, shaders::RECT_VERT);
    assert_eq!(RECT_FRAG, shaders::RECT_FRAG);
    assert_eq!(BLUR_FRAG, shaders::BLUR_FRAG);
    assert_eq!(BLIT_FRAG, shaders::BLIT_FRAG);
    assert_eq!(BLIT_RGBA_FRAG, shaders::BLIT_RGBA_FRAG);
    assert_eq!(FULLSCREEN_VERT, shaders::FULLSCREEN_VERT);
}
