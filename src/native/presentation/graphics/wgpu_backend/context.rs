use super::*;

impl WgpuContext {
    pub(super) fn new(
        native_surface: *mut c_void,
        width: i32,
        height: i32,
        requested: GraphicsBackend,
        pending_failures: PendingFailureQueue,
    ) -> Result<Self> {
        let backends = wgpu_backends(requested)?;
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = backends;
        descriptor.flags = wgpu::InstanceFlags::DISCARD_HAL_LABELS;
        let instance = wgpu::Instance::new(descriptor);
        let surface = unsafe { surface::create_surface(&instance, native_surface)? };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
        }))
        .map_err(|error| {
            Error::new(
                Errc::PlatformError,
                format!("wgpu {requested} adapter unavailable: {error}"),
            )
        })?;
        let info = adapter.get_info();
        if graphics_backend(info.backend) != requested {
            return Err(Error::new(
                Errc::PlatformError,
                format!(
                    "wgpu requested {requested}, but selected {:?} adapter {}",
                    info.backend, info.name
                ),
            ));
        }
        let required_limits = device_limits_for_adapter(&adapter.limits());
        let max_texture_dimension_2d = required_limits.max_texture_dimension_2d;
        let descriptor = wgpu::DeviceDescriptor {
            label: Some("uix-wgpu-device"),
            required_features: wgpu::Features::empty(),
            required_limits,
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&descriptor)).map_err(|error| {
                Error::new(Errc::PlatformError, format!("wgpu request_device: {error}"))
            })?;
        let pending_failures = pending_failures.source();
        let uncaptured_pending = pending_failures.clone();
        device.on_uncaptured_error(Arc::new(move |error| {
            let _ = uncaptured_pending.enqueue(Error::new(
                Errc::PlatformError,
                format!("wgpu uncaptured error: {error}"),
            ));
        }));
        let device_lost_pending = pending_failures.clone();
        device.set_device_lost_callback(move |reason, message| {
            let _ = device_lost_pending.enqueue(Error::new(
                Errc::GraphicsDeviceLost,
                format!("wgpu device lost: reason={reason:?}; message={message}"),
            ));
        });
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| !format.is_srgb())
            .or_else(|| capabilities.formats.first().copied())
            .ok_or_else(|| Error::new(Errc::PlatformError, "wgpu surface has no formats"))?;
        let present_mode = capabilities
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::Fifo)
            .or_else(|| capabilities.present_modes.first().copied())
            .ok_or_else(|| Error::new(Errc::PlatformError, "wgpu surface has no present modes"))?;
        let alpha_mode = choose_surface_alpha_mode(&capabilities.alpha_modes)
            .ok_or_else(|| Error::new(Errc::PlatformError, "wgpu surface has no alpha modes"))?;
        let extent = surface::drawable_extent(native_surface, width, height);
        let (drawable_width, drawable_height) =
            ensure_surface_extent(extent.width, extent.height, max_texture_dimension_2d)?;
        let config = wgpu::SurfaceConfiguration {
            // Swapchain textures are only render targets here. In particular,
            // the GL surface backend does not advertise COPY_DST support.
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: drawable_width,
            height: drawable_height,
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);
        let renderer = WgpuExecutor::new(&device, &queue, format)?;
        let separable_blur = SeparableBlur::new(&device, format)?;
        tracing::info!(
            "WgpuContext: backend={requested}; adapter=\"{}\"; type={:?}; driver=\"{}\"; {}x{}",
            info.name,
            info.device_type,
            info.driver,
            drawable_width,
            drawable_height
        );
        Ok(Self {
            _instance: instance,
            surface,
            _adapter: adapter,
            device,
            queue,
            config,
            renderer,
            backend: requested,
            native_surface,
            logical_width: extent.logical_width,
            logical_height: extent.logical_height,
            width: drawable_width as i32,
            height: drawable_height as i32,
            max_texture_dimension_2d,
            pending_failures,
            shutdown: false,
            offscreens: Vec::new(),
            free_offscreen_ids: Vec::new(),
            next_offscreen_id: 0,
            bound_offscreen: None,
            separable_blur,
        })
    }

    pub(super) fn ensure_active(&self) -> Result<()> {
        if self.shutdown {
            Err(Error::new(
                Errc::InvalidState,
                "WgpuContext operation requested after shutdown",
            ))
        } else if let Some(error) = self.pending_failures.take() {
            Err(error)
        } else {
            Ok(())
        }
    }

    pub(super) fn flush_bound_offscreen(&mut self) -> Result<()> {
        let Some(id) = self.bound_offscreen else {
            return Ok(());
        };
        let slot = self
            .offscreens
            .get(id as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "wgpu bound offscreen target disappeared before flush",
                )
            })?;
        let width = slot.width.max(1) as u32;
        let height = slot.height.max(1) as u32;
        // view 与 texture 同寿；先克隆尺寸再借 view，避免同时借 mut self 两次。
        let view = slot.view.clone();
        self.renderer
            .flush_offscreen_to_view(&self.device, &self.queue, &view, width, height)
    }

    pub(super) fn current_target_size(&self) -> (f32, f32) {
        let bound = self.bound_offscreen.and_then(|id| {
            self.offscreens
                .get(id as usize)
                .and_then(|slot| slot.as_ref())
                .map(|slot| (slot.width, slot.height))
        });
        logical_draw_viewport(bound, self.logical_width, self.logical_height)
    }
}

