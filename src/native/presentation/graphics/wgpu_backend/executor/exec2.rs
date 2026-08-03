use super::*;

impl WgpuExecutor {
    pub(crate) fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_format
    }

    pub(crate) fn texture_bind_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_bind_layout
    }

    pub(crate) fn texture_sampler(&self) -> &wgpu::Sampler {
        &self.texture_sampler
    }

    pub(crate) fn active_target(&self) -> ActiveTarget {
        self.active
    }

    pub(crate) fn set_active_target(&mut self, target: ActiveTarget) {
        self.active = target;
    }

    pub(crate) fn begin_frame(&mut self, clear: [f32; 4]) {
        match self.active {
            ActiveTarget::Swapchain => {
                self.swapchain.reset(clear);
                self.frame_bind_groups.clear();
                self.frame_textures.clear();
            }
            ActiveTarget::Offscreen => self.offscreen.reset(clear),
        }
    }

    pub(crate) fn stream(&mut self) -> &mut DrawStream {
        match self.active {
            ActiveTarget::Swapchain => &mut self.swapchain,
            ActiveTarget::Offscreen => &mut self.offscreen,
        }
    }

    pub(crate) fn solid_rects(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) {
        for rect in rects {
            self.shape_quad(
                viewport,
                scissor,
                Rect::new(rect.x, rect.y, rect.w, rect.h),
                rect.rgba,
                rect.rgba,
                [rect.w, rect.h, 0.0, 0.0],
                rect.radius,
                u32::from(rect.radius.iter().any(|radius| *radius > 0.0)),
                LocalCoordinates::Pixels,
                BatchKind::Shape,
            );
        }
    }

    /// 脏区 replace 清除：透明色覆写，不走 SrcOver。
    pub(crate) fn clear_rects(&mut self, viewport: (f32, f32), rects: &[GpuSolidRect]) {
        for rect in rects {
            self.shape_quad(
                viewport,
                None,
                Rect::new(rect.x, rect.y, rect.w, rect.h),
                rect.rgba,
                rect.rgba,
                [rect.w, rect.h, 0.0, 0.0],
                [0.0; 4],
                0,
                LocalCoordinates::Pixels,
                BatchKind::Clear,
            );
        }
    }

    pub(crate) fn stroke_rects(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuStrokeRect],
    ) {
        for rect in rects {
            let half = rect.line_width * 0.5;
            self.shape_quad(
                viewport,
                scissor,
                Rect::new(
                    rect.x - half,
                    rect.y - half,
                    rect.w + rect.line_width,
                    rect.h + rect.line_width,
                ),
                rect.rgba,
                rect.rgba,
                [
                    rect.w + rect.line_width,
                    rect.h + rect.line_width,
                    rect.line_width,
                    0.0,
                ],
                rect.radius.map(|radius| radius + half),
                2,
                LocalCoordinates::Pixels,
                BatchKind::Shape,
            );
        }
    }

    pub(crate) fn linear_gradients(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuLinearGradientRect],
    ) {
        for rect in rects {
            self.shape_quad(
                viewport,
                scissor,
                Rect::new(rect.x, rect.y, rect.w, rect.h),
                rect.color_a,
                rect.color_b,
                [rect.dir as f32, 0.0, 0.0, 0.0],
                [0.0; 4],
                3,
                LocalCoordinates::Normalized,
                BatchKind::Shape,
            );
        }
    }

    pub(crate) fn radial_gradients(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        gradients: &[GpuRadialGradient],
    ) {
        for gradient in gradients {
            let radius = gradient.outer_r;
            self.shape_quad(
                viewport,
                scissor,
                Rect::new(
                    gradient.cx - radius,
                    gradient.cy - radius,
                    radius * 2.0,
                    radius * 2.0,
                ),
                gradient.color_inner,
                gradient.color_outer,
                [gradient.inner_r, gradient.outer_r, 0.0, 0.0],
                [0.0; 4],
                4,
                LocalCoordinates::Centered,
                BatchKind::Shape,
            );
        }
    }

    pub(crate) fn sectors(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        sectors: &[GpuSector],
    ) {
        for sector in sectors {
            let radius = sector.radius;
            self.shape_quad(
                viewport,
                scissor,
                Rect::new(
                    sector.cx - radius,
                    sector.cy - radius,
                    radius * 2.0,
                    radius * 2.0,
                ),
                sector.rgba,
                sector.rgba,
                [radius, sector.start_angle, sector.sweep_angle, 0.0],
                [0.0; 4],
                7,
                LocalCoordinates::Centered,
                BatchKind::Shape,
            );
        }
    }

    pub(crate) fn solid_meshes(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        meshes: &[GpuSolidMesh],
    ) {
        for mesh in meshes {
            let stream = self.stream();
            let first = stream.shapes.len() as u32;
            for point in mesh.vertices.chunks_exact(2) {
                stream.shapes.push(ShapeVertex {
                    position: ndc(point[0], point[1], viewport),
                    local: [0.0; 2],
                    color_a: mesh.rgba,
                    color_b: mesh.rgba,
                    params0: [0.0; 4],
                    params1: [0.0; 4],
                    mode: 0,
                });
            }
            let count = stream.shapes.len() as u32 - first;
            stream.push_batch(BatchKind::Shape, first, count, viewport, scissor);
        }
    }

    pub(crate) fn box_shadows(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        shadows: &[GpuBoxShadow],
    ) {
        for shadow in shadows {
            let blur_x = shadow.blur_x.max(0.5);
            let blur_y = shadow.blur_y.max(0.5);
            let mode = if shadow.ambient { 6 } else { 5 };
            let inflate = (blur_x + blur_y) * 0.5;
            let expand_w = shadow.w + blur_x * 2.0;
            let expand_h = shadow.h + blur_y * 2.0;
            self.shape_oriented_quad(
                viewport,
                scissor,
                shadow.corners,
                [
                    [0.0, 0.0],
                    [expand_w, 0.0],
                    [expand_w, expand_h],
                    [0.0, expand_h],
                ],
                shadow.rgba,
                shadow.rgba,
                [expand_w, expand_h, blur_x, blur_y],
                shadow.radius.map(|radius| radius + inflate),
                mode,
                BatchKind::Shape,
            );
        }
    }

    pub(crate) fn glyphs(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        glyph_batch::record_glyphs(self.stream(), viewport, scissor, glyphs)
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "sampled blit geometry mirrors one queued GPU operation"
    )]
    pub(crate) fn queue_sampled_blit(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        bind_group: wgpu::BindGroup,
        src: Rect,
        source_size: (f32, f32),
        dst: Rect,
        opacity: f32,
        additive: bool,
    ) {
        if dst.w <= 0.0 || dst.h <= 0.0 || source_size.0 <= 0.0 || source_size.1 <= 0.0 {
            return;
        }
        let bind_index = self.frame_bind_groups.len() as u32;
        self.frame_bind_groups.push(bind_group);
        let u0 = (src.x / source_size.0).clamp(0.0, 1.0);
        let v0 = (src.y / source_size.1).clamp(0.0, 1.0);
        let u1 = ((src.x + src.w) / source_size.0).clamp(0.0, 1.0);
        let v1 = ((src.y + src.h) / source_size.1).clamp(0.0, 1.0);
        let stream = self.stream();
        let first = stream.textures.len() as u32;
        let positions = quad_positions(dst);
        let uvs = [[u0, v0], [u1, v0], [u1, v1], [u0, v0], [u1, v1], [u0, v1]];
        let opacity = opacity.clamp(0.0, 1.0);
        for (position, uv) in positions.into_iter().zip(uvs) {
            stream.textures.push(TextureVertex {
                position: ndc(position[0], position[1], viewport),
                uv,
                opacity,
                _pad: [0.0; 3],
            });
        }
        stream.push_batch(
            BatchKind::Texture {
                bind_group: bind_index,
                additive,
            },
            first,
            6,
            viewport,
            scissor,
        );
    }

    pub(crate) fn image_blits(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        blits: &[GpuImageBlit],
    ) -> Result<()> {
        for blit in blits {
            if blit.pixel_w == 0 || blit.pixel_h == 0 || blit.w <= 0.0 || blit.h <= 0.0 {
                continue;
            }
            let expected = (blit.pixel_w as usize).saturating_mul(blit.pixel_h as usize);
            if blit.pixels.len() < expected {
                return Err(Error::new(
                    Errc::InvalidArgument,
                    "wgpu image blit pixel buffer is smaller than pixel_w * pixel_h",
                ));
            }
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("uix-image-blit"),
                size: wgpu::Extent3d {
                    width: blit.pixel_w,
                    height: blit.pixel_h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            write_bgra_texture(
                queue,
                &texture,
                blit.pixel_w,
                blit.pixel_h,
                &blit.pixels[..expected],
            )?;
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("uix-image-blit-bind-group"),
                layout: &self.texture_bind_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.texture_sampler),
                    },
                ],
            });
            self.frame_textures.push(texture);
            self.queue_sampled_blit(
                viewport,
                scissor,
                bind_group,
                Rect::new(0.0, 0.0, blit.pixel_w as f32, blit.pixel_h as f32),
                (blit.pixel_w as f32, blit.pixel_h as f32),
                Rect::new(blit.x, blit.y, blit.w, blit.h),
                blit.opacity,
                blit.additive,
            );
        }
        Ok(())
    }

    /// Encode the offscreen command stream into `view` and submit.
    pub(crate) fn flush_offscreen_to_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> Result<()> {
        if self.offscreen.is_empty() {
            return Ok(());
        }
        self.flush_stream_to_view(device, queue, ActiveTarget::Offscreen, view, width, height)?;
        self.offscreen.clear_commands_only();
        Ok(())
    }

    pub(crate) fn present(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface: &wgpu::Surface<'_>,
        width: u32,
        height: u32,
    ) -> Result<()> {
        if self.active != ActiveTarget::Swapchain {
            return Err(Error::new(
                Errc::InvalidState,
                "wgpu present requires the swapchain target",
            ));
        }
        let width = width.max(1);
        let height = height.max(1);
        // 先绘入保留色缓冲，再以全屏纹理四边形提交到 swapchain：绘制侧可
        // Load/脏区 clear，present 仍保持 FullOnly（不依赖 swapchain 图像保留）。
        // 不能使用 texture copy，因为 GL surface 不保证 COPY_DST。
        self.ensure_retained_color(device, width, height)?;
        let retained_view = {
            let retained = self.retained_color.as_ref().ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "wgpu retained color target missing after ensure",
                )
            })?;
            retained.view.clone()
        };
        self.flush_stream_to_view(
            device,
            queue,
            ActiveTarget::Swapchain,
            &retained_view,
            width,
            height,
        )?;

        let surface_texture = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Err(Error::new(
                    Errc::GraphicsOccluded,
                    "wgpu surface is temporarily unavailable",
                ));
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                return Err(Error::new(
                    Errc::GraphicsSurfaceLost,
                    "wgpu surface must be reconfigured",
                ));
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(Error::new(
                    Errc::PlatformError,
                    "wgpu surface validation failed",
                ));
            }
        };
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uix-present-retained"),
            layout: &self.texture_bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&retained_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.texture_sampler),
                },
            ],
        });
        let positions = quad_positions(Rect::new(0.0, 0.0, width as f32, height as f32));
        let uvs = [
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
        ];
        let vertices =
            positions.map(|position| ndc(position[0], position[1], (width as f32, height as f32)));
        let vertices = std::array::from_fn::<TextureVertex, 6, _>(|index| TextureVertex {
            position: vertices[index],
            uv: uvs[index],
            opacity: 1.0,
            _pad: [0.0; 3],
        });
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("uix-present-blit"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("uix-present-blit-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.texture_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw(0..6, 0..1);
        }
        queue.submit(std::iter::once(encoder.finish()));
        let surface_present_t0 = std::time::Instant::now();
        queue.present(surface_texture);
        let mut sample = crate::core::perf_probe::take_present();
        sample.wgpu_surface_present_cpu_us = surface_present_t0.elapsed().as_micros();
        sample.drawable_pixels = u64::from(width).saturating_mul(u64::from(height));
        sample.drawable_width = width;
        sample.drawable_height = height;
        sample.skipped = 0;
        crate::core::perf_probe::record_present(sample);
        self.swapchain.clear_commands_only();
        self.frame_bind_groups.clear();
        self.frame_textures.clear();
        Ok(())
    }
}
