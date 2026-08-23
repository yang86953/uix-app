use super::*;

// 把共享顶点语义翻译为 D3D11 shader 输入名称。
fn d3d11_vertex_semantic(semantic: PipelineVertexSemantic) -> PCSTR {
    // 穷尽共享语义闭集，新增语义时必须显式映射。
    match semantic {
        // position 对应 HLSL POSITION。
        PipelineVertexSemantic::Position => PCSTR::from_raw(c"POSITION".as_ptr().cast()),
        // texture coordinate 对应 HLSL TEXCOORD。
        PipelineVertexSemantic::TextureCoordinate => PCSTR::from_raw(c"TEXCOORD".as_ptr().cast()),
        // color 对应 HLSL COLOR。
        PipelineVertexSemantic::Color => PCSTR::from_raw(c"COLOR".as_ptr().cast()),
        // coverage 复用 HLSL TEXCOORD0 标量输入。
        PipelineVertexSemantic::Coverage => PCSTR::from_raw(c"TEXCOORD".as_ptr().cast()),
    }
}

// 把共享顶点格式翻译为 D3D11 DXGI 格式。
fn d3d11_vertex_format(format: PipelineVertexFormat) -> DXGI_FORMAT {
    // 穷尽共享格式闭集，新增格式时必须显式映射。
    match format {
        // float1 对应一个三十二位浮点通道。
        PipelineVertexFormat::Float32 => DXGI_FORMAT_R32_FLOAT,
        // float2 对应两个三十二位浮点通道。
        PipelineVertexFormat::Float32x2 => DXGI_FORMAT_R32G32_FLOAT,
        // float4 对应四个三十二位浮点通道。
        PipelineVertexFormat::Float32x4 => DXGI_FORMAT_R32G32B32A32_FLOAT,
    }
}

// 从共享属性序列机械构造 D3D11 输入布局描述。
fn d3d11_vertex_elements(
    // 接收 pipeline 选择的唯一共享布局。
    layout: PipelineVertexLayout,
) -> Result<Vec<D3D11_INPUT_ELEMENT_DESC>> {
    // 共享布局必须在进入 Adapter 前满足槽位、偏移与步长边界。
    if !layout.is_valid() {
        // 返回稳定的类型化参数错误，禁止 D3D11 接受 OpenGL 拒绝的布局。
        return Err(Error::new(
            // 无效共享布局属于 RHI 参数契约错误。
            Errc::InvalidArgument,
            // 诊断不泄漏任何原生枚举。
            "D3d11 RHI vertex layout is invalid",
        ));
    }
    // 按共享顺序逐项翻译原生描述符。
    Ok(layout
        // 读取唯一属性序列。
        .attributes()
        // 以只读方式遍历共享事实。
        .iter()
        // 为每项创建一个 D3D11 描述符。
        .map(|attribute| D3D11_INPUT_ELEMENT_DESC {
            // 从共享语义机械选择 HLSL 名称。
            SemanticName: d3d11_vertex_semantic(attribute.semantic()),
            // 当前共享语义都使用零号语义索引。
            SemanticIndex: 0,
            // 从共享格式机械选择 DXGI 枚举。
            Format: d3d11_vertex_format(attribute.format()),
            // 当前薄 RHI 只暴露一个顶点输入槽。
            InputSlot: 0,
            // 直接使用共享属性字节偏移。
            AlignedByteOffset: attribute.offset_bytes(),
            // 所有共享属性都按顶点推进。
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            // 非实例化属性不使用步进率。
            InstanceDataStepRate: 0,
        })
        // 物化为 CreateInputLayout 使用的连续描述符数组。
        .collect())
}

