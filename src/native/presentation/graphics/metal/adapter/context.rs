//! macOS Metal GPU-native swapchain context。

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_foundation::NSError;
use objc2_metal::{MTLCommandQueue, MTLCreateSystemDefaultDevice, MTLDevice, MTLPixelFormat};
use objc2_quartz_core::CAMetalLayer;

use crate::core::{Errc, Error, Result};
use crate::native::backends::macos::platform;
use crate::platform::presentation::rhi::{
    RhiExtent, RhiSurfaceLifecycle, RhiSurfaceRecreateReason, RhiSurfaceResizeTransaction,
};

use super::pipeline;

#[path = "context/rhi_device.rs"]
mod rhi_device;
#[path = "context/rhi_pass.rs"]
mod rhi_pass;
#[path = "context/rhi_surface.rs"]
mod rhi_surface;

// 将 Metal command-buffer NSError 收敛到框架稳定错误分类。
fn metal_command_buffer_error(
    operation: &'static str,
    error: Option<&NSError>,
    fallback: Errc,
) -> Error {
    use objc2_metal::MTLCommandBufferError;

    let code = error.map_or(fallback, |error| match error.code() as usize {
        value if value == MTLCommandBufferError::OutOfMemory.0 => Errc::GraphicsOutOfMemory,
        value if value == MTLCommandBufferError::Timeout.0 => Errc::Timeout,
        value
            if value == MTLCommandBufferError::PageFault.0
                || value == MTLCommandBufferError::AccessRevoked.0
                || value == MTLCommandBufferError::DeviceRemoved.0 =>
        {
            Errc::GraphicsDeviceLost
        }
        value if value == MTLCommandBufferError::NotPermitted.0 => Errc::PermissionDenied,
        _ => fallback,
    });
    let detail = error.map_or_else(
        || "unknown command-buffer error".to_owned(),
        |error| format!("{error:?}"),
    );
    Error::new(code, format!("Metal {operation} failed: {detail}"))
}

// Device 与 Surface 保持同一 owner，满足共享 thin RHI 的组合生命周期。
pub struct MetalContext {
    device: Retained<ProtocolObject<dyn MTLDevice>>,
    queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    // CAMetalLayer 由 AppKit window 拥有；context 只在窗口存活期间借用。
    layer: NonNull<CAMetalLayer>,
    rhi_device: rhi_device::MetalRhiDevice,
    surface_lifecycle: RhiSurfaceLifecycle,
    logical_width: i32,
    logical_height: i32,
    device_pixel_ratio: f32,
    acquired_drawable: Option<Retained<ProtocolObject<dyn objc2_quartz_core::CAMetalDrawable>>>,
    shutdown: bool,
    fault: Option<String>,
}

