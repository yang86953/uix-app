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
    GraphicsContextCaps, IGraphicsContext, NativeRasterCaps, PixelUploadSurface, PresentDamage,
    PresentSurface, SwapchainPresentation,
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
    /// only after successful owner-thread lifecycle changes, so foreign-thread
    /// queries never touch the native context.
    caps: GraphicsContextCaps,
    native_raster_caps: NativeRasterCaps,
    // 把 live drawable 元数据保存为不可撕裂的单一快照。
    present_surface: PresentSurface,
    // `Rc` is intentionally !Send + !Sync. `Cell` makes the intent equally
    // explicit to readers inspecting the wrapper's auto-trait boundary.
    _thread_bound: PhantomData<Rc<Cell<()>>>,
}

impl ThreadBoundGraphicsContext {
    fn new(inner: Box<dyn IGraphicsContext>) -> Self {
        let caps = inner.caps();
        let native_raster_caps = inner.native_raster_caps();
        // 在 owner thread 一次读取完整 surface 元数据。
        let present_surface = inner.present_surface();
        Self {
            owner_thread: thread::current().id(),
            inner: ManuallyDrop::new(inner),
            caps,
            native_raster_caps,
            present_surface,
            _thread_bound: PhantomData,
        }
    }

    fn refresh_metadata(&mut self) {
        self.caps = self.inner.caps();
        self.native_raster_caps = self.inner.native_raster_caps();
        // 生命周期变更成功后原子替换完整 surface 快照。
        self.present_surface = self.inner.present_surface();
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

    // 只有 owner thread 可以借用 inner 的 PixelUpload surface 视图。
    fn pixel_upload_surface(&mut self) -> Option<&mut dyn PixelUploadSurface> {
        // 无错误返回通道的 capability 查询在跨线程时保守返回不支持。
        if self.require_owner("pixel_upload_surface").is_err() {
            // 禁止把 wrapper 自身暴露给错误线程。
            return None;
        }
        // 只有真实 context 明确实现专用契约时 wrapper 才提供同一能力。
        self.inner.pixel_upload_surface()?;
        // 返回继续执行 owner-thread 检查与元数据同步的 wrapper 视图。
        Some(self)
    }

    // 只有 owner thread 可以借用 inner 的 external swapchain 提交视图。
    fn swapchain_presentation(&mut self) -> Option<&mut dyn SwapchainPresentation> {
        // 无错误返回通道的 capability 查询在跨线程时保守返回不支持。
        if self.require_owner("swapchain_presentation").is_err() {
            // 禁止把 wrapper 自身暴露给错误线程。
            return None;
        }
        // 先确认真实 context 明确提供 external presenter 提交能力。
        self.inner.swapchain_presentation()?;
        // 返回继续执行 owner-thread 检查的 wrapper 视图。
        Some(self)
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

    // 返回 owner-thread 最近一次确认的完整 surface 元数据快照。
    fn present_surface(&self) -> PresentSurface {
        // 非失败查询只读取 wrapper 缓存，不跨线程触碰 native context。
        self.present_surface
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.with_owner("try_shutdown", |inner| inner.try_shutdown())
    }
}

// 在线程绑定边界实现 CPU PixelUpload 的专用 surface resize。
impl PixelUploadSurface for ThreadBoundGraphicsContext {
    // 把 resize 委托给真实 PixelUpload adapter 并刷新只读元数据。
    fn resize_pixel_upload_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 在创建线程上借用 inner 的专用 surface 契约。
        self.with_owner("resize_pixel_upload_surface", |inner| {
            // recipe 宣称 PixelUpload 却未提供专用 surface 属于状态破坏。
            let Some(surface) = inner.pixel_upload_surface() else {
                // 返回 typed 状态错误，禁止回退已经删除的兼容入口。
                return Err(Error::new(
                    // 使用稳定状态分类交给恢复层。
                    Errc::InvalidState,
                    // 明确指出 factory/context 契约不一致。
                    "PixelUpload graphics context does not expose its dedicated surface",
                ));
            };
            // 在同一 owner-thread 借用范围内执行 adapter resize。
            surface.resize_pixel_upload_surface(width, height)
        })?;
        // resize 成功后同步完整 drawable surface 快照。
        self.refresh_metadata();
        // 返回已经完成线程检查和元数据同步的成功结果。
        Ok(())
    }

    // 在线程绑定边界内提交 CPU PixelUpload 像素。
    fn present_pixels(
        // 借用 thread-bound wrapper。
        &mut self,
        // 转发 premultiplied BGRA 像素。
        pixels: &[u32],
        // 转发物理像素宽度。
        width: i32,
        // 转发物理像素高度。
        height: i32,
        // 转发最终提交 damage。
        damage: PresentDamage,
    ) -> Result<()> {
        // 在 owner thread 上借用 inner 的专用 PixelUpload surface。
        self.with_owner("present_pixels", |inner| {
            // recipe 能力在构造后消失属于 native context 状态破坏。
            let Some(surface) = inner.pixel_upload_surface() else {
                // 返回 typed 状态错误，禁止回退已移除的统一 present。
                return Err(Error::new(
                    // 使用稳定状态分类交给恢复层。
                    Errc::InvalidState,
                    // 明确指出专用提交契约缺失。
                    "PixelUpload graphics context lost its dedicated presentation surface",
                ));
            };
            // 在同一 owner-thread 借用范围内执行像素提交。
            surface.present_pixels(pixels, width, height, damage)
        })
    }
}

// 在线程绑定边界内实现 external presenter 的专用 swapchain 提交。
impl SwapchainPresentation for ThreadBoundGraphicsContext {
    // 把提交委托给真实 context 暴露的专用视图。
    fn present_swapchain(&mut self, damage: PresentDamage) -> Result<()> {
        // 在创建线程上借用真实 external swapchain 提交 owner。
        self.with_owner("present_swapchain", |inner| {
            // capability 在构造后消失属于 native context 状态破坏。
            let Some(presentation) = inner.swapchain_presentation() else {
                // 返回 typed 状态错误，禁止回退已移除的统一 present。
                return Err(Error::new(
                    // 使用稳定状态分类交给恢复层。
                    Errc::InvalidState,
                    // 明确指出 external presenter 契约缺失。
                    "graphics context lost its external swapchain presentation view",
                ));
            };
            // 在同一 owner-thread 借用范围内提交 swapchain。
            presentation.present_swapchain(damage)
        })
    }
}