// 把 API 无关混合因子翻译为 D3D11 枚举。
fn d3d11_blend_factor(factor: PipelineBlendFactor) -> D3D11_BLEND {
    // 只映射共享层允许的封闭因子集合。
    match factor {
        // Zero 对应 D3D11_BLEND_ZERO。
        PipelineBlendFactor::Zero => D3D11_BLEND_ZERO,
        // One 对应 D3D11_BLEND_ONE。
        PipelineBlendFactor::One => D3D11_BLEND_ONE,
        // SourceAlpha 对应 D3D11_BLEND_SRC_ALPHA。
        PipelineBlendFactor::SourceAlpha => D3D11_BLEND_SRC_ALPHA,
        // OneMinusSourceAlpha 对应 D3D11_BLEND_INV_SRC_ALPHA。
        PipelineBlendFactor::OneMinusSourceAlpha => D3D11_BLEND_INV_SRC_ALPHA,
    }
}

// 把 API 无关混合运算翻译为 D3D11 枚举。
fn d3d11_blend_operation(operation: PipelineBlendOperation) -> D3D11_BLEND_OP {
    // 只映射共享层允许的封闭运算集合。
    match operation {
        // Add 对应 D3D11_BLEND_OP_ADD。
        PipelineBlendOperation::Add => D3D11_BLEND_OP_ADD,
    }
}

// 把 API 无关颜色写掩码翻译为 D3D11 位集合。
fn d3d11_color_write_mask(mask: PipelineColorWriteMask) -> u8 {
    // 只映射共享层允许的封闭写掩码集合。
    match mask {
        // All 对应 D3D11 的完整 RGBA 写入位。
        PipelineColorWriteMask::All => D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
    }
}

// 把 API 无关面剔除模式翻译为 D3D11 枚举。
fn d3d11_cull_mode(cull_mode: PipelineCullMode) -> D3D11_CULL_MODE {
    // 只映射共享层允许的封闭剔除集合。
    match cull_mode {
        // None 对应不剔除任何绕序。
        PipelineCullMode::None => D3D11_CULL_NONE,
    }
}

// 把 API 无关正面绕序翻译为 D3D11 布尔值。
fn d3d11_front_counter_clockwise(front_face: PipelineFrontFace) -> BOOL {
    // 只映射共享层允许的封闭绕序集合。
    match front_face {
        // CounterClockwise 在 D3D11 中必须显式启用逆时针正面。
        PipelineFrontFace::CounterClockwise => TRUE,
    }
}

// 把 API 无关深度裁剪语义翻译为 D3D11 布尔值。
fn d3d11_depth_clip_enabled(depth_clip: PipelineDepthClip) -> BOOL {
    // 只映射共享层允许的封闭裁剪集合。
    match depth_clip {
        // Enabled 与 OpenGL ES 固定裁剪体保持一致。
        PipelineDepthClip::Enabled => TRUE,
    }
}

// 把 API 无关深度状态翻译为 D3D11 测试开关。
fn d3d11_depth_enabled(depth: PipelineDepthState) -> BOOL {
    // 只映射共享层允许的封闭深度集合。
    match depth {
        // Disabled 关闭深度测试。
        PipelineDepthState::Disabled => FALSE,
    }
}

// 把 API 无关深度状态翻译为 D3D11 写入开关。
fn d3d11_depth_write_mask(depth: PipelineDepthState) -> D3D11_DEPTH_WRITE_MASK {
    // 只映射共享层允许的封闭深度集合。
    match depth {
        // Disabled 同时关闭深度写入。
        PipelineDepthState::Disabled => D3D11_DEPTH_WRITE_MASK_ZERO,
    }
}

// 把 API 无关深度状态翻译为禁用时的稳定 D3D11 比较函数。
fn d3d11_depth_comparison(depth: PipelineDepthState) -> D3D11_COMPARISON_FUNC {
    // 只映射共享层允许的封闭深度集合。
    match depth {
        // Disabled 使用 Always，避免描述继续携带默认 Less 语义。
        PipelineDepthState::Disabled => D3D11_COMPARISON_ALWAYS,
    }
}