impl MetalContext {
    pub(crate) fn new(native_surface: *mut c_void, width: i32, height: i32) -> Result<Self> {
        let layer = NonNull::new(native_surface.cast::<CAMetalLayer>()).ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "MetalContext: native CAMetalLayer surface is null",
            )
        })?;
        if width <= 0 || height <= 0 {
            return Err(Error::new(
                Errc::InvalidArgument,
                "MetalContext: initial surface extent must be positive",
            ));
        }
        let device = MTLCreateSystemDefaultDevice().ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "MetalContext: no Metal-capable device is available",
            )
        })?;
        let queue = device.newCommandQueue().ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "MetalContext: failed to create command queue",
            )
        })?;
        // SAFETY: 原始对象由同一 AppKit window 创建并在 context 生命周期内存活。
        let native_layer = unsafe { layer.as_ref() };
        native_layer.setDevice(Some(&device));
        native_layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        native_layer.setFramebufferOnly(false);
        native_layer.setPresentsWithTransaction(false);
        let device_pixel_ratio = metal_layer_scale(native_layer)?;
        let initial_extent = metal_drawable_extent(width, height, device_pixel_ratio)?;
        let mut surface_lifecycle = RhiSurfaceLifecycle::uninitialized(initial_extent);
        let initialize = surface_lifecycle
            .begin_recreate(initial_extent, RhiSurfaceRecreateReason::Initialize)?;
        // SAFETY: AppKit layer 指针有效，尺寸已验证为正 i32。
        unsafe {
            platform::set_metal_layer_drawable_size(
                native_surface,
                initial_extent.width as i32,
                initial_extent.height as i32,
            );
        }
        surface_lifecycle.commit_recreate(initialize, initial_extent)?;
        let library = pipeline::compile_library(&device)?;
        let rhi_device = rhi_device::MetalRhiDevice::new(&device, library)?;
        Ok(Self {
            device,
            queue,
            layer,
            rhi_device,
            surface_lifecycle,
            logical_width: width,
            logical_height: height,
            device_pixel_ratio,
            acquired_drawable: None,
            shutdown: false,
            fault: None,
        })
    }

    fn layer(&self) -> &CAMetalLayer {
        // SAFETY: AppKit window 拥有 layer，且生命周期长于对应图形 context。
        unsafe { self.layer.as_ref() }
    }

    fn ensure_healthy(&self) -> Result<()> {
        if self.shutdown {
            return Err(Error::new(
                Errc::InvalidState,
                "MetalContext: operation after shutdown",
            ));
        }
        if let Some(fault) = &self.fault {
            return Err(Error::new(
                Errc::GraphicsDeviceLost,
                format!("MetalContext: device is unavailable: {fault}"),
            ));
        }
        self.surface_lifecycle.ensure_active()
    }

    fn record_fault(&mut self, error: &Error) {
        if matches!(error.code(), Errc::GraphicsDeviceLost) {
            self.fault = Some(error.short_what().to_owned());
        }
    }

    // 回滚尚未提交的单帧原生状态，并释放本帧 drawable。
    fn discard_acquired_frame_state(&mut self) {
        self.rhi_device.discard_pending_frame();
        self.acquired_drawable = None;
    }

    fn shutdown_result(&mut self) -> Result<()> {
        if self.shutdown {
            return Ok(());
        }
        self.acquired_drawable = None;
        self.rhi_device.shutdown()?;
        self.shutdown = true;
        Ok(())
    }

    fn resize_physical(&mut self, requested: RhiExtent) -> Result<()> {
        self.ensure_healthy()?;
        let width = i32::try_from(requested.width).map_err(|_| {
            Error::new(
                Errc::InvalidArgument,
                "MetalContext: drawable width exceeds the native i32 range",
            )
        })?;
        let height = i32::try_from(requested.height).map_err(|_| {
            Error::new(
                Errc::InvalidArgument,
                "MetalContext: drawable height exceeds the native i32 range",
            )
        })?;
        if width <= 0 || height <= 0 {
            return Err(Error::new(
                Errc::InvalidArgument,
                "MetalContext: resize extent must be positive",
            ));
        }
        if self.acquired_drawable.is_some() || self.rhi_device.has_pending_commands() {
            return Err(Error::new(
                Errc::InvalidState,
                "MetalContext: resize requires an idle frame boundary",
            ));
        }
        let current = self.surface_lifecycle.token();
        let resize = RhiSurfaceResizeTransaction::validate(requested, current)?;
        if resize.extent() == current.extent {
            resize.complete(current)?;
            return Ok(());
        }
        let transaction = self
            .surface_lifecycle
            .begin_recreate(requested, RhiSurfaceRecreateReason::Resize)?;
        // SAFETY: layer 仍由 AppKit owner 持有，请求尺寸已验证为正 i32。
        unsafe {
            platform::set_metal_layer_drawable_size(
                self.layer.as_ptr().cast::<c_void>(),
                width,
                height,
            );
        }
        let commit = self
            .surface_lifecycle
            .commit_recreate(transaction, requested)?;
        resize.complete(commit.token())?;
        Ok(())
    }

    // 在窗口跨屏改变 backing scale 后，于下一次 acquire 前同步物理 drawable。
    fn sync_layer_scale(&mut self) -> Result<bool> {
        let device_pixel_ratio = metal_layer_scale(self.layer())?;
        let requested =
            metal_drawable_extent(self.logical_width, self.logical_height, device_pixel_ratio)?;
        let changed = requested != self.surface_lifecycle.token().extent;
        if changed {
            self.resize_physical(requested)?;
        }
        self.device_pixel_ratio = device_pixel_ratio;
        Ok(changed)
    }
}

// 读取 AppKit 为当前 layer 维护的真实 backing scale。
fn metal_layer_scale(layer: &CAMetalLayer) -> Result<f32> {
    let scale = layer.contentsScale() as f32;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(Error::new(
            Errc::PlatformError,
            "MetalContext: CAMetalLayer contentsScale is invalid",
        ));
    }
    Ok(scale)
}

// 把逻辑窗口尺寸与 backing scale 转换为 CAMetalLayer 物理像素范围。
fn metal_drawable_extent(width: i32, height: i32, scale: f32) -> Result<RhiExtent> {
    if width <= 0 || height <= 0 || !scale.is_finite() || scale <= 0.0 {
        return Err(Error::new(
            Errc::InvalidArgument,
            "MetalContext: logical drawable extent and scale must be positive",
        ));
    }
    let physical_width = (width as f64 * scale as f64).round();
    let physical_height = (height as f64 * scale as f64).round();
    if physical_width < 1.0
        || physical_height < 1.0
        || physical_width > i32::MAX as f64
        || physical_height > i32::MAX as f64
    {
        return Err(Error::new(
            Errc::InvalidArgument,
            "MetalContext: scaled drawable extent exceeds the native range",
        ));
    }
    Ok(RhiExtent::new(
        physical_width as u32,
        physical_height as u32,
    ))
}

impl Drop for MetalContext {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown_result() {
            tracing::error!("MetalContext shutdown failed: {}", error.short_what());
        }
    }
}
