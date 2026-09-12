//! OpenGL ES graphics contexts (WGL on Windows, EGL on Linux).

use std::cell::Cell;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::core::{Error, Result};
use crate::platform::presentation::GraphicsContextCandidate;

#[path = "adapter/mod.rs"]
pub(crate) mod platform;
pub(crate) mod raster;
// WGL/EGL context 通过这一层共享同一套 FramePlan/RHI host 实现。
pub(crate) mod rhi_host;

// 显式测试 feature 只转发真实 EGL/GLES harness，不扩大生产 Adapter 接口。
#[cfg(all(target_os = "linux", uix_gpu_parity_opengl))]
pub(crate) fn run_gpu_parity_test() {
    raster::run_gpu_parity_test();
}

// Shader sources are API artefacts: they live with the native GL runtime even
// while the transitional draw-side canvas still owns program/FBO lifetime.

/// Native-owned OpenGL ES runtime.
///
/// The loader and `glow` context are deliberately created below the native
/// boundary. Draw code may borrow the context through crate-private methods
/// while this owner remains alive, but it never receives a proc-loader
/// function pointer from the typed graphics recipe context.
pub(crate) struct NativeOpenGlRuntime {
    context: Box<glow::Context>,
    // GL contexts are bound to the native graphics thread. This marker makes
    // the runtime explicitly !Send + !Sync even if a dependency changes its
    // auto-trait implementation in the future.
    _thread_bound: PhantomData<Rc<Cell<()>>>,
}

impl NativeOpenGlRuntime {
    pub(crate) fn from_loader(mut load: impl FnMut(&str) -> *const c_void) -> Self {
        // SAFETY: the native WGL/EGL implementation supplies function
        // addresses for its already-current context. The runtime is retained
        // by the same thread-affine backend for every later GL call.
        let context = Box::new(unsafe { glow::Context::from_loader_function(|name| load(name)) });
        Self {
            context,
            _thread_bound: PhantomData,
        }
    }

    pub(crate) fn context(&self) -> &glow::Context {
        &self.context
    }
}

#[cfg_attr(test, allow(dead_code))]
pub(crate) fn create(
    surface: *mut c_void,
    width: i32,
    height: i32,
    // 向 registry 返回平台层已经组装的 context candidate。
) -> Result<GraphicsContextCandidate, Error> {
    platform::create(surface, width, height)
}