// 把 API 无关模板状态翻译为 D3D11 测试开关。
fn d3d11_stencil_enabled(stencil: PipelineStencilState) -> BOOL {
    // 只映射共享层允许的封闭模板集合。
    match stencil {
        // Disabled 关闭模板测试与写入。
        PipelineStencilState::Disabled => FALSE,
    }
}

// 把 API 无关采样覆盖状态翻译为 D3D11 alpha-to-coverage 开关。
fn d3d11_alpha_to_coverage_enabled(state: PipelineMultisampleState) -> BOOL {
    // 共享值对象已经穷尽定义每个变体的 coverage 事实。
    if state.alpha_to_coverage_enabled() {
        // 启用值映射为 D3D11 TRUE。
        TRUE
    } else {
        // 关闭值映射为 D3D11 FALSE。
        FALSE
    }
}

// 把 API 无关采样覆盖状态翻译为 D3D11 rasterizer 多样本开关。
fn d3d11_raster_multisample_enabled(state: PipelineMultisampleState) -> BOOL {
    // 共享值对象已经穷尽定义每个变体的 rasterizer 事实。
    if state.raster_multisample_enabled() {
        // 启用值映射为 D3D11 TRUE。
        TRUE
    } else {
        // 关闭值映射为 D3D11 FALSE。
        FALSE
    }
}

// 从共享二维状态创建一个 D3D11 rasterizer 对象。
fn create_rhi_rasterizer_state(
    // 借用当前 pipeline 所属的 D3D11 device。
    device: &ID3D11Device,
    // 接收 API 无关二维光栅状态。
    state: PipelineRasterState,
    // 接收 API 无关采样覆盖状态。
    multisample: PipelineMultisampleState,
) -> Result<ID3D11RasterizerState> {
    // 构造只做机械枚举翻译的原生描述。
    let desc = D3D11_RASTERIZER_DESC {
        // UIX 当前只提交实心三角形。
        FillMode: D3D11_FILL_SOLID,
        // 翻译共享面剔除语义。
        CullMode: d3d11_cull_mode(state.cull_mode),
        // 翻译共享正面绕序。
        FrontCounterClockwise: d3d11_front_counter_clockwise(state.front_face),
        // 二维 pipeline 不使用常量深度偏移。
        DepthBias: 0,
        // 二维 pipeline 不使用深度偏移钳制。
        DepthBiasClamp: 0.0,
        // 二维 pipeline 不使用斜率深度偏移。
        SlopeScaledDepthBias: 0.0,
        // 翻译与 OpenGL ES 对齐的共享深度裁剪语义。
        DepthClipEnable: d3d11_depth_clip_enabled(state.depth_clip),
        // 动态 scissor 由当前 DrawPacket 控制，rasterizer 必须允许该状态生效。
        ScissorEnable: TRUE,
        // 翻译共享 rasterizer 多样本语义。
        MultisampleEnable: d3d11_raster_multisample_enabled(multisample),
        // RHI 只提交三角形，不启用线抗锯齿。
        AntialiasedLineEnable: FALSE,
    };
    // 保存 D3D11 返回的状态对象。
    let mut native = None;
    // SAFETY：desc 完整初始化且输出指针指向当前栈帧中的 Option。
    unsafe {
        // 让 D3D11 device 创建不可变 rasterizer 对象。
        device
            // 传入共享状态机械翻译后的描述。
            .CreateRasterizerState(&desc, Some(&mut native))
            // 保留稳定原生操作名。
            .map_err(|error| d3d_error("CreateRasterizerState(RHI)", error))?;
    }
    // 驱动成功但没有返回对象仍属于平台错误。
    native.ok_or_else(|| {
        // 返回稳定的状态对象缺失错误。
        Error::new(
            // 状态对象缺失属于平台失败。
            Errc::PlatformError,
            // 保留固定诊断文本。
            "D3d11Pipeline: no RHI rasterizer",
        )
    })
}

