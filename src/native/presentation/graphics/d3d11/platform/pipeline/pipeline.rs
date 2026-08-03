use super::*;

impl D3d11Pipeline {
    pub(crate) fn new(device: &ID3D11Device) -> Result<Self> {
        let vs_blob = compile_shader(RECT_HLSL, c"VSMain", c"vs_4_0")?;
        let ps_blob = compile_shader(RECT_HLSL, c"PSMain", c"ps_4_0")?;
        let blit_vs_blob = compile_shader(BLIT_HLSL, c"VSMain", c"vs_4_0")?;
        let blit_ps_blob = compile_shader(BLIT_HLSL, c"PSMain", c"ps_4_0")?;
        let glyph_vs_blob = compile_shader(GLYPH_HLSL, c"VSMain", c"vs_4_0")?;
        let glyph_ps_blob = compile_shader(GLYPH_HLSL, c"PSMain", c"ps_4_0")?;
        let grad_vs_blob = compile_shader(GRADIENT_HLSL, c"VSMain", c"vs_4_0")?;
        let grad_ps_blob = compile_shader(GRADIENT_HLSL, c"PSMain", c"ps_4_0")?;
        let mesh_vs_blob = compile_shader(MESH_HLSL, c"VSMain", c"vs_4_0")?;
        let mesh_ps_blob = compile_shader(MESH_HLSL, c"PSMain", c"ps_4_0")?;
        let shadow_vs_blob = compile_shader(SHADOW_HLSL, c"VSMain", c"vs_4_0")?;
        let shadow_ps_blob = compile_shader(SHADOW_HLSL, c"PSMain", c"ps_4_0")?;

        let mut vs_rect = None;
        unsafe {
            device
                .CreateVertexShader(
                    std::slice::from_raw_parts(
                        vs_blob.GetBufferPointer() as *const u8,
                        vs_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut vs_rect),
                )
                .map_err(|e| d3d_error("CreateVertexShader(rect)", e))?;
        }
        let vs_rect =
            vs_rect.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no rect VS"))?;

        let mut ps_rect = None;
        unsafe {
            device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        ps_blob.GetBufferPointer() as *const u8,
                        ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut ps_rect),
                )
                .map_err(|e| d3d_error("CreatePixelShader(rect)", e))?;
        }
        let ps_rect =
            ps_rect.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no rect PS"))?;

        let mut vs_blit = None;
        unsafe {
            device
                .CreateVertexShader(
                    std::slice::from_raw_parts(
                        blit_vs_blob.GetBufferPointer() as *const u8,
                        blit_vs_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut vs_blit),
                )
                .map_err(|e| d3d_error("CreateVertexShader(blit)", e))?;
        }
        let vs_blit =
            vs_blit.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no blit VS"))?;

        let mut ps_blit = None;
        unsafe {
            device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        blit_ps_blob.GetBufferPointer() as *const u8,
                        blit_ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut ps_blit),
                )
                .map_err(|e| d3d_error("CreatePixelShader(blit)", e))?;
        }
        let ps_blit =
            ps_blit.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no blit PS"))?;

        // 可分离高斯模糊 PS（blur_offscreen_target 使用）。
        let blur_ps_blob = compile_shader(BLUR_HLSL, c"PSMain", c"ps_4_0")?;
        let mut ps_blur = None;
        unsafe {
            device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        blur_ps_blob.GetBufferPointer() as *const u8,
                        blur_ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut ps_blur),
                )
                .map_err(|e| d3d_error("CreatePixelShader(blur)", e))?;
        }
        let ps_blur =
            ps_blur.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no blur PS"))?;

        let mut vs_glyph = None;
        unsafe {
            device
                .CreateVertexShader(
                    std::slice::from_raw_parts(
                        glyph_vs_blob.GetBufferPointer() as *const u8,
                        glyph_vs_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut vs_glyph),
                )
                .map_err(|e| d3d_error("CreateVertexShader(glyph)", e))?;
        }
        let vs_glyph = vs_glyph
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no glyph VS"))?;

        let mut ps_glyph = None;
        unsafe {
            device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        glyph_ps_blob.GetBufferPointer() as *const u8,
                        glyph_ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut ps_glyph),
                )
                .map_err(|e| d3d_error("CreatePixelShader(glyph)", e))?;
        }
        let ps_glyph = ps_glyph
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no glyph PS"))?;

        let mut vs_grad = None;
        unsafe {
            device
                .CreateVertexShader(
                    std::slice::from_raw_parts(
                        grad_vs_blob.GetBufferPointer() as *const u8,
                        grad_vs_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut vs_grad),
                )
                .map_err(|e| d3d_error("CreateVertexShader(grad)", e))?;
        }
        let vs_grad =
            vs_grad.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no grad VS"))?;

        let mut ps_grad = None;
        unsafe {
            device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        grad_ps_blob.GetBufferPointer() as *const u8,
                        grad_ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut ps_grad),
                )
                .map_err(|e| d3d_error("CreatePixelShader(grad)", e))?;
        }
        let ps_grad =
            ps_grad.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no grad PS"))?;

        let mut vs_mesh = None;
        unsafe {
            device
                .CreateVertexShader(
                    std::slice::from_raw_parts(
                        mesh_vs_blob.GetBufferPointer() as *const u8,
                        mesh_vs_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut vs_mesh),
                )
                .map_err(|e| d3d_error("CreateVertexShader(mesh)", e))?;
        }
        let vs_mesh =
            vs_mesh.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no mesh VS"))?;

        let mut ps_mesh = None;
        unsafe {
            device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        mesh_ps_blob.GetBufferPointer() as *const u8,
                        mesh_ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut ps_mesh),
                )
                .map_err(|e| d3d_error("CreatePixelShader(mesh)", e))?;
        }
        let ps_mesh =
            ps_mesh.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no mesh PS"))?;

        let mut vs_shadow = None;
        unsafe {
            device
                .CreateVertexShader(
                    std::slice::from_raw_parts(
                        shadow_vs_blob.GetBufferPointer() as *const u8,
                        shadow_vs_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut vs_shadow),
                )
                .map_err(|e| d3d_error("CreateVertexShader(shadow)", e))?;
        }
        let vs_shadow = vs_shadow
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no shadow VS"))?;

        let mut ps_shadow = None;
        unsafe {
            device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        shadow_ps_blob.GetBufferPointer() as *const u8,
                        shadow_ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut ps_shadow),
                )
                .map_err(|e| d3d_error("CreatePixelShader(shadow)", e))?;
        }
        let ps_shadow = ps_shadow
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no shadow PS"))?;

        let input_elems = [D3D11_INPUT_ELEMENT_DESC {
            SemanticName: PCSTR::from_raw(c"POSITION".as_ptr().cast()),
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 0,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        }];
        let mut layout = None;
        unsafe {
            device
                .CreateInputLayout(
                    &input_elems,
                    std::slice::from_raw_parts(
                        vs_blob.GetBufferPointer() as *const u8,
                        vs_blob.GetBufferSize(),
                    ),
                    Some(&mut layout),
                )
                .map_err(|e| d3d_error("CreateInputLayout", e))?;
        }
        let layout = layout
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no input layout"))?;

        let glyph_elems = [
            D3D11_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR::from_raw(c"POSITION".as_ptr().cast()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 0,
                InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
            D3D11_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR::from_raw(c"TEXCOORD".as_ptr().cast()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 8,
                InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
            D3D11_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR::from_raw(c"COLOR".as_ptr().cast()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 16,
                InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
        ];
        let mut layout_glyph = None;
        unsafe {
            device
                .CreateInputLayout(
                    &glyph_elems,
                    std::slice::from_raw_parts(
                        glyph_vs_blob.GetBufferPointer() as *const u8,
                        glyph_vs_blob.GetBufferSize(),
                    ),
                    Some(&mut layout_glyph),
                )
                .map_err(|e| d3d_error("CreateInputLayout(glyph)", e))?;
        }
        let layout_glyph = layout_glyph.ok_or_else(|| {
            Error::new(Errc::PlatformError, "D3d11Pipeline: no glyph input layout")
        })?;

        let unit: [f32; 12] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0];
        let fullscreen: [f32; 12] = [
            -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0,
        ];
        let vb_unit = create_static_vb(device, &unit)?;
        let vb_fullscreen = create_static_vb(device, &fullscreen)?;
        let vb_glyph_capacity = GLYPH_VB_INITIAL_GLYPHS;
        let vb_glyph = create_dynamic_vb(device, vb_glyph_capacity * 6 * size_of::<GlyphVertex>())?;
        let vb_mesh_capacity_floats = MESH_VB_INITIAL_FLOATS;
        let vb_mesh = create_dynamic_vb(device, vb_mesh_capacity_floats * size_of::<f32>())?;

        let cb_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of::<RectConstants>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut cb = None;
        unsafe {
            device
                .CreateBuffer(&cb_desc, None, Some(&mut cb))
                .map_err(|e| d3d_error("CreateBuffer(cb)", e))?;
        }
        let cb = cb.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no CB"))?;

        let cb_blit_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of::<BlitConstants>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut cb_blit = None;
        unsafe {
            device
                .CreateBuffer(&cb_blit_desc, None, Some(&mut cb_blit))
                .map_err(|e| d3d_error("CreateBuffer(cb_blit)", e))?;
        }
        let cb_blit =
            cb_blit.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no blit CB"))?;

        let cb_blur_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of::<BlurConstants>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut cb_blur = None;
        unsafe {
            device
                .CreateBuffer(&cb_blur_desc, None, Some(&mut cb_blur))
                .map_err(|e| d3d_error("CreateBuffer(cb_blur)", e))?;
        }
        let cb_blur =
            cb_blur.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no blur CB"))?;

        let cb_glyph_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of::<GlyphConstants>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut cb_glyph = None;
        unsafe {
            device
                .CreateBuffer(&cb_glyph_desc, None, Some(&mut cb_glyph))
                .map_err(|e| d3d_error("CreateBuffer(cb_glyph)", e))?;
        }
        let cb_glyph = cb_glyph
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no glyph CB"))?;

        let cb_grad_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of::<GradientConstants>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut cb_grad = None;
        unsafe {
            device
                .CreateBuffer(&cb_grad_desc, None, Some(&mut cb_grad))
                .map_err(|e| d3d_error("CreateBuffer(cb_grad)", e))?;
        }
        let cb_grad =
            cb_grad.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no grad CB"))?;

        let cb_mesh_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of::<MeshConstants>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut cb_mesh = None;
        unsafe {
            device
                .CreateBuffer(&cb_mesh_desc, None, Some(&mut cb_mesh))
                .map_err(|e| d3d_error("CreateBuffer(cb_mesh)", e))?;
        }
        let cb_mesh =
            cb_mesh.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no mesh CB"))?;

        let cb_shadow_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of::<ShadowConstants>() as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: 0,
        };
        let mut cb_shadow = None;
        unsafe {
            device
                .CreateBuffer(&cb_shadow_desc, None, Some(&mut cb_shadow))
                .map_err(|e| d3d_error("CreateBuffer(cb_shadow)", e))?;
        }
        let cb_shadow = cb_shadow
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no shadow CB"))?;

        let mut blend_alpha = None;
        let alpha_desc = D3D11_BLEND_DESC {
            AlphaToCoverageEnable: FALSE,
            IndependentBlendEnable: FALSE,
            RenderTarget: [D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: TRUE,
                SrcBlend: D3D11_BLEND_SRC_ALPHA,
                DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            }; 8],
        };
        unsafe {
            device
                .CreateBlendState(&alpha_desc, Some(&mut blend_alpha))
                .map_err(|e| d3d_error("CreateBlendState", e))?;
        }
        let blend_alpha = blend_alpha
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no blend state"))?;

        let mut blend_premultiplied = None;
        let premultiplied_desc = D3D11_BLEND_DESC {
            AlphaToCoverageEnable: FALSE,
            IndependentBlendEnable: FALSE,
            RenderTarget: [D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: TRUE,
                SrcBlend: D3D11_BLEND_ONE,
                DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            }; 8],
        };
        unsafe {
            device
                .CreateBlendState(&premultiplied_desc, Some(&mut blend_premultiplied))
                .map_err(|e| d3d_error("CreateBlendState(premultiplied)", e))?;
        }
        let blend_premultiplied = blend_premultiplied.ok_or_else(|| {
            Error::new(
                Errc::PlatformError,
                "D3d11Pipeline: no premultiplied blend state",
            )
        })?;

        let mut blend_replace = None;
        let replace_desc = D3D11_BLEND_DESC {
            AlphaToCoverageEnable: FALSE,
            IndependentBlendEnable: FALSE,
            RenderTarget: [D3D11_RENDER_TARGET_BLEND_DESC {
                BlendEnable: FALSE,
                SrcBlend: D3D11_BLEND_ONE,
                DestBlend: D3D11_BLEND_ZERO,
                BlendOp: D3D11_BLEND_OP_ADD,
                SrcBlendAlpha: D3D11_BLEND_ONE,
                DestBlendAlpha: D3D11_BLEND_ZERO,
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            }; 8],
        };
        unsafe {
            device
                .CreateBlendState(&replace_desc, Some(&mut blend_replace))
                .map_err(|e| d3d_error("CreateBlendState(replace)", e))?;
        }
        let blend_replace = blend_replace
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no replace blend"))?;

        let mut rasterizer = None;
        let rs_desc = D3D11_RASTERIZER_DESC {
            FillMode: D3D11_FILL_SOLID,
            CullMode: D3D11_CULL_NONE,
            FrontCounterClockwise: FALSE,
            DepthBias: 0,
            DepthBiasClamp: 0.0,
            SlopeScaledDepthBias: 0.0,
            DepthClipEnable: FALSE,
            ScissorEnable: TRUE,
            MultisampleEnable: FALSE,
            AntialiasedLineEnable: FALSE,
        };
        unsafe {
            device
                .CreateRasterizerState(&rs_desc, Some(&mut rasterizer))
                .map_err(|e| d3d_error("CreateRasterizerState", e))?;
        }
        let rasterizer = rasterizer
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no rasterizer"))?;

        let mut sampler = None;
        let samp_desc = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_POINT,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            MipLODBias: 0.0,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            BorderColor: [0.0; 4],
            MinLOD: 0.0,
            MaxLOD: 0.0,
        };
        unsafe {
            device
                .CreateSamplerState(&samp_desc, Some(&mut sampler))
                .map_err(|e| d3d_error("CreateSamplerState", e))?;
        }
        let sampler =
            sampler.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no sampler"))?;

        Ok(Self {
            vs_rect,
            ps_rect,
            layout,
            vs_blit,
            ps_blit,
            ps_blur,
            vs_glyph,
            ps_glyph,
            layout_glyph,
            vs_grad,
            ps_grad,
            vs_mesh,
            ps_mesh,
            vs_shadow,
            ps_shadow,
            vb_unit,
            vb_fullscreen,
            vb_glyph,
            vb_glyph_capacity,
            vb_mesh,
            vb_mesh_capacity_floats,
            cb,
            cb_blit,
            cb_blur,
            cb_glyph,
            cb_grad,
            cb_mesh,
            cb_shadow,
            blend_alpha,
            blend_premultiplied,
            blend_replace,
            rasterizer,
            sampler,
            soft_tex: None,
            soft_srv: None,
            soft_w: 0,
            soft_h: 0,
            atlas_tex: None,
            atlas_srv: None,
            atlas_w: 0,
            atlas_h: 0,
            atlas_cursor: AtlasCursor {
                x: 0,
                y: 0,
                row_h: 0,
            },
            atlas_cache: HashMap::new(),
            atlas_upload_count: 0,
            atlas_upload: Vec::new(),
            glyph_verts: Vec::new(),
        })
    }

    pub(crate) fn ensure_soft_texture(
        &mut self,
        device: &ID3D11Device,
        width: i32,
        height: i32,
    ) -> Result<()> {
        let w = width.max(1);
        let h = height.max(1);
        if self.soft_tex.is_some() && self.soft_w == w && self.soft_h == h {
            return Ok(());
        }
        self.soft_srv = None;
        self.soft_tex = None;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w as u32,
            Height: h as u32,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
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
                .map_err(|e| d3d_error("CreateTexture2D(soft)", e))?;
        }
        let tex =
            tex.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no soft texture"))?;
        let srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
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
                .map_err(|e| d3d_error("CreateShaderResourceView", e))?;
        }
        let srv =
            srv.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no soft SRV"))?;
        self.soft_tex = Some(tex);
        self.soft_srv = Some(srv);
        self.soft_w = w;
        self.soft_h = h;
        Ok(())
    }
}