impl IGraphicsContext for WgpuContext {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(
            self.backend,
            PresentCoherency::FullOnly,
            self.device_pixel_ratio(),
        )
    }

    fn native_raster_caps(&self) -> NativeRasterCaps {
        NativeRasterCaps::wgpu_full()
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
        self.ensure_active()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        self.ensure_active()?;
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let extent = surface::drawable_extent(self.native_surface, width, height);
        let (drawable_width, drawable_height) =
            ensure_surface_extent(extent.width, extent.height, self.max_texture_dimension_2d)?;
        self.logical_width = extent.logical_width;
        self.logical_height = extent.logical_height;
        self.width = drawable_width as i32;
        self.height = drawable_height as i32;
        self.config.width = drawable_width;
        self.config.height = drawable_height;
        self.surface.configure(&self.device, &self.config);
        self.renderer.invalidate_retained_color();
        Ok(())
    }

    fn make_current(&mut self) -> Result<()> {
        self.ensure_active()
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> Result<()> {
        self.ensure_active()?;
        if self.bound_offscreen.is_some() {
            self.flush_bound_offscreen()?;
            self.bound_offscreen = None;
            self.renderer.set_active_target(ActiveTarget::Swapchain);
        }
        match self.renderer.present(
            &self.device,
            &self.queue,
            &self.surface,
            self.config.width,
            self.config.height,
        ) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == Errc::GraphicsSurfaceLost => {
                self.surface.configure(&self.device, &self.config);
                self.renderer.invalidate_retained_color();
                Err(error)
            }
            Err(error) => Err(error),
        }
    }

    fn try_shutdown(&mut self) -> Result<()> {
        if self.shutdown {
            return Ok(());
        }

        // Keep the owner live and retryable until every bound/offscreen
        // resource has crossed its checked destruction boundary.  In
        // particular, do not close the source before a pending callback or a
        // flush failure can be returned to the owner.
        self.bind_swapchain_target()?;
        let handles = self
            .offscreens
            .iter()
            .enumerate()
            .filter_map(|(id, target)| target.as_ref().map(|_| OffscreenTargetId(id as u32)))
            .collect::<Vec<_>>();
        for handle in handles {
            self.try_destroy_offscreen_target(handle)?;
        }

        self.offscreens.clear();
        self.free_offscreen_ids.clear();
        self.next_offscreen_id = 0;
        self.bound_offscreen = None;

        // Late callbacks from this context are now stale. Closing the source
        // before the final device poll makes them harmless and prevents them
        // from being observed by a replacement context.
        self.pending_failures.close();
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        self.shutdown = true;
        Ok(())
    }

    fn read_pixels(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) -> Result<Vec<u32>> {
        Err(Error::new(
            Errc::NotImplemented,
            "GPU-only WgpuContext does not expose synchronous CPU readback",
        ))
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.width as f32 / self.logical_width.max(1) as f32
    }

    fn test_present(&mut self) -> Result<PresentTestResult> {
        Err(Error::new(
            Errc::NotImplemented,
            "wgpu surface does not expose a no-draw present test",
        ))
    }

    fn clear_render_target(&mut self, r: f32, g: f32, b: f32, a: f32) -> Result<()> {
        self.ensure_active()?;
        self.renderer.begin_frame([r, g, b, a]);
        Ok(())
    }

    fn clear_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        self.ensure_active()?;
        self.renderer.clear_rects((viewport_w, viewport_h), rects);
        Ok(())
    }

    fn draw_solid_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        self.ensure_active()?;
        self.renderer
            .solid_rects((viewport_w, viewport_h), scissor, rects);
        Ok(())
    }

    fn draw_stroke_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuStrokeRect],
    ) -> Result<()> {
        self.ensure_active()?;
        self.renderer
            .stroke_rects((viewport_w, viewport_h), scissor, rects);
        Ok(())
    }

    fn draw_glyphs(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        self.ensure_active()?;
        self.renderer
            .glyphs((viewport_w, viewport_h), scissor, glyphs)
    }

    fn draw_linear_gradients(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuLinearGradientRect],
    ) -> Result<()> {
        self.ensure_active()?;
        self.renderer
            .linear_gradients((viewport_w, viewport_h), scissor, rects);
        Ok(())
    }

    fn draw_radial_gradients(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        gradients: &[GpuRadialGradient],
    ) -> Result<()> {
        self.ensure_active()?;
        self.renderer
            .radial_gradients((viewport_w, viewport_h), scissor, gradients);
        Ok(())
    }

    fn draw_sectors(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        sectors: &[GpuSector],
    ) -> Result<()> {
        self.ensure_active()?;
        self.renderer
            .sectors((viewport_w, viewport_h), scissor, sectors);
        Ok(())
    }

    fn draw_solid_meshes(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        meshes: &[GpuSolidMesh],
    ) -> Result<()> {
        self.ensure_active()?;
        self.renderer
            .solid_meshes((viewport_w, viewport_h), scissor, meshes);
        Ok(())
    }

    fn draw_box_shadows(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        shadows: &[GpuBoxShadow],
    ) -> Result<()> {
        self.ensure_active()?;
        self.renderer
            .box_shadows((viewport_w, viewport_h), scissor, shadows);
        Ok(())
    }

    fn draw_image_blits(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        blits: &[GpuImageBlit],
    ) -> Result<()> {
        self.ensure_active()?;
        self.renderer.image_blits(
            &self.device,
            &self.queue,
            (viewport_w, viewport_h),
            scissor,
            blits,
        )
    }

    fn create_offscreen_target(
        &mut self,
        width: i32,
        height: i32,
    ) -> Result<OffscreenTargetId, Error> {
        self.ensure_active()?;
        let w = width.max(1) as u32;
        let h = height.max(1) as u32;
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uix-offscreen-rt"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.renderer.surface_format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uix-offscreen-sample"),
            layout: self.renderer.texture_bind_layout(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(self.renderer.texture_sampler()),
                },
            ],
        });
        let id = if let Some(id) = self.free_offscreen_ids.pop() {
            id
        } else {
            let id = self.next_offscreen_id;
            self.next_offscreen_id = self.next_offscreen_id.saturating_add(1);
            id
        };
        let idx = id as usize;
        while self.offscreens.len() <= idx {
            self.offscreens.push(None);
        }
        self.offscreens[idx] = Some(OffscreenSlot {
            texture,
            view,
            bind_group,
            width: w as i32,
            height: h as i32,
        });
        Ok(OffscreenTargetId(id))
    }

    fn try_destroy_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error> {
        self.ensure_active()?;
        self.bind_swapchain_target()?;
        let idx = id.0 as usize;
        if idx < self.offscreens.len() && self.offscreens[idx].take().is_some() {
            self.free_offscreen_ids.push(id.0);
        }
        Ok(())
    }

    fn destroy_offscreen_target(&mut self, id: OffscreenTargetId) {
        if let Err(error) = self.try_destroy_offscreen_target(id) {
            enqueue_offscreen_destroy_failure(&self.pending_failures, error);
        }
    }

    fn bind_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error> {
        self.ensure_active()?;
        let idx = id.0 as usize;
        if self.offscreens.get(idx).and_then(|o| o.as_ref()).is_none() {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("WgpuContext: bind_offscreen_target unknown id {}", id.0),
            ));
        }
        if self.bound_offscreen != Some(id.0) {
            self.flush_bound_offscreen()?;
        }
        self.bound_offscreen = Some(id.0);
        self.renderer.set_active_target(ActiveTarget::Offscreen);
        Ok(())
    }

    fn bind_swapchain_target(&mut self) -> Result<()> {
        self.ensure_active()?;
        self.flush_bound_offscreen()?;
        self.bound_offscreen = None;
        self.renderer.set_active_target(ActiveTarget::Swapchain);
        Ok(())
    }

    fn blit_offscreen_target(
        &mut self,
        id: OffscreenTargetId,
        src: Rect,
        dst: Rect,
        opacity: f32,
        additive: bool,
    ) -> Result<(), Error> {
        self.ensure_active()?;
        if !opacity.is_finite() || opacity <= 0.0 {
            return Ok(());
        }
        let idx = id.0 as usize;
        let Some(Some(slot)) = self.offscreens.get(idx) else {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("WgpuContext: blit_offscreen_target unknown id {}", id.0),
            ));
        };
        if self.bound_offscreen == Some(id.0) {
            return Err(Error::new(
                Errc::InvalidState,
                "WgpuContext: cannot blit offscreen while it is the bound RT",
            ));
        }
        // 若源刚画完但仍挂在 offscreen 流上，先落到纹理再采样。
        if self.renderer.active_target() == ActiveTarget::Offscreen {
            // 当前绑定的是别的 offscreen；源纹理应已在先前 bind_swapchain/flush 时提交。
        }
        let source_w = slot.width as f32;
        let source_h = slot.height as f32;
        let bind_group = slot.bind_group.clone();
        let (viewport_w, viewport_h) = self.current_target_size();
        let opacity = opacity.clamp(0.0, 1.0);
        if self.bound_offscreen.is_some() {
            // 画到另一个 offscreen：先把目标已有命令 flush（Load），再立刻把 blit
            // 合入 offscreen 流并再次 flush，保持与 D3D11「立即 blit」同序。
            self.flush_bound_offscreen()?;
            self.renderer.set_active_target(ActiveTarget::Offscreen);
            self.renderer.queue_sampled_blit(
                (viewport_w, viewport_h),
                None,
                bind_group,
                src,
                (source_w, source_h),
                dst,
                opacity,
                additive,
            );
            self.flush_bound_offscreen()?;
        } else {
            self.renderer.set_active_target(ActiveTarget::Swapchain);
            self.renderer.queue_sampled_blit(
                (viewport_w, viewport_h),
                None,
                bind_group,
                src,
                (source_w, source_h),
                dst,
                opacity,
                additive,
            );
        }
        Ok(())
    }

    fn blur_offscreen_target(
        &mut self,
        id: OffscreenTargetId,
        region: Rect,
        radius: f32,
    ) -> Result<(), Error> {
        self.ensure_active()?;
        if !radius.is_finite() || radius < 0.5 {
            return Ok(());
        }
        let idx = id.0 as usize;
        if self.offscreens.get(idx).and_then(|o| o.as_ref()).is_none() {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("WgpuContext: blur_offscreen_target unknown id {}", id.0),
            ));
        }
        // 若该目标正作为绑定 RT 或刚画完，先把命令落到纹理再采样。
        if self.bound_offscreen == Some(id.0)
            || self.renderer.active_target() == ActiveTarget::Offscreen
        {
            self.flush_bound_offscreen()?;
        }
        let (view, width, height) = {
            let slot = self.offscreens[idx].as_ref().ok_or_else(|| {
                Error::new(
                    Errc::InvalidArgument,
                    format!("WgpuContext: blur_offscreen_target unknown id {}", id.0),
                )
            })?;
            (slot.view.clone(), slot.width as u32, slot.height as u32)
        };
        self.separable_blur.blur_target(
            &self.device,
            &self.queue,
            &view,
            width,
            height,
            region,
            radius,
        )
    }

    fn snapshot_overlay_backdrop(&mut self) -> Result<(), Error> {
        self.ensure_active()?;
        // 上一帧 present 已把主路径写入保留色缓冲；此处仅 GPU 纹理复制。
        self.renderer
            .snapshot_overlay_backdrop(&self.device, &self.queue)
    }

    fn restore_overlay_backdrop(&mut self) -> Result<(), Error> {
        self.ensure_active()?;
        self.renderer
            .restore_overlay_backdrop(&self.device, &self.queue)
    }

    fn release_overlay_backdrop(&mut self) {
        self.renderer.release_overlay_backdrop();
    }

    fn has_overlay_backdrop(&self) -> bool {
        self.renderer.has_overlay_backdrop()
    }
}