// 从共享关闭状态创建一个 D3D11 depth-stencil 对象。
fn create_rhi_depth_stencil_state(
    // 借用当前 pipeline 所属的 D3D11 device。
    device: &ID3D11Device,
    // 接收 API 无关深度模板状态。
    state: PipelineDepthStencilState,
) -> Result<ID3D11DepthStencilState> {
    // 构造禁用模板时仍完整初始化的稳定正反面描述。
    let stencil_face = D3D11_DEPTH_STENCILOP_DESC {
        // 模板测试失败时保持原值。
        StencilFailOp: D3D11_STENCIL_OP_KEEP,
        // 深度测试失败时保持原值。
        StencilDepthFailOp: D3D11_STENCIL_OP_KEEP,
        // 两项测试通过时仍保持原值。
        StencilPassOp: D3D11_STENCIL_OP_KEEP,
        // 禁用模板时使用稳定 Always 比较。
        StencilFunc: D3D11_COMPARISON_ALWAYS,
    };
    // 构造只做机械枚举翻译的原生描述。
    let desc = D3D11_DEPTH_STENCIL_DESC {
        // 翻译共享深度测试开关。
        DepthEnable: d3d11_depth_enabled(state.depth),
        // 翻译共享深度写入开关。
        DepthWriteMask: d3d11_depth_write_mask(state.depth),
        // 翻译共享深度比较语义。
        DepthFunc: d3d11_depth_comparison(state.depth),
        // 翻译共享模板测试开关。
        StencilEnable: d3d11_stencil_enabled(state.stencil),
        // 禁用模板时不读取模板位。
        StencilReadMask: 0,
        // 禁用模板时不写入模板位。
        StencilWriteMask: 0,
        // 正面使用同一个稳定禁用描述。
        FrontFace: stencil_face,
        // 反面使用同一个稳定禁用描述。
        BackFace: stencil_face,
    };
    // 保存 D3D11 返回的状态对象。
    let mut native = None;
    // SAFETY：desc 完整初始化且输出指针指向当前栈帧中的 Option。
    unsafe {
        // 让 D3D11 device 创建不可变 depth-stencil 对象。
        device
            // 传入共享状态机械翻译后的描述。
            .CreateDepthStencilState(&desc, Some(&mut native))
            // 保留稳定原生操作名。
            .map_err(|error| d3d_error("CreateDepthStencilState(RHI)", error))?;
    }
    // 驱动成功但没有返回对象仍属于平台错误。
    native.ok_or_else(|| {
        // 返回稳定的状态对象缺失错误。
        Error::new(
            // 状态对象缺失属于平台失败。
            Errc::PlatformError,
            // 保留固定诊断文本。
            "D3d11Pipeline: no RHI depth-stencil state",
        )
    })
}

