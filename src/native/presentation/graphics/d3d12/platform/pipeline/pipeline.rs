use super::*;

use super::*;

impl D3d12Pipeline {
    pub(super) fn new(device: &ID3D12Device, frame_count: usize) -> Result<Self> {
        let root_signature = create_root_signature(device)?;
        let rect_vs = compile_shader(RECT_HLSL, b"VSMain\0", b"vs_5_0\0")?;
        let rect_ps = compile_shader(RECT_HLSL, b"PSMain\0", b"ps_5_0\0")?;
        let blit_vs = compile_shader(BLIT_HLSL, b"VSMain\0", b"vs_5_0\0")?;
        let blit_ps = compile_shader(BLIT_HLSL, b"PSMain\0", b"ps_5_0\0")?;
        let glyph_vs = compile_shader(GLYPH_HLSL, b"VSMain\0", b"vs_5_0\0")?;
        let glyph_ps = compile_shader(GLYPH_HLSL, b"PSMain\0", b"ps_5_0\0")?;
        let glyph_input_layout = [
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR(c"POSITION".as_ptr().cast()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 0,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR(c"TEXCOORD".as_ptr().cast()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 8,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR(c"COLOR".as_ptr().cast()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 16,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
        ];
        let solid_pso = create_pso(
            device,
            &root_signature,
            &rect_vs,
            &rect_ps,
            &[],
            true,
            "CreateGraphicsPipelineState(solid)",
        )?;
        let soft_pso = create_pso(
            device,
            &root_signature,
            &blit_vs,
            &blit_ps,
            &[],
            true,
            "CreateGraphicsPipelineState(soft blit)",
        )?;
        let glyph_pso = create_pso(
            device,
            &root_signature,
            &glyph_vs,
            &glyph_ps,
            &glyph_input_layout,
            true,
            "CreateGraphicsPipelineState(glyph)",
        )?;
        let heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            NumDescriptors: SRV_DESCRIPTOR_COUNT,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
            NodeMask: 0,
        };
        let srv_heap = unsafe { device.CreateDescriptorHeap(&heap_desc) }
            .map_err(|error| pipeline_error("CreateDescriptorHeap(SRV)", error))?;
        let srv_stride = unsafe {
            device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV)
        };
        Ok(Self {
            root_signature,
            solid_pso,
            soft_pso,
            glyph_pso,
            srv_heap,
            srv_stride,
            soft_texture: None,
            soft_texture_state: D3D12_RESOURCE_STATE_COPY_DEST,
            soft_width: 0,
            soft_height: 0,
            glyph_atlas: None,
            glyph_atlas_state: D3D12_RESOURCE_STATE_COPY_DEST,
            glyph_state: GlyphAtlasState::default(),
            #[cfg(test)]
            glyph_atlas_upload_count: 0,
            frame_uploads: (0..frame_count).map(|_| FrameUploads::default()).collect(),
        })
    }

    pub(super) fn begin_frame(&mut self, frame_index: usize) {
        if let Some(frame) = self.frame_uploads.get_mut(frame_index) {
            frame.transient.clear();
            frame.used = 0;
        }
    }

    pub(crate) fn srv_cpu_handle(&self, slot: usize) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        let mut handle = unsafe { self.srv_heap.GetCPUDescriptorHandleForHeapStart() };
        handle.ptr += slot * self.srv_stride as usize;
        handle
    }

    pub(crate) fn srv_gpu_handle(&self, slot: usize) -> D3D12_GPU_DESCRIPTOR_HANDLE {
        let mut handle = unsafe { self.srv_heap.GetGPUDescriptorHandleForHeapStart() };
        handle.ptr += (slot * self.srv_stride as usize) as u64;
        handle
    }