#[cfg(test)]
mod tests {
    use super::enqueue_offscreen_destroy_failure;
    use crate::core::{Errc, Error};
    use crate::diagnostics::PendingFailureQueue;

    #[test]
    fn offscreen_destroy_failure_reaches_owner_with_typed_cause() {
        let queue = PendingFailureQueue::new();
        let source = queue.source();
        enqueue_offscreen_destroy_failure(
            &source,
            Error::new(Errc::GraphicsDeviceLost, "offscreen destroy device lost"),
        );

        let Some(error) = source.take() else {
            panic!("offscreen destroy failure must be queued");
        };
        assert_eq!(error.code(), Errc::GraphicsDeviceLost);
        assert_eq!(error.message(), "offscreen destroy device lost");
        assert!(source.take().is_none());
    }

    #[test]
    fn offscreen_destroy_failure_after_source_close_cannot_reach_replacement_owner() {
        let queue = PendingFailureQueue::new();
        let source = queue.source();
        let replacement = queue.source();
        source.close();
        enqueue_offscreen_destroy_failure(
            &source,
            Error::new(Errc::PlatformError, "late offscreen destroy failure"),
        );

        assert!(source.take().is_none());
        enqueue_offscreen_destroy_failure(
            &replacement,
            Error::new(Errc::GraphicsDeviceLost, "replacement device lost"),
        );
        let Some(error) = replacement.take() else {
            panic!("replacement source must receive only its own failure");
        };
        assert_eq!(error.code(), Errc::GraphicsDeviceLost);
        assert_eq!(error.message(), "replacement device lost");
        assert!(replacement.take().is_none());
    }

