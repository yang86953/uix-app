use super::*;

impl D3d11Pipeline {
    pub(crate) fn new(device: &ID3D11Device) -> Result<Self> {
        let vs_blob = compile_shader(RECT_HLSL, c"VSMain", c"vs_4_0")?;
        let ps_blob = compile_shader(RECT_HLSL, c"PSMain", c"ps_4_0")?;
        let blit_vs_blob = compile_shader(BLIT_HLSL, c"VSMain", c"vs_4_0")?;
        let glyph_vs_blob = compile_shader(GLYPH_HLSL, c"VSMain", c"vs_4_0")?;
        let glyph_ps_blob = compile_shader(GLYPH_HLSL, c"PSMain", c"ps_4_0")?;
        // 编译 RGBA8 MSDF 字形的共享 VS/PS 源，保证其 constant ABI 自洽。
        let msdf_vs_blob = compile_shader(MSDF_GLYPH_HLSL, c"VSMain", c"vs_4_0")?;
        // 编译 RGBA8 MSDF 字形的像素 shader。
        let msdf_ps_blob = compile_shader(MSDF_GLYPH_HLSL, c"PSMain", c"ps_4_0")?;
        let rhi_textured_ps_blob = compile_shader(RHI_TEXTURED_PS_HLSL, c"PSMain", c"ps_4_0")?;
        let grad_vs_blob = compile_shader(GRADIENT_HLSL, c"VSMain", c"vs_4_0")?;
        let grad_ps_blob = compile_shader(GRADIENT_HLSL, c"PSMain", c"ps_4_0")?;
        let mesh_vs_blob = compile_shader(MESH_HLSL, c"VSMain", c"vs_4_0")?;
        let mesh_ps_blob = compile_shader(MESH_HLSL, c"PSMain", c"ps_4_0")?;
        let shadow_vs_blob = compile_shader(SHADOW_HLSL, c"VSMain", c"vs_4_0")?;
        let shadow_ps_blob = compile_shader(SHADOW_HLSL, c"PSMain", c"ps_4_0")?;
        // 编译分析扇形 VS/PS，确保原生 sector 也在 RHI probe 中可用。
        let (vs_sector, ps_sector) = rhi_sector::create_sector_shaders(device)?;

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

        // 可分离高斯模糊 PS 只供通用 RHI BLUR_PASS 使用。
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

        // 创建 MSDF 字形 VS，输入布局与普通 glyph float8 ABI 完全一致。
        let mut vs_msdf = None;
        // SAFETY: MSDF shader blob 由本函数刚刚编译，指针和长度在调用期间有效。
        unsafe {
            device
                .CreateVertexShader(
                    std::slice::from_raw_parts(
                        msdf_vs_blob.GetBufferPointer() as *const u8,
                        msdf_vs_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut vs_msdf),
                )
                .map_err(|e| d3d_error("CreateVertexShader(msdf)", e))?;
        }
        // 拒绝驱动返回的空 MSDF VS；该局部 VS 只用于创建兼容输入布局。
        let _vs_msdf =
            vs_msdf.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no MSDF VS"))?;

        // 创建 MSDF 字形像素 shader。
        let mut ps_msdf = None;
        // SAFETY: MSDF shader blob 由本函数刚刚编译，指针和长度在调用期间有效。
        unsafe {
            device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        msdf_ps_blob.GetBufferPointer() as *const u8,
                        msdf_ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut ps_msdf),
                )
                .map_err(|e| d3d_error("CreatePixelShader(msdf)", e))?;
        }
        // 保存驱动创建出的 MSDF PS。
        let ps_msdf =
            ps_msdf.ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no MSDF PS"))?;

        // 创建薄 RHI 通用纹理 quad 的像素着色器。
        let mut ps_rhi_textured = None;
        // SAFETY: shader blob 由本函数刚刚编译，指针和长度在调用期间有效。
        unsafe {
            device
                .CreatePixelShader(
                    std::slice::from_raw_parts(
                        rhi_textured_ps_blob.GetBufferPointer() as *const u8,
                        rhi_textured_ps_blob.GetBufferSize(),
                    ),
                    None,
                    Some(&mut ps_rhi_textured),
                )
                .map_err(|e| d3d_error("CreatePixelShader(rhi_textured)", e))?;
        }
        // 拒绝驱动返回的空 shader。
        let ps_rhi_textured = ps_rhi_textured
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no RHI textured PS"))?;

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

        // 为 sampled Additive quad 创建源目标均为 ONE 的加法 blend。
        let mut blend_additive = None;
        // 描述颜色与 alpha 都执行 source + destination。
        let additive_desc = D3D11_BLEND_DESC {
            // Additive 不使用 alpha-to-coverage。
            AlphaToCoverageEnable: FALSE,
            // 所有 render target 使用相同的固定状态。
            IndependentBlendEnable: FALSE,
            // 设置第一个 render target 的加法因子。
            RenderTarget: [D3D11_RENDER_TARGET_BLEND_DESC {
                // 打开硬件 blend。
                BlendEnable: TRUE,
                // 累加源颜色。
                SrcBlend: D3D11_BLEND_ONE,
                // 累加目标颜色。
                DestBlend: D3D11_BLEND_ONE,
                // 颜色执行加法。
                BlendOp: D3D11_BLEND_OP_ADD,
                // 累加源 alpha。
                SrcBlendAlpha: D3D11_BLEND_ONE,
                // 累加目标 alpha。
                DestBlendAlpha: D3D11_BLEND_ONE,
                // alpha 执行加法。
                BlendOpAlpha: D3D11_BLEND_OP_ADD,
                // 保留四个颜色通道。
                RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
            }; 8],
        };
        // 在当前 D3D11 device 上创建加法状态对象。
        unsafe {
            device
                .CreateBlendState(&additive_desc, Some(&mut blend_additive))
                .map_err(|e| d3d_error("CreateBlendState(additive)", e))?;
        }
        // 驱动必须返回有效的加法状态对象。
        let blend_additive = blend_additive
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no additive blend"))?;

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

        Ok(Self {
            vs_rect,
            ps_rect,
            layout,
            vs_blit,
            ps_blur,
            vs_glyph,
            ps_glyph,
            ps_msdf,
            ps_rhi_textured,
            layout_glyph,
            vs_grad,
            ps_grad,
            vs_mesh,
            ps_mesh,
            vs_shadow,
            ps_shadow,
            vs_sector,
            ps_sector,
            blend_alpha,
            blend_premultiplied,
            blend_additive,
            blend_replace,
            rasterizer,
        })
    }
}