    pub(super) fn draw_solid_rects(
        &self,
        list: &ID3D12GraphicsCommandList,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        if rects.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        if !viewport_w.is_finite() || !viewport_h.is_finite() {
            return Err(invalid_input("D3d12Pipeline: viewport must be finite"));
        }
        let viewport = D3D12_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: viewport_w,
            Height: viewport_h,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        let scissor = scissor_rect(viewport_w, viewport_h, scissor);
        if scissor.right <= scissor.left || scissor.bottom <= scissor.top {
            return Ok(());
        }
        unsafe {
            list.SetGraphicsRootSignature(&self.root_signature);
            list.SetPipelineState(&self.solid_pso);
            list.RSSetViewports(&[viewport]);
            list.RSSetScissorRects(&[scissor]);
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        }
        for rect in rects {
            if rect.w <= 0.0 || rect.h <= 0.0 {
                continue;
            }
            let constants = RectConstants {
                viewport: [viewport_w, viewport_h],
                _pad0: [0.0; 2],
                rect: [rect.x, rect.y, rect.w, rect.h],
                color: rect.rgba,
                radius: rect.radius,
                stroke: [0.0; 4],
            };
            unsafe {
                list.SetGraphicsRoot32BitConstants(
                    0,
                    RECT_ROOT_DWORDS,
                    (&constants as *const RectConstants).cast::<c_void>(),
                    0,
                );
                list.DrawInstanced(6, 1, 0, 0);
            }
        }
        Ok(())
    }

