//! CAMetalLayer acquire、resize、present 与 GPU recipe 生命周期。

use objc2::runtime::ProtocolObject;
use objc2_metal::{MTLCommandBuffer, MTLCommandBufferStatus, MTLCommandQueue, MTLDrawable};

use crate::core::{Errc, Error, PresentCoherency, Result};
use crate::platform::presentation::rhi::{
    GraphicsSurface, GraphicsSurfaceCapabilities, RhiExtent, RhiPresentTransaction,
    RhiSurfaceResizeTransaction, SurfaceFrame, SurfaceToken,
};
use crate::platform::presentation::{GpuRecipeContext, GraphicsContextLifecycle};

use super::{MetalContext, metal_command_buffer_error};

impl GraphicsContextLifecycle for MetalContext {
    fn present_surface(&self) -> crate::platform::presentation::PresentSurface {
        let token = self.surface_lifecycle.token();
        crate::platform::presentation::PresentSurface::identity(
            token.extent.width as i32,
            token.extent.height as i32,
            self.device_pixel_ratio,
            token.generation,
        )
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }
}

impl GpuRecipeContext for MetalContext {
    fn rhi_context(
        &mut self,
    ) -> Result<&mut dyn crate::platform::presentation::rhi::GraphicsContextRhi> {
        self.ensure_healthy()?;
        Ok(self)
    }

    fn resize_surface(&mut self, width: i32, height: i32) -> Result<()> {
        self.ensure_healthy()?;
        let logical_width = width.max(1);
        let logical_height = height.max(1);
        let device_pixel_ratio = super::metal_layer_scale(self.layer())?;
        let requested =
            super::metal_drawable_extent(logical_width, logical_height, device_pixel_ratio)?;
        self.resize_physical(requested)?;
        self.logical_width = logical_width;
        self.logical_height = logical_height;
        self.device_pixel_ratio = device_pixel_ratio;
        Ok(())
    }
}

impl GraphicsSurface for MetalContext {
    fn surface_capabilities(&self) -> GraphicsSurfaceCapabilities {
        GraphicsSurfaceCapabilities {
            present_coherency: PresentCoherency::FullOnly,
            readback: false,
        }
    }

    fn token(&self) -> SurfaceToken {
        self.surface_lifecycle.token()
    }

    fn acquire(&mut self) -> Result<SurfaceFrame> {
        self.ensure_healthy()?;
        if self.sync_layer_scale()? {
            return Err(Error::new(
                Errc::GraphicsSurfaceChanged,
                "Metal drawable scale changed before acquire",
            ));
        }
        if self.acquired_drawable.is_some() {
            return Err(Error::new(
                Errc::InvalidState,
                "Metal surface already owns an acquired drawable",
            ));
        }
        let drawable = self.layer().nextDrawable().ok_or_else(|| {
            Error::new(
                Errc::GraphicsSurfaceLost,
                "CAMetalLayer returned no drawable",
            )
        })?;
        self.acquired_drawable = Some(drawable);
        Ok(SurfaceFrame::new(self.token()))
    }

    fn discard_acquired_frame(&mut self, _frame: SurfaceFrame) -> Result<()> {
        self.discard_acquired_frame_state();
        Ok(())
    }

    fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken> {
        self.ensure_healthy()?;
        if self.acquired_drawable.is_some() || self.rhi_device.has_pending_commands() {
            return Err(Error::new(
                Errc::InvalidState,
                "Metal surface resize requires an idle frame boundary",
            ));
        }
        let current = self.token();
        let resize = RhiSurfaceResizeTransaction::validate(extent, current)?;
        if resize.extent() == current.extent {
            resize.complete(current)?;
            return Ok(current);
        }
        let logical_width = (extent.width as f32 / self.device_pixel_ratio)
            .round()
            .max(1.0) as i32;
        let logical_height = (extent.height as f32 / self.device_pixel_ratio)
            .round()
            .max(1.0) as i32;
        self.resize_physical(extent)?;
        self.logical_width = logical_width;
        self.logical_height = logical_height;
        resize.complete(self.token())
    }

    fn present(&mut self, transaction: RhiPresentTransaction) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.pass.require_closed()?;
        if self.rhi_device.command_buffer.is_some() {
            return Err(Error::new(
                Errc::InvalidState,
                "Metal surface present requires submitted device commands",
            ));
        }
        transaction.validate(
            self.token(),
            PresentCoherency::FullOnly,
            &self.rhi_device.submission_sequence,
        )?;
        let drawable = self.acquired_drawable.take().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "Metal surface present requires an acquired drawable",
            )
        })?;
        let command = self.queue.commandBuffer().ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "Metal failed to create the present command buffer",
            )
        })?;
        let drawable_base: &ProtocolObject<dyn MTLDrawable> = ProtocolObject::from_ref(&*drawable);
        command.presentDrawable(drawable_base);
        command.commit();
        command.waitUntilCompleted();
        if command.status() != MTLCommandBufferStatus::Completed {
            let native_error = command.error();
            let error = metal_command_buffer_error(
                "present",
                native_error.as_deref(),
                Errc::GraphicsSurfaceLost,
            );
            self.record_fault(&error);
            return Err(error);
        }
        Ok(())
    }
}
