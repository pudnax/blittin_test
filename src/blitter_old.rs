#[derive(Copy, Clone, Debug)]
pub enum ColourSpace {
    Linear,
    Rgbe,
}

pub struct Blitter {
    render_pipeline: wgpu::RenderPipeline,
    render_bind_group: wgpu::BindGroup,
    dest_format: wgpu::TextureFormat,
}

impl Blitter {
    pub fn new(
        device: &wgpu::Device,
        src: &wgpu::TextureView,
        src_space: ColourSpace,
        dest_format: wgpu::TextureFormat,
    ) -> Self {
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_str!("blit_old.wgsl").into()),
        });
        let render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
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
        Blitter {
            render_bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &render_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(src) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&device.create_sampler(&wgpu::SamplerDescriptor {
                        min_filter: wgpu::FilterMode::Linear,
                        mag_filter: wgpu::FilterMode::Linear,
                        ..Default::default()
                    })) },
                ],
            }),
            render_pipeline: device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[&render_bind_group_layout],
                    push_constant_ranges: &[],
                })),
                vertex: wgpu::VertexState {
                    module: &render_shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &render_shader,
                    entry_point: match (src_space, dest_format) {
                        // FIXME use sRGB viewFormats instead once the API stabilises
                        (ColourSpace::Linear, wgpu::TextureFormat::Bgra8Unorm) => "fs_main_linear_to_srgb",
                        (ColourSpace::Linear, wgpu::TextureFormat::Rgba8Unorm) => "fs_main_linear_to_srgb",
                        (ColourSpace::Linear, wgpu::TextureFormat::Bgra8UnormSrgb) => "fs_main", // format automatically performs sRGB encoding
                        (ColourSpace::Linear, wgpu::TextureFormat::Rgba8UnormSrgb) => "fs_main",
                        (ColourSpace::Linear, wgpu::TextureFormat::Rgba16Float) => "fs_main",
                        (ColourSpace::Rgbe, wgpu::TextureFormat::Rgba16Float) => "fs_main_rgbe_to_linear",
                        _ => panic!("Blitter: unrecognised conversion from {src_space:?} to {dest_format:?}")
                    },
                    targets: &[Some(dest_format.into())],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            }),
            dest_format,
        }
    }

    pub fn blit_with_viewport(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        (x, y, w, h): (f32, f32, f32, f32),
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_viewport(x, y, w, h, 0., 1.);
        render_pass.set_bind_group(0, &self.render_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}