    pub(crate) fn ensure_soft_texture(
        &mut self,
        device: &ID3D12Device,
        width: i32,
        height: i32,
    ) -> Result<()> {
        if self.soft_texture.is_some() && self.soft_width == width && self.soft_height == height {
            return Ok(());
        }
        let texture = create_soft_texture(device, width, height)?;
        let srv_desc = D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
            Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
            Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D12_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                    PlaneSlice: 0,
                    ResourceMinLODClamp: 0.0,
                },
            },
        };
        unsafe {
            device.CreateShaderResourceView(
                &texture,
                Some(&srv_desc),
                self.srv_cpu_handle(SOFT_SRV_SLOT),
            );
        }
        self.soft_texture = Some(texture);
        self.soft_texture_state = D3D12_RESOURCE_STATE_COPY_DEST;
        self.soft_width = width;
        self.soft_height = height;
        Ok(())
    }

    pub(crate) fn ensure_glyph_atlas(&mut self, device: &ID3D12Device) -> Result<()> {
        if self.glyph_atlas.is_some() {
            return Ok(());
        }
        let texture = create_glyph_atlas(device)?;
        let srv_desc = D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_R8_UNORM,
            ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
            Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
            Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D12_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                    PlaneSlice: 0,
                    ResourceMinLODClamp: 0.0,
                },
            },
        };
        unsafe {
            device.CreateShaderResourceView(
                &texture,
                Some(&srv_desc),
                self.srv_cpu_handle(GLYPH_SRV_SLOT),
            );
        }
        self.glyph_atlas = Some(texture);
        self.glyph_atlas_state = D3D12_RESOURCE_STATE_COPY_DEST;
        self.glyph_state.reset();
        Ok(())
    }

    pub(crate) fn push_glyph_quad(
        vertices: &mut Vec<GlyphVertex>,
        glyph: &GpuGlyphBlit,
        placement: GlyphAtlasPlacement,
    ) {
        let (u0, v0, u1, v1) = placement.uv();
        let x0 = glyph.x;
        let y0 = glyph.y;
        let x1 = glyph.x + glyph.w;
        let y1 = glyph.y + glyph.h;
        let make = |pos, uv| GlyphVertex {
            pos,
            uv,
            color: glyph.rgba,
        };
        vertices.extend_from_slice(&[
            make([x0, y0], [u0, v0]),
            make([x1, y0], [u1, v0]),
            make([x0, y1], [u0, v1]),
            make([x0, y1], [u0, v1]),
            make([x1, y0], [u1, v0]),
            make([x1, y1], [u1, v1]),
        ]);
    }

    pub(crate) fn plan_glyph_segments(&mut self, glyphs: &[GpuGlyphBlit]) -> Result<Vec<GlyphSegment>> {
        let mut segments = Vec::new();
        let mut current = GlyphSegment::default();
        for glyph in glyphs {
            if glyph.w <= 0.0 || glyph.h <= 0.0 || glyph.cov_w == 0 || glyph.cov_h == 0 {
                continue;
            }
            let expected = (glyph.cov_w as usize)
                .checked_mul(glyph.cov_h as usize)
                .ok_or_else(|| invalid_input("D3d12Pipeline: glyph coverage size overflow"))?;
            validated_glyph_layout(glyph.cov_w, glyph.cov_h, glyph.coverage.len())?;
            let cache_key = (glyph.coverage.len() == expected)
                .then(|| GlyphAtlasKey::new(&glyph.coverage, glyph.cov_w, glyph.cov_h));
            if let Some(placement) = cache_key
                .as_ref()
                .and_then(|key| self.glyph_state.cache.get(key))
                .map(|entry| entry.placement)
            {
                Self::push_glyph_quad(&mut current.vertices, glyph, placement);
                continue;
            }

            if !self.glyph_state.can_fit(glyph.cov_w, glyph.cov_h) {
                if !current.vertices.is_empty() {
                    segments.push(std::mem::take(&mut current));
                }
                self.glyph_state.reset();
            }
            let placement = self
                .glyph_state
                .pack(glyph.cov_w, glyph.cov_h)
                .ok_or_else(|| {
                    Error::new(
                        Errc::PlatformError,
                        "D3d12Pipeline: glyph does not fit after atlas reset",
                    )
                })?;
            current.uploads.push(PendingGlyphUpload {
                placement,
                coverage: Arc::clone(&glyph.coverage),
            });
            if let Some(cache_key) = cache_key {
                self.glyph_state.cache.insert(
                    cache_key,
                    GlyphAtlasEntry {
                        _coverage: Arc::clone(&glyph.coverage),
                        placement,
                    },
                );
            }
            Self::push_glyph_quad(&mut current.vertices, glyph, placement);
        }
        if !current.vertices.is_empty() {
            segments.push(current);
        }
        Ok(segments)
    }

    pub(crate) fn layout_glyph_upload(segments: &mut [GlyphSegment]) -> Result<usize> {
        let mut cursor = 0usize;
        for segment in segments {
            if !segment.uploads.is_empty() {
                cursor = checked_align_up(
                    cursor,
                    D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT as usize,
                    "glyph footprint",
                )?;
                segment.texture_upload_offset = cursor;
                segment.texture_upload_height = segment
                    .uploads
                    .iter()
                    .map(|upload| upload.placement.y.saturating_add(upload.placement.height))
                    .max()
                    .unwrap_or(0);
                let byte_count = glyph_segment_staging_bytes(segment.texture_upload_height)?;
                cursor = cursor
                    .checked_add(byte_count)
                    .ok_or_else(|| invalid_input("D3d12Pipeline: glyph upload offset overflow"))?;
            }
            cursor = checked_align_up(cursor, 16, "glyph vertex buffer")?;
            segment.vertex_offset = cursor;
            segment.vertex_bytes = segment
                .vertices
                .len()
                .checked_mul(size_of::<GlyphVertex>())
                .ok_or_else(|| invalid_input("D3d12Pipeline: glyph vertex size overflow"))?;
            cursor = cursor
                .checked_add(segment.vertex_bytes)
                .ok_or_else(|| invalid_input("D3d12Pipeline: glyph vertex offset overflow"))?;
        }
        Ok(cursor)
    }

    pub(crate) fn acquire_glyph_upload(
        &mut self,
        device: &ID3D12Device,
        frame_index: usize,
        total_bytes: usize,
    ) -> Result<ID3D12Resource> {
        let frame = self.frame_uploads.get(frame_index).ok_or_else(|| {
            invalid_input(format!(
                "D3d12Pipeline: invalid glyph upload frame index {frame_index}"
            ))
        })?;
        let slot = frame.used;
        let retained_capacity = frame
            .buffers
            .iter()
            .try_fold(0usize, |total, buffer| total.checked_add(buffer.capacity));
        let current_slot_capacity = frame.buffers.get(slot).map_or(0, |buffer| buffer.capacity);
        if glyph_upload_fits_retained_budget(retained_capacity, current_slot_capacity, total_bytes)
        {
            return self.acquire_upload(device, frame_index, total_bytes);
        }
        let resource = create_upload_buffer(device, total_bytes)?;
        self.frame_uploads[frame_index]
            .transient
            .push(resource.clone());
        Ok(resource)
    }

    pub(crate) fn acquire_upload(
        &mut self,
        device: &ID3D12Device,
        frame_index: usize,
        total_bytes: usize,
    ) -> Result<ID3D12Resource> {
        let frame = self.frame_uploads.get_mut(frame_index).ok_or_else(|| {
            invalid_input(format!(
                "D3d12Pipeline: invalid upload frame index {frame_index}"
            ))
        })?;
        let slot = frame.used;
        let needs_buffer = frame
            .buffers
            .get(slot)
            .is_none_or(|buffer| buffer.capacity < total_bytes);
        if needs_buffer {
            let resource = create_upload_buffer(device, total_bytes)?;
            let buffer = UploadBuffer {
                resource,
                capacity: total_bytes,
            };
            if slot < frame.buffers.len() {
                frame.buffers[slot] = buffer;
            } else {
                frame.buffers.push(buffer);
            }
        }
        frame.used += 1;
        Ok(frame.buffers[slot].resource.clone())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the explicit frame and viewport inputs match the D3D12 recording boundary"
    )]
    pub(super) fn draw_glyphs(
        &mut self,
        device: &ID3D12Device,
        list: &ID3D12GraphicsCommandList,
        frame_index: usize,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        let result = self.draw_glyphs_inner(
            device,
            list,
            frame_index,
            viewport_w,
            viewport_h,
            scissor,
            glyphs,
        );
        if result.is_err() {
            // Planning updates the persistent cache before command recording.
            // On any failure, discard it so a later frame never samples an
            // entry whose upload may not have reached the queue.
            self.glyph_state.reset();
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_glyphs_inner(
        &mut self,
        device: &ID3D12Device,
        list: &ID3D12GraphicsCommandList,
        frame_index: usize,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        if glyphs.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        if !viewport_w.is_finite() || !viewport_h.is_finite() {
            return Err(invalid_input("D3d12Pipeline: viewport must be finite"));
        }
        let viewport = D3D12_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: viewport_w,
            Height: viewport_h,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        let scissor = scissor_rect(viewport_w, viewport_h, scissor);
        if scissor.right <= scissor.left || scissor.bottom <= scissor.top {
            return Ok(());
        }

        self.ensure_glyph_atlas(device)?;
        let mut segments = self.plan_glyph_segments(glyphs)?;
        if segments.is_empty() {
            return Ok(());
        }
        let total_bytes = Self::layout_glyph_upload(&mut segments)?;
        for segment in &segments {
            u32::try_from(segment.vertices.len()).map_err(|_| {
                invalid_input("D3d12Pipeline: glyph vertex count exceeds D3D12 range")
            })?;
            u32::try_from(segment.vertex_bytes).map_err(|_| {
                invalid_input("D3d12Pipeline: glyph vertex bytes exceed D3D12 range")
            })?;
        }
        let upload = self.acquire_glyph_upload(device, frame_index, total_bytes)?;

        let empty_read = D3D12_RANGE { Begin: 0, End: 0 };
        let mut mapped = std::ptr::null_mut();
        unsafe { upload.Map(0, Some(&empty_read), Some(&mut mapped)) }
            .map_err(|error| pipeline_error("ID3D12Resource::Map(glyph upload)", error))?;
        if mapped.is_null() {
            unsafe { upload.Unmap(0, None) };
            return Err(Error::new(
                Errc::PlatformError,
                "D3d12Pipeline: glyph upload Map returned null",
            ));
        }
        for segment in &segments {
            for pending in &segment.uploads {
                let row_bytes = pending.placement.width as usize;
                for row in 0..pending.placement.height as usize {
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            pending.coverage.as_ptr().add(row * row_bytes),
                            mapped.cast::<u8>().add(
                                segment.texture_upload_offset
                                    + (pending.placement.y as usize + row)
                                        * GLYPH_ATLAS_SIZE as usize
                                    + pending.placement.x as usize,
                            ),
                            row_bytes,
                        );
                    }
                }
            }
            unsafe {
                std::ptr::copy_nonoverlapping(
                    segment.vertices.as_ptr().cast::<u8>(),
                    mapped.cast::<u8>().add(segment.vertex_offset),
                    segment.vertex_bytes,
                );
            }
        }
        let written = D3D12_RANGE {
            Begin: 0,
            End: total_bytes,
        };
        unsafe { upload.Unmap(0, Some(&written)) };

        let atlas = self.glyph_atlas.as_ref().cloned().ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d12Pipeline: glyph atlas missing after creation",
            )
        })?;
        let constants = GlyphConstants {
            viewport: [viewport_w, viewport_h],
            _pad0: [0.0; 2],
        };
        let gpu_handle = self.srv_gpu_handle(GLYPH_SRV_SLOT);
        unsafe {
            list.SetDescriptorHeaps(&[Some(self.srv_heap.clone())]);
            list.SetGraphicsRootSignature(&self.root_signature);
            list.SetGraphicsRootDescriptorTable(1, gpu_handle);
            list.SetGraphicsRoot32BitConstants(
                0,
                (size_of::<GlyphConstants>() / size_of::<u32>()) as u32,
                (&constants as *const GlyphConstants).cast::<c_void>(),
                0,
            );
            list.SetPipelineState(&self.glyph_pso);
            list.RSSetViewports(&[viewport]);
            list.RSSetScissorRects(&[scissor]);
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        }

        for segment in &segments {
            if !segment.uploads.is_empty() {
                record_transition(
                    list,
                    &atlas,
                    self.glyph_atlas_state,
                    D3D12_RESOURCE_STATE_COPY_DEST,
                );
                self.glyph_atlas_state = D3D12_RESOURCE_STATE_COPY_DEST;
                for pending in &segment.uploads {
                    let footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                        Offset: segment.texture_upload_offset as u64,
                        Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                            Format: DXGI_FORMAT_R8_UNORM,
                            Width: GLYPH_ATLAS_SIZE,
                            Height: segment.texture_upload_height,
                            Depth: 1,
                            RowPitch: GLYPH_ATLAS_SIZE,
                        },
                    };
                    let mut destination = texture_copy_location_subresource(&atlas);
                    let mut source = texture_copy_location_footprint(&upload, footprint);
                    let source_box = D3D12_BOX {
                        left: pending.placement.x,
                        top: pending.placement.y,
                        front: 0,
                        right: pending.placement.x + pending.placement.width,
                        bottom: pending.placement.y + pending.placement.height,
                        back: 1,
                    };
                    unsafe {
                        list.CopyTextureRegion(
                            &destination,
                            pending.placement.x,
                            pending.placement.y,
                            0,
                            &source,
                            Some(&source_box),
                        );
                    }
                    release_copy_location(&mut destination);
                    release_copy_location(&mut source);
                    #[cfg(test)]
                    {
                        self.glyph_atlas_upload_count += 1;
                    }
                }
                record_transition(
                    list,
                    &atlas,
                    D3D12_RESOURCE_STATE_COPY_DEST,
                    D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                );
                self.glyph_atlas_state = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
            }
            let view = D3D12_VERTEX_BUFFER_VIEW {
                BufferLocation: unsafe { upload.GetGPUVirtualAddress() }
                    + segment.vertex_offset as u64,
                SizeInBytes: segment.vertex_bytes as u32,
                StrideInBytes: size_of::<GlyphVertex>() as u32,
            };
            unsafe {
                list.IASetVertexBuffers(0, Some(std::slice::from_ref(&view)));
                list.DrawInstanced(segment.vertices.len() as u32, 1, 0, 0);
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn glyph_atlas_upload_count(&self) -> usize {
        self.glyph_atlas_upload_count
    }

    #[cfg(test)]
    pub(crate) fn glyph_atlas_extent(&self) -> Option<(u32, u32)> {
        self.glyph_atlas
            .as_ref()
            .map(|_| (GLYPH_ATLAS_SIZE, GLYPH_ATLAS_SIZE))
    }

    pub(super) fn blit_soft_fallback(
        &mut self,
        device: &ID3D12Device,
        list: &ID3D12GraphicsCommandList,
        frame_index: usize,
        pixels: &[u32],
        width: i32,
        height: i32,
    ) -> Result<()> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| invalid_input("soft fallback pixel count overflow"))?;
        if pixels.len() < expected {
            return Err(invalid_input(format!(
                "soft fallback buffer too small, got {}, need {expected}",
                pixels.len()
            )));
        }
        let Some((x, y, upload_w, upload_h)) = visible_pixel_bounds(pixels, width, height) else {
            return Ok(());
        };
        let mut packed = Vec::with_capacity((upload_w as usize).saturating_mul(upload_h as usize));
        for row in y..y + upload_h {
            let start = row as usize * width as usize + x as usize;
            packed.extend_from_slice(&pixels[start..start + upload_w as usize]);
        }
        self.blit_soft_fallback_tile(
            device,
            list,
            frame_index,
            &packed,
            width,
            height,
            SoftFallbackTile::at_destination(x, y, upload_w, upload_h),
        )
    }

    #[allow(clippy::too_many_arguments)] // D3D12 command recording needs the device/list/frame plus API-neutral tile payload.
    pub(super) fn blit_soft_fallback_tile(
        &mut self,
        device: &ID3D12Device,
        list: &ID3D12GraphicsCommandList,
        frame_index: usize,
        pixels: &[u32],
        target_width: i32,
        target_height: i32,
        tile: SoftFallbackTile,
    ) -> Result<()> {
        if target_width <= 0 || target_height <= 0 {
            return Err(invalid_input(format!(
                "invalid soft target {target_width}x{target_height}"
            )));
        }
        tile.validate_payload(pixels)?;
        if tile.dst_x.saturating_add(tile.width) > target_width
            || tile.dst_y.saturating_add(tile.height) > target_height
        {
            return Err(invalid_input(format!(
                "soft tile {}x{} at {},{} exceeds {target_width}x{target_height}",
                tile.width, tile.height, tile.dst_x, tile.dst_y
            )));
        }
        let tile_pixels = (tile.width as usize)
            .checked_mul(tile.height as usize)
            .ok_or_else(|| invalid_input("soft fallback tile pixel count overflow"))?;
        let (row_pitch, total_bytes) = validated_soft_layout(tile.width, tile.height, tile_pixels)?;
        self.ensure_soft_texture(device, target_width, target_height)?;
        let upload = self.acquire_upload(device, frame_index, total_bytes)?;

        let empty_read = D3D12_RANGE { Begin: 0, End: 0 };
        let mut mapped = std::ptr::null_mut();
        unsafe { upload.Map(0, Some(&empty_read), Some(&mut mapped)) }
            .map_err(|error| pipeline_error("ID3D12Resource::Map(soft upload)", error))?;
        if mapped.is_null() {
            unsafe { upload.Unmap(0, None) };
            return Err(Error::new(
                Errc::PlatformError,
                "D3d12Pipeline: soft upload Map returned null",
            ));
        }
        let row_bytes = tile.width as usize * std::mem::size_of::<u32>();
        for row in 0..tile.height as usize {
            let source = row * tile.width as usize;
            unsafe {
                std::ptr::copy_nonoverlapping(
                    pixels.as_ptr().add(source).cast::<u8>(),
                    mapped.cast::<u8>().add(row * row_pitch),
                    row_bytes,
                );
            }
        }
        let written = D3D12_RANGE {
            Begin: 0,
            End: total_bytes,
        };
        unsafe { upload.Unmap(0, Some(&written)) };

        let texture = self.soft_texture.as_ref().cloned().ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d12Pipeline: soft texture missing after creation",
            )
        })?;
        record_transition(
            list,
            &texture,
            self.soft_texture_state,
            D3D12_RESOURCE_STATE_COPY_DEST,
        );
        let footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
            Offset: 0,
            Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                Width: tile.width as u32,
                Height: tile.height as u32,
                Depth: 1,
                RowPitch: row_pitch as u32,
            },
        };
        let mut destination = texture_copy_location_subresource(&texture);
        let mut source = texture_copy_location_footprint(&upload, footprint);
        unsafe {
            list.CopyTextureRegion(
                &destination,
                tile.dst_x as u32,
                tile.dst_y as u32,
                0,
                &source,
                None,
            );
        }
        release_copy_location(&mut destination);
        release_copy_location(&mut source);
        record_transition(
            list,
            &texture,
            D3D12_RESOURCE_STATE_COPY_DEST,
            D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
        );
        self.soft_texture_state = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;

        let viewport = D3D12_VIEWPORT {
            TopLeftX: tile.dst_x as f32,
            TopLeftY: tile.dst_y as f32,
            Width: tile.width as f32,
            Height: tile.height as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        let scissor = RECT {
            left: tile.dst_x,
            top: tile.dst_y,
            right: tile.dst_x + tile.width,
            bottom: tile.dst_y + tile.height,
        };
        let gpu_handle = self.srv_gpu_handle(SOFT_SRV_SLOT);
        unsafe {
            list.SetDescriptorHeaps(&[Some(self.srv_heap.clone())]);
            list.SetGraphicsRootSignature(&self.root_signature);
            list.SetPipelineState(&self.soft_pso);
            let uv_rect = [
                tile.dst_x as f32 / target_width as f32,
                tile.dst_y as f32 / target_height as f32,
                tile.width as f32 / target_width as f32,
                tile.height as f32 / target_height as f32,
            ];
            list.SetGraphicsRoot32BitConstants(
                0,
                uv_rect.len() as u32,
                uv_rect.as_ptr().cast::<c_void>(),
                0,
            );
            list.SetGraphicsRootDescriptorTable(1, gpu_handle);
            list.RSSetViewports(&[viewport]);
            list.RSSetScissorRects(&[scissor]);
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            list.DrawInstanced(6, 1, 0, 0);
        }
        Ok(())
    }

    pub(super) fn retain_gpu_objects_after_undrained_drop(&self) {
        std::mem::forget(self.root_signature.clone());
        std::mem::forget(self.solid_pso.clone());
        std::mem::forget(self.soft_pso.clone());
        std::mem::forget(self.glyph_pso.clone());
        std::mem::forget(self.srv_heap.clone());
        if let Some(texture) = self.soft_texture.as_ref() {
            std::mem::forget(texture.clone());
        }
        if let Some(texture) = self.glyph_atlas.as_ref() {
            std::mem::forget(texture.clone());
        }
        for frame in &self.frame_uploads {
            for upload in &frame.buffers {
                std::mem::forget(upload.resource.clone());
            }
            for upload in &frame.transient {
                std::mem::forget(upload.clone());
            }
        }
    }
}

