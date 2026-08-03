use super::*;

use super::*;

    pub(crate) fn ensure_atlas(&mut self, device: &ID3D11Device, need_w: u32, need_h: u32) -> Result<()> {
        let need_w = need_w.max(1);
        let need_h = need_h.max(1);
        let want_w = next_pow2_u32(need_w).clamp(ATLAS_MIN, ATLAS_MAX);
        let want_h = next_pow2_u32(need_h).clamp(ATLAS_MIN, ATLAS_MAX);
        if self.atlas_tex.is_some() && self.atlas_w >= want_w && self.atlas_h >= want_h {
            return Ok(());
        }
        // Grow to at least current size so we don't thrash on mixed glyph sizes.
        let w = next_pow2_u32(self.atlas_w.max(want_w)).clamp(ATLAS_MIN, ATLAS_MAX);
        let h = next_pow2_u32(self.atlas_h.max(want_h)).clamp(ATLAS_MIN, ATLAS_MAX);
        self.atlas_srv = None;
        self.atlas_tex = None;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut tex = None;
        unsafe {
            device
                .CreateTexture2D(&desc, None, Some(&mut tex))
                .map_err(|e| d3d_error("CreateTexture2D(atlas)", e))?;
        }
        let tex =
            tex.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no atlas texture"))?;
        let srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_R8_UNORM,
            ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                },
            },
        };
        let mut srv = None;
        unsafe {
            device
                .CreateShaderResourceView(&tex, Some(&srv_desc), Some(&mut srv))
                .map_err(|e| d3d_error("CreateShaderResourceView(atlas)", e))?;
        }
        let srv =
            srv.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no atlas SRV"))?;
        self.atlas_tex = Some(tex);
        self.atlas_srv = Some(srv);
        self.atlas_w = w;
        self.atlas_h = h;
        self.reset_atlas();
        Ok(())
    }

    pub(crate) fn reset_atlas(&mut self) {
        self.atlas_cursor = AtlasCursor {
            x: 0,
            y: 0,
            row_h: 0,
        };
        self.atlas_cache.clear();
    }

    pub(crate) fn ensure_glyph_vb(&mut self, device: &ID3D11Device, glyph_count: usize) -> Result<()> {
        if glyph_count <= self.vb_glyph_capacity {
            return Ok(());
        }
        let mut cap = self.vb_glyph_capacity.max(GLYPH_VB_INITIAL_GLYPHS);
        while cap < glyph_count {
            cap = cap.saturating_mul(2);
        }
        self.vb_glyph = create_dynamic_vb(device, cap * 6 * size_of::<GlyphVertex>())?;
        self.vb_glyph_capacity = cap;
        Ok(())
    }

    /// Returns `true` if `(gw, gh)` fits at the current shelf cursor without
    /// wrapping the atlas (may still advance to a new row).
    pub(crate) fn atlas_can_fit(&self, gw: u32, gh: u32) -> bool {
        if self.atlas_tex.is_none() || self.atlas_w == 0 || self.atlas_h == 0 {
            return false;
        }
        let mut x = self.atlas_cursor.x;
        let mut y = self.atlas_cursor.y;
        let mut row_h = self.atlas_cursor.row_h;
        if x + gw > self.atlas_w {
            x = 0;
            y += row_h;
            row_h = 0;
        }
        y + gh <= self.atlas_h && x + gw <= self.atlas_w && row_h.max(gh) <= self.atlas_h
    }

    /// Shelf-pack one glyph; caller must ensure space (flush/grow first).
    pub(crate) fn pack_glyph_unchecked(
        &mut self,
        context: &ID3D11DeviceContext,
        coverage: &[u8],
        cov_w: u32,
        cov_h: u32,
    ) -> Result<(f32, f32, f32, f32)> {
        let gw = cov_w.max(1);
        let gh = cov_h.max(1);
        if self.atlas_cursor.x + gw > self.atlas_w {
            self.atlas_cursor.x = 0;
            self.atlas_cursor.y += self.atlas_cursor.row_h;
            self.atlas_cursor.row_h = 0;
        }
        if self.atlas_cursor.y + gh > self.atlas_h || self.atlas_cursor.x + gw > self.atlas_w {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Pipeline: pack_glyph_unchecked called without space",
            ));
        }

        let ax = self.atlas_cursor.x;
        let ay = self.atlas_cursor.y;
        let expected = (gw as usize).saturating_mul(gh as usize);
        if coverage.len() < expected {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!(
                    "D3d11Pipeline: glyph coverage too small, got {}, need {expected}",
                    coverage.len()
                ),
            ));
        }

        let Some(tex) = self.atlas_tex.as_ref() else {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Pipeline: atlas texture missing",
            ));
        };

        self.atlas_upload.clear();
        self.atlas_upload.extend_from_slice(&coverage[..expected]);
        let box_ = D3D11_BOX {
            left: ax,
            top: ay,
            front: 0,
            right: ax + gw,
            bottom: ay + gh,
            back: 1,
        };
        unsafe {
            context.UpdateSubresource(
                tex,
                0,
                Some(&box_),
                self.atlas_upload.as_ptr().cast(),
                gw,
                0,
            );
        }
        #[cfg(test)]
        {
            self.atlas_upload_count = self.atlas_upload_count.saturating_add(1);
        }

        self.atlas_cursor.x = ax + gw + 1;
        self.atlas_cursor.row_h = self.atlas_cursor.row_h.max(gh + 1);

        let inv_w = 1.0 / self.atlas_w as f32;
        let inv_h = 1.0 / self.atlas_h as f32;
        Ok((
            ax as f32 * inv_w,
            ay as f32 * inv_h,
            (ax + gw) as f32 * inv_w,
            (ay + gh) as f32 * inv_h,
        ))
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the scalar coordinates mirror the fixed D3D11 vertex layout at this backend boundary"
    )]
    pub(crate) fn push_glyph_quad(
        verts: &mut Vec<GlyphVertex>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        rgba: [f32; 4],
    ) {
        let x1 = x + w;
        let y1 = y + h;
        verts.push(GlyphVertex {
            pos: [x, y],
            uv: [u0, v0],
            color: rgba,
        });
        verts.push(GlyphVertex {
            pos: [x1, y],
            uv: [u1, v0],
            color: rgba,
        });
        verts.push(GlyphVertex {
            pos: [x, y1],
            uv: [u0, v1],
            color: rgba,
        });
        verts.push(GlyphVertex {
            pos: [x, y1],
            uv: [u0, v1],
            color: rgba,
        });
        verts.push(GlyphVertex {
            pos: [x1, y],
            uv: [u1, v0],
            color: rgba,
        });
        verts.push(GlyphVertex {
            pos: [x1, y1],
            uv: [u1, v1],
            color: rgba,
        });
    }

    pub(crate) fn flush_glyph_batch(
        &mut self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
    ) -> Result<()> {
        if self.glyph_verts.is_empty() {
            return Ok(());
        }
        let glyph_count = self.glyph_verts.len() / 6;
        self.ensure_glyph_vb(device, glyph_count)?;
        let Some(srv) = self.atlas_srv.as_ref() else {
            return Err(Error::new(
                Errc::PlatformError,
                "D3d11Pipeline: atlas SRV missing",
            ));
        };

        let constants = GlyphConstants {
            viewport: [viewport_w, viewport_h],
            _pad0: [0.0, 0.0],
        };
        let mut mapped_cb = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            context
                .Map(
                    &self.cb_glyph,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut mapped_cb),
                )
                .map_err(|e| d3d_error("Map(cb_glyph)", e))?;
            std::ptr::copy_nonoverlapping(
                (&constants as *const GlyphConstants).cast::<u8>(),
                mapped_cb.pData.cast(),
                size_of::<GlyphConstants>(),
            );
            context.Unmap(&self.cb_glyph, 0);
        }

        let bytes = self.glyph_verts.len() * size_of::<GlyphVertex>();
        let mut mapped_vb = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            context
                .Map(
                    &self.vb_glyph,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut mapped_vb),
                )
                .map_err(|e| d3d_error("Map(vb_glyph)", e))?;
            std::ptr::copy_nonoverlapping(
                self.glyph_verts.as_ptr().cast::<u8>(),
                mapped_vb.pData.cast(),
                bytes,
            );
            context.Unmap(&self.vb_glyph, 0);
        }

        let stride = size_of::<GlyphVertex>() as u32;
        let offset = 0u32;
        let (sx, sy, sw, sh) =
            scissor.unwrap_or((0, 0, viewport_w.ceil() as i32, viewport_h.ceil() as i32));
        let rect = ::windows::Win32::Foundation::RECT {
            left: sx,
            top: sy,
            right: sx + sw.max(0),
            bottom: sy + sh.max(0),
        };
        unsafe {
            context.IASetInputLayout(&self.layout_glyph);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(self.vb_glyph.clone())),
                Some(&stride),
                Some(&offset),
            );
            context.VSSetShader(&self.vs_glyph, None);
            context.PSSetShader(&self.ps_glyph, None);
            context.VSSetConstantBuffers(0, Some(&[Some(self.cb_glyph.clone())]));
            context.PSSetShaderResources(0, Some(&[Some(srv.clone())]));
            context.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            context.RSSetState(&self.rasterizer);
            context.OMSetBlendState(&self.blend_premultiplied, None, 0xffff_ffff);
            context.RSSetScissorRects(Some(&[rect]));
            context.Draw(self.glyph_verts.len() as u32, 0);
            context.PSSetShaderResources(0, Some(&[None]));
        }
        self.glyph_verts.clear();
        Ok(())
    }

    pub fn draw_glyphs(
        &mut self,
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<()> {
        if glyphs.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        self.glyph_verts.clear();

        for g in glyphs {
            if g.w <= 0.0 || g.h <= 0.0 || g.cov_w == 0 || g.cov_h == 0 {
                continue;
            }
            let gw = g.cov_w.max(1);
            let gh = g.cov_h.max(1);
            if gw > ATLAS_MAX || gh > ATLAS_MAX {
                return Err(Error::new(
                    Errc::InvalidArgument,
                    format!("D3d11Pipeline: glyph {gw}x{gh} exceeds atlas max {ATLAS_MAX}"),
                ));
            }

            // Only exact payloads enter the persistent cache. This keeps the
            // retained CPU allocation bounded by packed atlas area even for
            // arbitrary Canvas callers that append unused coverage bytes.
            let cache_key = (g.coverage.len() == (gw as usize).saturating_mul(gh as usize))
                .then(|| GlyphAtlasKey::new(&g.coverage, g.cov_w, g.cov_h));
            if let Some(uv) = cache_key
                .as_ref()
                .and_then(|key| self.atlas_cache.get(key))
                .map(|entry| entry.uv)
            {
                Self::push_glyph_quad(
                    &mut self.glyph_verts,
                    g.x,
                    g.y,
                    g.w,
                    g.h,
                    uv.0,
                    uv.1,
                    uv.2,
                    uv.3,
                    g.rgba,
                );
                continue;
            }

            if self.atlas_tex.is_none() {
                self.ensure_atlas(device, gw, gh)?;
            }

            // Growing recreates the texture — flush any verts that still sample it.
            let want_w = next_pow2_u32(gw).clamp(ATLAS_MIN, ATLAS_MAX);
            let want_h = next_pow2_u32(gh).clamp(ATLAS_MIN, ATLAS_MAX);
            if want_w > self.atlas_w || want_h > self.atlas_h {
                self.flush_glyph_batch(device, context, viewport_w, viewport_h, scissor)?;
                self.ensure_atlas(device, gw, gh)?;
            }

            if !self.atlas_can_fit(gw, gh) {
                self.flush_glyph_batch(device, context, viewport_w, viewport_h, scissor)?;
                let grow_h =
                    next_pow2_u32(self.atlas_h.saturating_add(gh)).clamp(ATLAS_MIN, ATLAS_MAX);
                let grow_w = next_pow2_u32(self.atlas_w.max(gw)).clamp(ATLAS_MIN, ATLAS_MAX);
                if grow_h > self.atlas_h || grow_w > self.atlas_w {
                    self.ensure_atlas(device, grow_w, grow_h)?;
                }
                self.reset_atlas();
                if !self.atlas_can_fit(gw, gh) {
                    return Err(Error::new(
                        Errc::PlatformError,
                        "D3d11Pipeline: glyph does not fit in atlas after grow",
                    ));
                }
            }

            let uv = self.pack_glyph_unchecked(context, &g.coverage, g.cov_w, g.cov_h)?;
            if let Some(cache_key) = cache_key {
                self.atlas_cache.insert(
                    cache_key,
                    GlyphAtlasEntry {
                        _coverage: Arc::clone(&g.coverage),
                        uv,
                    },
                );
            }
            Self::push_glyph_quad(
                &mut self.glyph_verts,
                g.x,
                g.y,
                g.w,
                g.h,
                uv.0,
                uv.1,
                uv.2,
                uv.3,
                g.rgba,
            );
        }
        self.flush_glyph_batch(device, context, viewport_w, viewport_h, scissor)
    }

    #[cfg(test)]
    pub(crate) fn glyph_atlas_upload_count(&self) -> usize {
        self.atlas_upload_count
    }

    pub(crate) fn bind_rect_pipeline(
        &self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        replace_blend: bool,
    ) {
        let stride = (2 * size_of::<f32>()) as u32;
        let offset = 0u32;
        unsafe {
            context.IASetInputLayout(&self.layout);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.IASetVertexBuffers(
                0,
                1,
                Some(&Some(self.vb_unit.clone())),
                Some(&stride),
                Some(&offset),
            );
            context.VSSetShader(&self.vs_rect, None);
            context.PSSetShader(&self.ps_rect, None);
            context.VSSetConstantBuffers(0, Some(&[Some(self.cb.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(self.cb.clone())]));
            context.RSSetState(&self.rasterizer);
            if replace_blend {
                context.OMSetBlendState(&self.blend_replace, None, 0xffff_ffff);
            } else {
                context.OMSetBlendState(&self.blend_premultiplied, None, 0xffff_ffff);
            }
            let (sx, sy, sw, sh) =
                scissor.unwrap_or((0, 0, viewport_w.ceil() as i32, viewport_h.ceil() as i32));
            let rect = ::windows::Win32::Foundation::RECT {
                left: sx,
                top: sy,
                right: sx + sw.max(0),
                bottom: sy + sh.max(0),
            };
            context.RSSetScissorRects(Some(&[rect]));
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the shader constants stay explicit at the legacy D3D11 command boundary"
    )]
    pub(crate) fn draw_rect_constants(
        &self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        rgba: [f32; 4],
        radius: [f32; 4],
        half_stroke: f32,
    ) -> Result<()> {
        if w <= 0.0 || h <= 0.0 {
            return Ok(());
        }
        let constants = RectConstants {
            viewport: [viewport_w, viewport_h],
            _pad0: [0.0, 0.0],
            rect: [x, y, w, h],
            color: rgba,
            radius,
            stroke: [half_stroke.max(0.0), 0.0, 0.0, 0.0],
        };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            context
                .Map(&self.cb, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))
                .map_err(|e| d3d_error("Map(cb)", e))?;
            std::ptr::copy_nonoverlapping(
                (&constants as *const RectConstants).cast::<u8>(),
                mapped.pData.cast(),
                size_of::<RectConstants>(),
            );
            context.Unmap(&self.cb, 0);
            context.Draw(6, 0);
        }
        Ok(())
    }

    pub fn draw_solid_rects(
        &mut self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        if rects.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        self.bind_rect_pipeline(context, viewport_w, viewport_h, scissor, false);
        for rect in rects {
            self.draw_rect_constants(
                context,
                viewport_w,
                viewport_h,
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                rect.rgba,
                rect.radius,
                0.0,
            )?;
        }
        Ok(())
    }

    pub fn draw_stroke_rects(
        &mut self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuStrokeRect],
    ) -> Result<()> {
        if rects.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        self.bind_rect_pipeline(context, viewport_w, viewport_h, scissor, false);
        for rect in rects {
            let half = rect.line_width.max(0.0) * 0.5;
            if half <= 0.0 {
                continue;
            }
            self.draw_rect_constants(
                context,
                viewport_w,
                viewport_h,
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                rect.rgba,
                rect.radius,
                half,
            )?;
        }
        Ok(())
    }

    /// Replace-blend clear quads (partial dirty clear; D3D ClearRTV is full-surface).
    pub fn clear_rects(
        &mut self,
        context: &ID3D11DeviceContext,
        viewport_w: f32,
        viewport_h: f32,
        rects: &[GpuSolidRect],
    ) -> Result<()> {
        if rects.is_empty() || viewport_w <= 0.0 || viewport_h <= 0.0 {
            return Ok(());
        }
        self.bind_rect_pipeline(context, viewport_w, viewport_h, None, true);
        for rect in rects {
            self.draw_rect_constants(
                context,
                viewport_w,
                viewport_h,
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                rect.rgba,
                rect.radius,
                0.0,
            )?;
        }
        Ok(())
    }

