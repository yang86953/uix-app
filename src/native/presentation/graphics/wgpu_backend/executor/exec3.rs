use super::*;

impl WgpuExecutor {
    /// resize / surface rebuild 后丢弃保留缓冲，下一帧强制全幅 clear。
    pub(crate) fn invalidate_retained_color(&mut self) {
        self.retained_color = None;
        self.overlay_backdrop = None;
        self.swapchain.needs_clear = true;
    }

    /// 是否持有干净的 overlay 主表面 GPU 快照。
    pub(crate) fn has_overlay_backdrop(&self) -> bool {
        self.overlay_backdrop.is_some()
    }

    /// 释放 overlay 背景快照（普通树变脏、尺寸变化或浮层全部离场）。
    pub(crate) fn release_overlay_backdrop(&mut self) {
        self.overlay_backdrop = None;
    }

    /// 在 begin_frame 清除前，把保留色缓冲复制到 overlay 快照（无 CPU readback）。
    pub(crate) fn snapshot_overlay_backdrop(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        let (retained_tex, width, height) = {
            let retained = self.retained_color.as_ref().ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "wgpu overlay backdrop snapshot requires a retained color target",
                )
            })?;
            (retained.texture.clone(), retained.width, retained.height)
        };
        if width == 0 || height == 0 {
            return Err(Error::new(
                Errc::InvalidState,
                "wgpu overlay backdrop snapshot rejected empty retained extent",
            ));
        }
        let needs_new = match &self.overlay_backdrop {
            Some(target) => target.width != width || target.height != height,
            None => true,
        };
        if needs_new {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("uix-overlay-backdrop"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_format,
                usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.overlay_backdrop = Some(OverlayBackdropTarget {
                texture,
                width,
                height,
            });
        }
        let backdrop = self.overlay_backdrop.as_ref().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "wgpu overlay backdrop missing after ensure",
            )
        })?;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("uix-overlay-backdrop-snapshot"),
        });
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &retained_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &backdrop.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// 把 overlay 快照写回保留色缓冲，并取消本帧全幅 LoadOp::Clear。
    ///
    /// 调用方须在 begin_frame 标记 clear 之后、绘制浮层之前调用，使半透明
    /// 遮罩每帧都从干净背景合成，而不是叠在上一帧的遮罩上。
    pub(crate) fn restore_overlay_backdrop(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        let (backdrop_tex, width, height) = {
            let backdrop = self.overlay_backdrop.as_ref().ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "wgpu overlay backdrop restore requires a captured snapshot",
                )
            })?;
            (backdrop.texture.clone(), backdrop.width, backdrop.height)
        };
        // 恢复路径不得重建保留缓冲：ensure 在新建时会清掉 overlay_backdrop。
        let retained = self.retained_color.as_ref().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "wgpu overlay backdrop restore requires an existing retained color target",
            )
        })?;
        if retained.width != width || retained.height != height {
            return Err(Error::new(
                Errc::InvalidState,
                "wgpu overlay backdrop extent does not match retained color",
            ));
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("uix-overlay-backdrop-restore"),
        });
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &backdrop_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &retained.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));
        // 快照已写入保留缓冲：后续 flush 必须 Load，不能 Clear 抹掉背景。
        self.swapchain.mark_content_retained();
        Ok(())
    }

    pub(crate) fn ensure_retained_color(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let needs_new = match &self.retained_color {
            Some(target) => target.width != width || target.height != height,
            None => true,
        };
        if needs_new {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("uix-retained-color"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.surface_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.retained_color = Some(RetainedColorTarget {
                texture,
                view,
                width,
                height,
            });
            // 新缓冲内容未定义，必须全幅 clear 后再绘制。
            self.swapchain.needs_clear = true;
            // 尺寸变化后旧快照不再有效。
            self.overlay_backdrop = None;
        }
        Ok(())
    }

    pub(crate) fn flush_stream_to_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        which: ActiveTarget,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let covers = {
            let stream = match which {
                ActiveTarget::Swapchain => &mut self.swapchain,
                ActiveTarget::Offscreen => &mut self.offscreen,
            };
            if stream.is_empty() {
                return Ok(());
            }
            std::mem::take(&mut stream.glyph_covers)
        };
        let stream = match which {
            ActiveTarget::Swapchain => &self.swapchain,
            ActiveTarget::Offscreen => &self.offscreen,
        };
        let shape_bytes = bytemuck::cast_slice(&stream.shapes);
        let glyph_bytes = bytemuck::cast_slice(&stream.glyphs);
        let msdf_bytes = bytemuck::cast_slice(&stream.msdf_glyphs);
        let texture_bytes = bytemuck::cast_slice(&stream.textures);
        let glyph_offset = align_up(shape_bytes.len() as u64, 16);
        let msdf_offset = align_up(glyph_offset.saturating_add(glyph_bytes.len() as u64), 16);
        let texture_offset = align_up(msdf_offset.saturating_add(msdf_bytes.len() as u64), 16);
        let required = texture_offset
            .saturating_add(texture_bytes.len() as u64)
            .max(1);
        if required > self.vertex_capacity {
            self.vertex_capacity = required.next_power_of_two();
            self.vertex_buffer = create_vertex_buffer(device, self.vertex_capacity);
        }
        if !shape_bytes.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, shape_bytes);
        }
        if !glyph_bytes.is_empty() {
            queue.write_buffer(&self.vertex_buffer, glyph_offset, glyph_bytes);
            if stream.glyph_used_height > 0 {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.glyph_texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &stream.glyph_pixels,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(GLYPH_ATLAS_WIDTH),
                        rows_per_image: Some(stream.glyph_used_height),
                    },
                    wgpu::Extent3d {
                        width: GLYPH_ATLAS_WIDTH,
                        height: stream.glyph_used_height,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
        if !msdf_bytes.is_empty() {
            queue.write_buffer(&self.vertex_buffer, msdf_offset, msdf_bytes);
        }
        if !texture_bytes.is_empty() {
            queue.write_buffer(&self.vertex_buffer, texture_offset, texture_bytes);
        }
        let needs_clear = stream.needs_clear;
        let clear = stream.clear;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("uix-frame-encoder"),
        });
        // 先把轮廓边写入 RGBA8 MSDF atlas，再进入主 pass 采样。
        if !covers.is_empty() {
            self.glyph_cover.encode(
                device,
                queue,
                &mut encoder,
                &self.msdf_atlas_view,
                GLYPH_ATLAS_WIDTH,
                GLYPH_ATLAS_HEIGHT,
                &covers,
            )?;
        }
        let load = if needs_clear {
            wgpu::LoadOp::Clear(wgpu::Color {
                r: clear[0] as f64,
                g: clear[1] as f64,
                b: clear[2] as f64,
                a: clear[3] as f64,
            })
        } else {
            wgpu::LoadOp::Load
        };
        let stream = match which {
            ActiveTarget::Swapchain => &self.swapchain,
            ActiveTarget::Offscreen => &self.offscreen,
        };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("uix-main-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let mut bound: Option<BatchKind> = None;
            for batch in &stream.batches {
                if bound.as_ref().map(|kind| match (kind, &batch.kind) {
                    (BatchKind::Shape, BatchKind::Shape)
                    | (BatchKind::Clear, BatchKind::Clear)
                    | (BatchKind::Glyph, BatchKind::Glyph)
                    | (BatchKind::GlyphMsdf, BatchKind::GlyphMsdf) => true,
                    (
                        BatchKind::Texture {
                            bind_group: a,
                            additive: add_a,
                        },
                        BatchKind::Texture {
                            bind_group: b,
                            additive: add_b,
                        },
                    ) => a == b && add_a == add_b,
                    _ => false,
                }) != Some(true)
                {
                    match batch.kind {
                        BatchKind::Shape => {
                            pass.set_pipeline(&self.shape_pipeline);
                            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..glyph_offset));
                        }
                        BatchKind::Clear => {
                            pass.set_pipeline(&self.clear_pipeline);
                            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..glyph_offset));
                        }
                        BatchKind::Glyph => {
                            pass.set_pipeline(&self.glyph_pipeline);
                            pass.set_bind_group(0, &self.glyph_bind_group, &[]);
                            pass.set_vertex_buffer(
                                0,
                                self.vertex_buffer.slice(glyph_offset..msdf_offset),
                            );
                        }
                        BatchKind::GlyphMsdf => {
                            pass.set_pipeline(&self.glyph_msdf_pipeline);
                            pass.set_bind_group(0, &self.msdf_bind_group, &[]);
                            pass.set_vertex_buffer(
                                0,
                                self.vertex_buffer.slice(msdf_offset..texture_offset),
                            );
                        }
                        BatchKind::Texture {
                            bind_group,
                            additive,
                        } => {
                            if additive {
                                pass.set_pipeline(&self.texture_additive_pipeline);
                            } else {
                                pass.set_pipeline(&self.texture_pipeline);
                            }
                            let group = self
                                .frame_bind_groups
                                .get(bind_group as usize)
                                .ok_or_else(|| {
                                    Error::new(
                                        Errc::InvalidState,
                                        "wgpu texture blit bind group missing",
                                    )
                                })?;
                            pass.set_bind_group(0, group, &[]);
                            pass.set_vertex_buffer(0, self.vertex_buffer.slice(texture_offset..));
                        }
                    }
                    bound = Some(batch.kind);
                }
                let (x, y, scissor_width, scissor_height) =
                    physical_scissor((width, height), batch.viewport, batch.scissor);
                if scissor_width == 0 || scissor_height == 0 {
                    continue;
                }
                pass.set_scissor_rect(x, y, scissor_width, scissor_height);
                pass.draw(
                    batch.first_vertex..batch.first_vertex + batch.vertex_count,
                    0..1,
                );
            }
        }
        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn shape_oriented_quad(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        corners: [[f32; 2]; 4],
        locals: [[f32; 2]; 4],
        color_a: [f32; 4],
        color_b: [f32; 4],
        params0: [f32; 4],
        params1: [f32; 4],
        mode: u32,
        kind: BatchKind,
    ) {
        let stream = self.stream();
        let first = stream.shapes.len() as u32;
        let positions = [
            corners[0], corners[1], corners[2], corners[0], corners[2], corners[3],
        ];
        let local = [
            locals[0], locals[1], locals[2], locals[0], locals[2], locals[3],
        ];
        for (position, local) in positions.into_iter().zip(local) {
            stream.shapes.push(ShapeVertex {
                position: ndc(position[0], position[1], viewport),
                local,
                color_a,
                color_b,
                params0,
                params1,
                mode,
            });
        }
        stream.push_batch(kind, first, 6, viewport, scissor);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn shape_quad(
        &mut self,
        viewport: (f32, f32),
        scissor: Option<(i32, i32, i32, i32)>,
        rect: Rect,
        color_a: [f32; 4],
        color_b: [f32; 4],
        params0: [f32; 4],
        params1: [f32; 4],
        mode: u32,
        coordinates: LocalCoordinates,
        kind: BatchKind,
    ) {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let stream = self.stream();
        let first = stream.shapes.len() as u32;
        let positions = quad_positions(rect);
        let local = coordinates.values(rect);
        for (position, local) in positions.into_iter().zip(local) {
            stream.shapes.push(ShapeVertex {
                position: ndc(position[0], position[1], viewport),
                local,
                color_a,
                color_b,
                params0,
                params1,
                mode,
            });
        }
        stream.push_batch(kind, first, 6, viewport, scissor);
    }
}
