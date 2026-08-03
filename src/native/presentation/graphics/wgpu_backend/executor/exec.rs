use super::*;

impl WgpuExecutor {
    pub(crate) fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Result<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("uix-shared-2d-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../executor.wgsl").into()),
        });
        let glyph_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uix-glyph-atlas"),
            size: wgpu::Extent3d {
                width: GLYPH_ATLAS_WIDTH,
                height: GLYPH_ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let glyph_atlas_view = glyph_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let glyph_view = glyph_atlas_view.clone();
        let glyph_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("uix-glyph-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let glyph_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uix-glyph-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let glyph_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uix-glyph-bind-group"),
            layout: &glyph_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&glyph_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&glyph_sampler),
                },
            ],
        });
        let msdf_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uix-glyph-msdf-atlas"),
            size: wgpu::Extent3d {
                width: GLYPH_ATLAS_WIDTH,
                height: GLYPH_ATLAS_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let msdf_atlas_view = msdf_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let msdf_view = msdf_atlas_view.clone();
        let msdf_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uix-glyph-msdf-bind-group"),
            layout: &glyph_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&msdf_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&glyph_sampler),
                },
            ],
        });
        let texture_bind_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uix-texture-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("uix-texture-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let shape_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("uix-shape-pipeline-layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let glyph_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("uix-glyph-pipeline-layout"),
                bind_group_layouts: &[Some(&glyph_layout)],
                immediate_size: 0,
            });
        let texture_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("uix-texture-pipeline-layout"),
                bind_group_layouts: &[Some(&texture_bind_layout)],
                immediate_size: 0,
            });
        let target = Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        });
        const SHAPE_ATTRIBUTES: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Float32x4,
            3 => Float32x4,
            4 => Float32x4,
            5 => Float32x4,
            6 => Uint32
        ];
        const GLYPH_ATTRIBUTES: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];
        // stride 32：position(8)+uv(8)+opacity(4)+pad(12)，与 TextureVertex 一致。
        const TEXTURE_ATTRIBUTES: [wgpu::VertexAttribute; 3] = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 8,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 16,
                shader_location: 2,
            },
        ];
        let shape_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uix-shape-pipeline"),
            layout: Some(&shape_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("shape_vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<ShapeVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &SHAPE_ATTRIBUTES,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("shape_fs"),
                compilation_options: Default::default(),
                targets: std::slice::from_ref(&target),
            }),
            multiview_mask: None,
            cache: None,
        });
        let clear_target = Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            write_mask: wgpu::ColorWrites::ALL,
        });
        let clear_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uix-clear-pipeline"),
            layout: Some(&shape_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("shape_vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<ShapeVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &SHAPE_ATTRIBUTES,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("shape_fs"),
                compilation_options: Default::default(),
                targets: &[clear_target],
            }),
            multiview_mask: None,
            cache: None,
        });
        let glyph_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uix-glyph-pipeline"),
            layout: Some(&glyph_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("glyph_vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<GlyphVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &GLYPH_ATTRIBUTES,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("glyph_fs"),
                compilation_options: Default::default(),
                targets: std::slice::from_ref(&target),
            }),
            multiview_mask: None,
            cache: None,
        });
        let glyph_msdf_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uix-glyph-msdf-pipeline"),
            layout: Some(&glyph_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("glyph_vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<GlyphVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &GLYPH_ATTRIBUTES,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("glyph_msdf_fs"),
                compilation_options: Default::default(),
                targets: std::slice::from_ref(&target),
            }),
            multiview_mask: None,
            cache: None,
        });
        let texture_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("uix-texture-pipeline"),
            layout: Some(&texture_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("texture_vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<TextureVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &TEXTURE_ATTRIBUTES,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("texture_fs"),
                compilation_options: Default::default(),
                targets: std::slice::from_ref(&target),
            }),
            multiview_mask: None,
            cache: None,
        });
        let additive_target = Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            write_mask: wgpu::ColorWrites::ALL,
        });
        let texture_additive_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("uix-texture-additive-pipeline"),
                layout: Some(&texture_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("texture_vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: size_of::<TextureVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &TEXTURE_ATTRIBUTES,
                    })],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("texture_fs"),
                    compilation_options: Default::default(),
                    targets: &[additive_target],
                }),
                multiview_mask: None,
                cache: None,
            });
        let vertex_capacity = 4096;
        let vertex_buffer = create_vertex_buffer(device, vertex_capacity);
        let glyph_cover = GlyphCoverPipeline::new(device)?;
        Ok(Self {
            shape_pipeline,
            clear_pipeline,
            glyph_pipeline,
            glyph_msdf_pipeline,
            texture_pipeline,
            texture_additive_pipeline,
            glyph_texture,
            glyph_atlas_view,
            glyph_bind_group,
            msdf_texture,
            msdf_atlas_view,
            msdf_bind_group,
            glyph_cover,
            texture_bind_layout,
            texture_sampler,
            vertex_buffer,
            vertex_capacity,
            swapchain: DrawStream::new(),
            offscreen: DrawStream::new(),
            active: ActiveTarget::Swapchain,
            frame_bind_groups: Vec::new(),
            frame_textures: Vec::new(),
            surface_format: format,
            retained_color: None,
            overlay_backdrop: None,
        })
    }
}