    #[test]
    fn teardown_source_close_is_idempotent_and_discards_late_failures() {
        let queue = PendingFailureQueue::new();
        let source = queue.source();
        enqueue_offscreen_destroy_failure(
            &source,
            Error::new(Errc::GraphicsDeviceLost, "queued before teardown"),
        );
        source.close();
        source.close();
        enqueue_offscreen_destroy_failure(
            &source,
            Error::new(Errc::PlatformError, "late after repeated teardown"),
        );

        assert!(source.take().is_none());
    }
}

fn wgpu_backends(backend: GraphicsBackend) -> Result<wgpu::Backends> {
    match backend {
        GraphicsBackend::Vulkan => Ok(wgpu::Backends::VULKAN),
        GraphicsBackend::D3d12 => Ok(wgpu::Backends::DX12),
        GraphicsBackend::Metal => Ok(wgpu::Backends::METAL),
        GraphicsBackend::OpenGlEs => Ok(wgpu::Backends::GL),
        other => Err(Error::new(
            Errc::InvalidArgument,
            format!("wgpu does not expose backend {other}"),
        )),
    }
}

fn graphics_backend(backend: wgpu::Backend) -> GraphicsBackend {
    match backend {
        wgpu::Backend::Vulkan => GraphicsBackend::Vulkan,
        wgpu::Backend::Dx12 => GraphicsBackend::D3d12,
        wgpu::Backend::Metal => GraphicsBackend::Metal,
        wgpu::Backend::Gl => GraphicsBackend::OpenGlEs,
        _ => GraphicsBackend::Auto,
    }
}

impl Drop for WgpuContext {
    fn drop(&mut self) {
        if let Err(error) = self.try_shutdown() {
            tracing::error!("WgpuContext shutdown failed: {}", error.short_what());
        }
    }
}
