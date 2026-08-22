//! Metal identity PixelUpload context — CAMetalLayer CPU upload path.
//!
//! Native GPU raster via Metal remains planned. This context declares
//! [`RasterMode::Cpu`] × [`PresentMode::PixelUpload`] and does not own a Metal
//! device, command queue, or raster pipeline.

use std::ffi::c_void;

use crate::core::{Errc, Error, Result};
use crate::native::backends::macos::platform;
use crate::platform::presentation::rhi::{
    RhiExtent, RhiSurfaceLifecycle, RhiSurfaceRecreateReason, RhiSurfaceRecreateTransaction,
    RhiSurfaceResizeTransaction,
};
use crate::platform::presentation::{
    GraphicsContextLifecycle, PixelUploadSurface, PresentDamage, validate_pixel_buffer,
};

type LayerId = *mut c_void;

pub struct MetalPixelUploadContext {
    layer: LayerId,
    device_pixel_ratio: f32,
    // 唯一拥有当前 PixelUpload extent、generation 与 resize 事务状态。
    surface_lifecycle: RhiSurfaceLifecycle,
    shutdown: bool,
}

impl MetalPixelUploadContext {
    pub(crate) fn new(native_surface: *mut c_void, width: i32, height: i32) -> Result<Self> {
        if native_surface.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "MetalPixelUploadContext: native CAMetalLayer surface is null",
            ));
        }
        // 非正初始尺寸映射为共享 RHI 值对象的无效零值并由唯一 lifecycle 拒绝。
        let initial_extent = RhiExtent::new(width.max(0) as u32, height.max(0) as u32);
        let mut surface_lifecycle = RhiSurfaceLifecycle::uninitialized(initial_extent);
        let initialize = surface_lifecycle
            .begin_recreate(initial_extent, RhiSurfaceRecreateReason::Initialize)?;
        // 窗口组合根已创建 CAMetalLayer 并设置 drawableSize；这里只发布同一初始事实。
        surface_lifecycle.commit_recreate(initialize, initial_extent)?;
        Ok(Self {
            layer: native_surface,
            device_pixel_ratio: 1.0,
            surface_lifecycle,
            shutdown: false,
        })
    }

    fn shutdown_result(&mut self) -> Result<()> {
        // CAMetalLayer 由 AppKit window 拥有；本 Adapter 只提交自身关闭事实。
        self.shutdown = true;
        Ok(())
    }

    // 在任何 token 读取或 AppKit helper 前统一拒绝关闭/失效 owner。
    fn ensure_active(&self, operation: &'static str) -> Result<()> {
        if self.shutdown {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("MetalPixelUploadContext: {operation} after shutdown"),
            ));
        }
        self.surface_lifecycle.ensure_active()
    }

    // 机械消费共享事务并更新 AppKit-owned CAMetalLayer，不拥有提交权。
    fn resize_surface_native(
        &mut self,
        recreate: RhiSurfaceRecreateTransaction,
    ) -> Result<RhiExtent> {
        let (width, height) = recreate.native_size_i32();
        if self.layer.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "MetalPixelUploadContext: CAMetalLayer was lost during resize",
            ));
        }
        // SAFETY: layer 由 AppKit window 持有；共享事务已证明尺寸可进入 Objective-C ABI。
        unsafe {
            platform::set_metal_layer_drawable_size(self.layer, width, height);
        }
        Ok(recreate.requested())
    }
}

impl GraphicsContextLifecycle for MetalPixelUploadContext {
    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }

    // 返回 CAMetalLayer PixelUpload 的完整 drawable 快照。
    fn present_surface(&self) -> crate::platform::presentation::PresentSurface {
        // extent 与 generation 必须从共享 lifecycle 的同一个 token 投影。
        let token = self.surface_lifecycle.token();
        crate::platform::presentation::PresentSurface::identity(
            // lifecycle 已验证 extent 属于 i32 正值域。
            token.extent.width as i32,
            // 高度与宽度来自同一原子 token。
            token.extent.height as i32,
            // 记录构造时确定的设备像素比。
            self.device_pixel_ratio,
            // resize 成功后只由共享事务推进 generation。
            token.generation,
        )
    }
}

// 为 Metal CPU PixelUpload recipe 实现专用 surface 生命周期。
impl PixelUploadSurface for MetalPixelUploadContext {
    // 把 CPU retained pixels 上传到 CAMetalLayer 并完成最终提交。
    fn present_pixels(
        // 借用当前 Metal PixelUpload owner。
        &mut self,
        // 接收 premultiplied BGRA 像素。
        pixels: &[u32],
        // 接收物理像素宽度。
        width: i32,
        // 接收物理像素高度。
        height: i32,
        // 接收最终提交 damage。
        damage: PresentDamage,
    ) -> Result<()> {
        // checked shutdown 或失败 resize 失效必须先于任何 AppKit helper 拒绝。
        self.ensure_active("present")?;
        // 提交前验证像素长度与物理 extent 一致。
        validate_pixel_buffer(pixels, width, height)?;
        let token = self.surface_lifecycle.token();
        let (current_width, current_height) = token.extent.native_size_i32().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "MetalPixelUploadContext: active surface token has invalid extent",
            )
        })?;
        if width != current_width || height != current_height {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "MetalPixelUploadContext: present extent {width}x{height} does not match active surface {current_width}x{current_height}"
                ),
            ));
        }
        // SAFETY: layer pointer comes from AppKit-owned CAMetalLayer on the UI thread.
        unsafe {
            // 把专用 PixelUpload payload 交给平台 CAMetalLayer helper。
            platform::present_layer_pixels(self.layer, pixels, width, height, damage)?;
        }
        // 只有平台 helper 成功后才报告提交完成。
        Ok(())
    }

    // 通过共享事务更新 CAMetalLayer drawable extent。
    fn resize_pixel_upload_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 关闭或先前失败失效必须在读取 token 和触碰 CAMetalLayer 前拒绝。
        self.ensure_active("resize")?;
        let current = self.surface_lifecycle.token();
        // 负值映射到无效零值，统一由共享 resize 值域门禁拒绝。
        let requested = RhiExtent::new(width.max(0) as u32, height.max(0) as u32);
        let resize = RhiSurfaceResizeTransaction::validate(requested, current)?;
        // 同尺寸不调用 AppKit helper，也不制造新 generation。
        if resize.extent() == current.extent {
            resize.complete(current)?;
            return Ok(());
        }
        let transaction = self
            .surface_lifecycle
            .begin_recreate(resize.extent(), RhiSurfaceRecreateReason::Resize)?;
        match self.resize_surface_native(transaction) {
            // 只有 native layer 更新完成后才发布新 token，并验证精确后置条件。
            Ok(actual) => {
                let commit = self
                    .surface_lifecycle
                    .commit_recreate(transaction, actual)?;
                resize.complete(commit.token())?;
                Ok(())
            }
            // 原生失败不预提交 token；共享 lifecycle 保留旧 token 并拒绝继续提交。
            Err(error) => match self.surface_lifecycle.abort_recreate(transaction) {
                Ok(()) => Err(error),
                Err(lifecycle_error) => Err(lifecycle_error.with_source(error)),
            },
        }
    }
}
