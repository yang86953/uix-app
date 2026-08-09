//! Factory-owned thread affinity for native graphics contexts (#183).
//!
//! Native APIs such as WGL, EGL, D3D and Vulkan associate a context with the
//! creating thread.  Every production context therefore leaves the registry
//! wrapped in this type.  The `Rc` marker deliberately makes the wrapper
//! neither `Send` nor `Sync`; safe Rust cannot transfer it to another thread.
//! Result-returning graphics operations also validate the owner and report an
//! `InvalidState` error instead of reaching an API object from the wrong
//! thread.
//!
//! The former non-fallible lifecycle operations now have checked `try_*`
//! counterparts.  The legacy hooks remain compatibility adapters while each
//! platform implementation migrates; they log and return a safe empty value
//! on an owner violation rather than panicking or reaching native state.

use std::cell::Cell;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::rc::Rc;
use std::thread::{self, ThreadId};

use crate::core::{Errc, Error, Result};
use crate::native::present::{
    GraphicsContextCaps, IGraphicsContext, NativeRasterCaps, PresentDamage, PresentFrame,
    PresentTestResult,
};

pub(crate) fn bind_to_current_thread(
    inner: Box<dyn IGraphicsContext>,
) -> Box<dyn IGraphicsContext> {
    Box::new(ThreadBoundGraphicsContext::new(inner))
}

pub(crate) struct ThreadBoundGraphicsContext {
    owner_thread: ThreadId,
    inner: ManuallyDrop<Box<dyn IGraphicsContext>>,
    /// Read-only metadata is captured on the creation thread and refreshed
    /// only after successful owner-thread lifecycle changes. These legacy
    /// non-fallible queries can therefore never touch a native context from a
    /// foreign thread.
    caps: GraphicsContextCaps,
    native_raster_caps: NativeRasterCaps,
    width: i32,
    height: i32,
    device_pixel_ratio: f32,
    // `Rc` is intentionally !Send + !Sync. `Cell` makes the intent equally
    // explicit to readers inspecting the wrapper's auto-trait boundary.
    _thread_bound: PhantomData<Rc<Cell<()>>>,
}

impl ThreadBoundGraphicsContext {
    fn new(inner: Box<dyn IGraphicsContext>) -> Self {
        let caps = inner.caps();
        let native_raster_caps = inner.native_raster_caps();
        let width = inner.width();
        let height = inner.height();
        let device_pixel_ratio = inner.device_pixel_ratio();
        Self {
            owner_thread: thread::current().id(),
            inner: ManuallyDrop::new(inner),
            caps,
            native_raster_caps,
            width,
            height,
            device_pixel_ratio,
            _thread_bound: PhantomData,
        }
    }

    fn refresh_metadata(&mut self) {
        self.caps = self.inner.caps();
        self.native_raster_caps = self.inner.native_raster_caps();
        self.width = self.inner.width();
        self.height = self.inner.height();
        self.device_pixel_ratio = self.inner.device_pixel_ratio();
    }

    fn require_owner(&self, operation: &str) -> Result<()> {
        let current = thread::current().id();
        if current == self.owner_thread {
            return Ok(());
        }
        Err(Error::new(
            Errc::InvalidState,
            format!(
                "graphics context operation {operation} must run on creation thread {:?}; current thread is {:?}",
                self.owner_thread, current
            ),
        ))
    }

    fn with_owner<T>(
        &mut self,
        operation: &str,
        run: impl FnOnce(&mut dyn IGraphicsContext) -> Result<T>,
    ) -> Result<T> {
        self.require_owner(operation)?;
        run(self.inner.as_mut())
    }

    fn log_legacy_rejection(operation: &str, error: &Error) {
        tracing::error!(
            "legacy graphics context {operation} rejected: {}",
            error.what()
        );
    }

    // 测试目标保留可注入 owner thread 的构造器，供线程归属契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn with_test_owner(
        inner: Box<dyn IGraphicsContext>,
        owner_thread: ThreadId,
    ) -> Self {
        let mut bound = Self::new(inner);
        bound.owner_thread = owner_thread;
        bound
    }
}

impl Drop for ThreadBoundGraphicsContext {
    fn drop(&mut self) {
        // SAFETY: Drop runs once; the inner Box is taken exactly once here.
        let mut inner = unsafe { ManuallyDrop::take(&mut self.inner) };
        if self.require_owner("drop").is_err() {
            // Wrong-thread Drop must not tear down native API objects. Leak the
            // context so its Drop cannot run on this foreign thread.
            Self::log_legacy_rejection(
                "drop",
                &Error::new(
                    Errc::InvalidState,
                    format!(
                        "graphics context operation drop must run on creation thread {:?}; current thread is {:?}",
                        self.owner_thread,
                        thread::current().id()
                    ),
                ),
            );
            std::mem::forget(inner);
            return;
        }
        if let Err(error) = inner.try_shutdown() {
            Self::log_legacy_rejection("drop", &error);
        }
        drop(inner);
    }
}

macro_rules! forward_result {
    ($name:ident($($argument:ident : $argument_type:ty),* $(,)?) -> $output:ty) => {
        fn $name(&mut self, $($argument: $argument_type),*) -> Result<$output> {
            self.with_owner(stringify!($name), |inner| inner.$name($($argument),*))
        }
    };
}

impl IGraphicsContext for ThreadBoundGraphicsContext {
    fn caps(&self) -> GraphicsContextCaps {
        self.caps
    }

    // 只有 owner thread 可以借用 inner 的薄 RHI 组合视图。
    fn rhi_context(&mut self) -> Option<&mut dyn crate::native::present::rhi::GraphicsContextRhi> {
        // 该查询无错误返回通道，跨线程时保守地报告不支持。
        if self.require_owner("rhi_context").is_err() {
            return None;
        }
        // 把 RHI 借用继续委托给真实 native context。
        self.inner.rhi_context()
    }

    // 在线程绑定边界内执行 RHI surface resize，并同步外层 drawable 元数据。
    fn resize_rhi_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 把实际 resize 委托给创建线程上的 native context。
        self.with_owner("resize_rhi_surface", |inner| {
            inner.resize_rhi_surface(width, height)
        })?;
        // RHI 可能改变物理 drawable，成功后刷新 wrapper 的缓存查询值。
        self.refresh_metadata();
        // 返回已经通过 owner-thread 和 surface generation 边界的成功结果。
        Ok(())
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        self.native_raster_caps
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        self.with_owner("resize", |inner| inner.resize(width, height))?;
        self.refresh_metadata();
        Ok(())
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.with_owner("try_shutdown", |inner| inner.try_shutdown())
    }

    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u32>> {
        self.with_owner("read_pixels", |inner| {
            inner.read_pixels(x, y, width, height)
        })
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn present_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        damage: PresentDamage,
    ) -> Result<()> {
        self.with_owner("present_pixels", |inner| {
            inner.present_pixels(pixels, width, height, damage)
        })
    }

    forward_result!(present(frame: &PresentFrame) -> ());
    forward_result!(test_present() -> PresentTestResult);

    fn device_pixel_ratio(&self) -> f32 {
        self.device_pixel_ratio
    }
}