// 从共享状态创建一个 D3D11 blend 对象。
fn create_rhi_blend_state(
    // 借用当前 pipeline 所属的 D3D11 device。
    device: &ID3D11Device,
    // 接收 API 无关混合语义。
    blend: PipelineBlend,
    // 接收 API 无关采样覆盖状态。
    multisample: PipelineMultisampleState,
    // 接收稳定的原生错误操作名。
    operation: &'static str,
) -> Result<ID3D11BlendState> {
    // 从共享层读取完整颜色与 alpha 因子。
    let state = blend.state();
    // 构造只做机械枚举翻译的原生描述。
    let desc = D3D11_BLEND_DESC {
        // 翻译共享 alpha-to-coverage 语义。
        AlphaToCoverageEnable: d3d11_alpha_to_coverage_enabled(multisample),
        // 所有 render target 使用相同状态。
        IndependentBlendEnable: FALSE,
        // 设置第一目标以及 D3D11 要求的其余固定槽位。
        RenderTarget: [D3D11_RENDER_TARGET_BLEND_DESC {
            // 直接采用共享 enable 事实。
            BlendEnable: if state.enabled { TRUE } else { FALSE },
            // 翻译源 RGB 因子。
            SrcBlend: d3d11_blend_factor(state.source_color),
            // 翻译目标 RGB 因子。
            DestBlend: d3d11_blend_factor(state.destination_color),
            // 翻译共享 RGB 混合运算。
            BlendOp: d3d11_blend_operation(state.color_operation),
            // 翻译源 alpha 因子。
            SrcBlendAlpha: d3d11_blend_factor(state.source_alpha),
            // 翻译目标 alpha 因子。
            DestBlendAlpha: d3d11_blend_factor(state.destination_alpha),
            // 翻译共享 alpha 混合运算。
            BlendOpAlpha: d3d11_blend_operation(state.alpha_operation),
            // 翻译共享颜色通道写掩码。
            RenderTargetWriteMask: d3d11_color_write_mask(state.write_mask),
        }; 8],
    };
    // 保存 D3D11 返回的状态对象。
    let mut native = None;
    // SAFETY: desc 完整初始化且输出指针指向当前栈帧中的 Option。
    unsafe {
        // 让 D3D11 device 创建不可变状态对象。
        device
            // 传入共享状态机械翻译后的描述。
            .CreateBlendState(&desc, Some(&mut native))
            // 保留调用点对应的稳定原生操作名。
            .map_err(|error| d3d_error(operation, error))?;
    }
    // 驱动成功但没有返回对象仍属于平台错误。
    native.ok_or_else(|| {
        // 返回包含操作名的稳定错误。
        Error::new(
            // 状态对象缺失属于平台失败。
            Errc::PlatformError,
            // 记录具体混合状态身份。
            format!("D3d11Pipeline: {operation} returned no blend state"),
        )
    })
}

impl D3d11Pipeline {
    pub(crate) fn new(device: &ID3D11Device) -> Result<Self> {
        let vs_blob = compile_shader(RECT_HLSL, c"VSMain", c"vs_4_0")?;
        let ps_blob = compile_shader(RECT_HLSL, c"PSMain", c"ps_4_0")?;
        // BlurCB 与 BlitCB ABI 不同，必须编译 blur 自己的顶点 shader。
        let blur_vs_blob = compile_shader(BLUR_HLSL, c"VSMain", c"vs_4_0")?;
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
        // 编译解析线段 VS/PS，确保所有原生 Adapter 使用同一覆盖率公式。
        let (vs_line, ps_line) = rhi_line::create_line_shaders(device)?;

        let mut vs_rect = None;
        // SAFETY: vs_blob 为本函数刚编译成功的字节码，GetBufferPointer/GetBufferSize 在调用期间有效；device 存活；输出指针指向栈上 Option。
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
        // SAFETY: ps_blob 为本函数刚编译成功的字节码，指针与长度在调用期间有效；device 存活；输出指针指向栈上 Option。
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

        // 创建读取 BlurCB 区域与尺寸字段的专用顶点 shader。
        let mut vs_blur = None;
        // 从已编译 blur VS 字节码创建原生对象。
        // SAFETY: blur_vs_blob 为本函数刚编译成功的字节码，指针与长度在调用期间有效；device 存活；输出指针指向栈上 Option。
        unsafe {
            // D3D11 device 拥有 shader 生命周期。
            device
                // 使用 BLUR_HLSL 的 VSMain，而不是 ABI 不兼容的 blit VS。
                .CreateVertexShader(
                    // 借用完整已编译字节码。
                    std::slice::from_raw_parts(
                        // 取得字节码首地址。
                        blur_vs_blob.GetBufferPointer() as *const u8,
                        // 取得字节码长度。
                        blur_vs_blob.GetBufferSize(),
                    ),
                    // 不使用 class linkage。
                    None,
                    // 写入唯一 blur VS owner。
                    Some(&mut vs_blur),
                )
                // 保留平台创建失败的原始 HRESULT。
                .map_err(|e| d3d_error("CreateVertexShader(blur)", e))?;
        }
        // 创建成功后取得非空 shader。
        let vs_blur = vs_blur
            // 缺失对象属于稳定平台错误。
            .ok_or_else(|| Error::new(Errc::PlatformError, "D3d11Pipeline: no blur VS"))?;

        // 可分离高斯模糊 PS 只供通用 RHI BLUR_PASS 使用。
        let blur_ps_blob = compile_shader(BLUR_HLSL, c"PSMain", c"ps_4_0")?;
        let mut ps_blur = None;
        // SAFETY: blur_ps_blob 为本函数刚编译成功的字节码，指针与长度在调用期间有效；device 存活；输出指针指向栈上 Option。
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
        // SAFETY: glyph_vs_blob 为本函数刚编译成功的字节码，指针与长度在调用期间有效；device 存活；输出指针指向栈上 Option。
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
        // SAFETY: glyph_ps_blob 为本函数刚编译成功的字节码，指针与长度在调用期间有效；device 存活；输出指针指向栈上 Option。
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
        // SAFETY: grad_vs_blob 为本函数刚编译成功的字节码，指针与长度在调用期间有效；device 存活；输出指针指向栈上 Option。
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
        // SAFETY: grad_ps_blob 为本函数刚编译成功的字节码，指针与长度在调用期间有效；device 存活；输出指针指向栈上 Option。
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
        // SAFETY: mesh_vs_blob 为本函数刚编译成功的字节码，指针与长度在调用期间有效；device 存活；输出指针指向栈上 Option。
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
        // SAFETY: mesh_ps_blob 为本函数刚编译成功的字节码，指针与长度在调用期间有效；device 存活；输出指针指向栈上 Option。
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
        // SAFETY: shadow_vs_blob 为本函数刚编译成功的字节码，指针与长度在调用期间有效；device 存活；输出指针指向栈上 Option。
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
        // SAFETY: shadow_ps_blob 为本函数刚编译成功的字节码，指针与长度在调用期间有效；device 存活；输出指针指向栈上 Option。
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

        // 从共享 position float2 属性序列创建基础输入布局。
        let input_elems = d3d11_vertex_elements(PipelineVertexLayout::PositionF32x2)?;
        let mut layout = None;
        // SAFETY: input_elems 为栈上完整初始化的描述数组；vs_blob 字节码指针与长度在调用期间有效；输出指针指向栈上 Option。
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

        // 实心网格使用独立的 position + coverage 输入布局。
        let mesh_elems = d3d11_vertex_elements(PipelineVertexLayout::PositionCoverageF32)?;
        let mut layout_mesh = None;
        // SAFETY: mesh_elems 与 mesh VS 的 POSITION/TEXCOORD0 签名严格匹配，blob 在调用期间存活。
        unsafe {
            device
                .CreateInputLayout(
                    &mesh_elems,
                    std::slice::from_raw_parts(
                        mesh_vs_blob.GetBufferPointer() as *const u8,
                        mesh_vs_blob.GetBufferSize(),
                    ),
                    Some(&mut layout_mesh),
                )
                .map_err(|e| d3d_error("CreateInputLayout(mesh)", e))?;
        }
        let layout_mesh = layout_mesh.ok_or_else(|| {
            Error::new(Errc::PlatformError, "D3d11Pipeline: no mesh input layout")
        })?;

        // 从共享 position/uv-float4 属性序列创建 Blur 专用输入布局。
        let blur_elems = d3d11_vertex_elements(PipelineVertexLayout::PositionUvF32)?;
        let mut layout_blur = None;
        // SAFETY: blur_elems 完整初始化；blur_vs_blob 在调用期间存活且来自相同 VSMain。
        unsafe {
            device
                .CreateInputLayout(
                    &blur_elems,
                    std::slice::from_raw_parts(
                        blur_vs_blob.GetBufferPointer() as *const u8,
                        blur_vs_blob.GetBufferSize(),
                    ),
                    Some(&mut layout_blur),
                )
                .map_err(|e| d3d_error("CreateInputLayout(blur)", e))?;
        }
        let layout_blur = layout_blur.ok_or_else(|| {
            Error::new(Errc::PlatformError, "D3d11Pipeline: no blur input layout")
        })?;

        // 从共享 position/uv/color 属性序列创建采样输入布局。
        let glyph_elems = d3d11_vertex_elements(PipelineVertexLayout::PositionUvColorF32)?;
        let mut layout_glyph = None;
        // SAFETY: glyph_elems 为栈上完整初始化的描述数组；glyph_vs_blob 字节码指针与长度在调用期间有效；输出指针指向栈上 Option。
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

        // 冻结 blend、rasterizer 与输出合并阶段共用的单样本覆盖语义。
        let multisample_state = PipelineMultisampleState::SingleSample;
        // 由共享 straight-alpha 因子创建 D3D11 状态对象。
        let blend_alpha = create_rhi_blend_state(
            // 使用当前 pipeline device。
            device,
            // 选择 straight-alpha 语义。
            PipelineBlend::StraightAlpha,
            // 使用同一个共享单样本覆盖语义。
            multisample_state,
            // 保留稳定诊断名。
            "CreateBlendState(straight-alpha)",
        )?;
        // 由共享 premultiplied-alpha 因子创建 D3D11 状态对象。
        let blend_premultiplied = create_rhi_blend_state(
            // 使用当前 pipeline device。
            device,
            // 选择 premultiplied-alpha 语义。
            PipelineBlend::PremultipliedAlpha,
            // 使用同一个共享单样本覆盖语义。
            multisample_state,
            // 保留稳定诊断名。
            "CreateBlendState(premultiplied-alpha)",
        )?;
        // 由共享 Additive 因子创建 D3D11 状态对象。
        let blend_additive = create_rhi_blend_state(
            // 使用当前 pipeline device。
            device,
            // 选择 Additive 语义。
            PipelineBlend::Additive,
            // 使用同一个共享单样本覆盖语义。
            multisample_state,
            // 保留稳定诊断名。
            "CreateBlendState(additive)",
        )?;
        // 由共享 Replace 状态创建关闭混合的 D3D11 对象。
        let blend_replace = create_rhi_blend_state(
            // 使用当前 pipeline device。
            device,
            // 选择 Replace 语义。
            PipelineBlend::Replace,
            // 使用同一个共享单样本覆盖语义。
            multisample_state,
            // 保留稳定诊断名。
            "CreateBlendState(replace)",
        )?;

        // 冻结全部现有 pipeline 共用的共享二维光栅语义。
        let raster_state = PIPELINE_RASTER_2D;
        // 由共享二维状态创建唯一 D3D11 rasterizer 对象。
        let rasterizer = create_rhi_rasterizer_state(device, raster_state, multisample_state)?;
        // 冻结全部现有 pipeline 共用的共享关闭深度模板语义。
        let depth_stencil_state = PIPELINE_DEPTH_STENCIL_DISABLED;
        // 由共享关闭状态创建唯一 D3D11 depth-stencil 对象。
        let depth_stencil = create_rhi_depth_stencil_state(device, depth_stencil_state)?;

        Ok(Self {
            vs_rect,
            ps_rect,
            layout,
            // 保存与 BlurCB ABI 匹配的专用顶点 shader。
            vs_blur,
            layout_blur,
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
            layout_mesh,
            vs_shadow,
            ps_shadow,
            vs_sector,
            ps_sector,
            vs_line,
            ps_line,
            blend_alpha,
            blend_premultiplied,
            blend_additive,
            blend_replace,
            // 保留三个 D3D11 固定状态阶段共同使用的共享采样覆盖身份。
            multisample_state,
            rasterizer,
            // 保留原生 rasterizer 的共享创建身份。
            raster_state,
            // 保持显式 depth-stencil 对象的生命周期。
            depth_stencil,
            // 保留原生 depth-stencil 的共享创建身份。
            depth_stencil_state,
        })
    }
}
