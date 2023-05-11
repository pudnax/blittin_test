use anyhow::Result;
use pollster::FutureExt;
use wgpu::{util::DeviceExt, TextureFormat};
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
};

mod blitter_new;
mod blitter_old;
use blitter_old::Blitter;

fn main() -> Result<()> {
    env_logger::builder()
        .parse_env(env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"))
        .filter_module("wgpu_core", log::LevelFilter::Warn)
        .filter_module("wgpu_hal", log::LevelFilter::Warn)
        .filter_module("MANGOHUD", log::LevelFilter::Warn)
        .filter_module("winit", log::LevelFilter::Warn)
        .filter_module("naga", log::LevelFilter::Error)
        .init();

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("Voidin")
        .with_inner_size(LogicalSize::new(1280, 1024))
        // .with_resizable(false)
        // .with_decorations(false)
        .build(&event_loop)?;

    let PhysicalSize { width, height } = window.inner_size();

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        dx12_shader_compiler: wgpu::Dx12Compiler::Fxc,
    });

    let surface = unsafe { instance.create_surface(&window) }?;

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .block_on()
        .unwrap();

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("GPU Device"),
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::default(),
            },
            None,
        )
        .block_on()?;

    let mut surface_config = surface.get_default_config(&adapter, width, height).unwrap();
    surface_config.format = wgpu::TextureFormat::Bgra8Unorm;
    surface.configure(&device, &surface_config);

    let cat_pic = image::open("catfish.png")?.into_rgba8();
    let cat_format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut cat_texture_desc = wgpu::TextureDescriptor {
        label: Some("Catfish"),
        size: wgpu::Extent3d {
            width: cat_pic.width(),
            height: cat_pic.height(),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: cat_format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[
            cat_format.add_srgb_suffix(),
            cat_format.remove_srgb_suffix(),
        ],
    };
    let cat_texture_srgb =
        device.create_texture_with_data(&queue, &cat_texture_desc, cat_pic.as_raw());

    cat_texture_desc.format = cat_format.remove_srgb_suffix();
    let cat_texture_norm =
        device.create_texture_with_data(&queue, &cat_texture_desc, cat_pic.as_raw());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Cat BGL"),
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
    let create_bg = |tex: &wgpu::Texture, srgb| {
        let view = tex.create_view(&wgpu::TextureViewDescriptor {
            format: Some(if srgb {
                tex.format().add_srgb_suffix()
            } else {
                tex.format().remove_srgb_suffix()
            }),
            ..Default::default()
        });
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Cat BG"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        })
    };
    let tex_srgb_view_srgb = create_bg(&cat_texture_srgb, true);
    let tex_srgb_view_norm = create_bg(&cat_texture_srgb, false);
    let tex_norm_view_srgb = create_bg(&cat_texture_norm, true);
    let tex_norm_view_norm = create_bg(&cat_texture_norm, false);

    let shader = device.create_shader_module(wgpu::include_wgsl!("trig.wgsl"));
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Pipeline Desc"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main_full",
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(surface_config.format.into())],
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    let new_blitter = blitter_new::Blitter::new(&device);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::RedrawEventsCleared => window.request_redraw(),
            Event::RedrawRequested(_) => {
                let Ok(frame) = surface.get_current_texture() else { return; };
                let frame_view = frame.texture.create_view(&Default::default());

                let width = surface_config.width as f32;
                let height = surface_config.height as f32;
                let woff = width / 4.;
                let hoff = height / 3.;

                let create_old_blitter = |tex: &wgpu::Texture, format| {
                    Blitter::new(
                        &device,
                        &tex.create_view(&wgpu::TextureViewDescriptor {
                            format: Some(format),
                            ..Default::default()
                        }),
                        blitter_old::ColourSpace::Linear,
                        surface_config.format,
                    )
                };
                let blit_new =
                    |encoder: &mut wgpu::CommandEncoder, tex: &wgpu::Texture, format, dims| {
                        new_blitter.blit_to_texture(
                            encoder,
                            &device,
                            &tex.create_view(&wgpu::TextureViewDescriptor {
                                format: Some(format),
                                ..Default::default()
                            }),
                            &frame_view,
                            surface_config.format,
                            dims,
                        );
                    };

                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Main Render Scope"),
                });
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Main Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: None,
                });
                rpass.set_pipeline(&pipeline);
                rpass.set_viewport(0., 0., woff, hoff, 0., 1.);
                rpass.set_bind_group(0, &tex_srgb_view_srgb, &[]);
                rpass.draw(0..3, 0..1);

                rpass.set_viewport(woff, 0., woff, hoff, 0., 1.);
                rpass.set_bind_group(0, &tex_srgb_view_norm, &[]);
                rpass.draw(0..3, 0..1);

                rpass.set_viewport(2. * woff, 0., woff, hoff, 0., 1.);
                rpass.set_bind_group(0, &tex_norm_view_srgb, &[]);
                rpass.draw(0..3, 0..1);

                rpass.set_viewport(3. * woff, 0., woff, hoff, 0., 1.);
                rpass.set_bind_group(0, &tex_norm_view_norm, &[]);
                rpass.draw(0..3, 0..1);
                drop(rpass);

                let blitter = create_old_blitter(&cat_texture_srgb, TextureFormat::Rgba8UnormSrgb);
                blitter.blit_with_viewport(&mut encoder, &frame_view, (0., hoff, woff, hoff));
                let blitter = create_old_blitter(&cat_texture_srgb, TextureFormat::Rgba8Unorm);
                blitter.blit_with_viewport(&mut encoder, &frame_view, (woff, hoff, woff, hoff));
                let blitter = create_old_blitter(&cat_texture_srgb, TextureFormat::Rgba8UnormSrgb);
                blitter.blit_with_viewport(
                    &mut encoder,
                    &frame_view,
                    (2. * woff, hoff, woff, hoff),
                );
                let blitter = create_old_blitter(&cat_texture_srgb, TextureFormat::Rgba8Unorm);
                blitter.blit_with_viewport(
                    &mut encoder,
                    &frame_view,
                    (3. * woff, hoff, woff, hoff),
                );

                blit_new(
                    &mut encoder,
                    &cat_texture_srgb,
                    TextureFormat::Rgba8UnormSrgb,
                    (0., 2. * hoff, woff, hoff),
                );
                blit_new(
                    &mut encoder,
                    &cat_texture_srgb,
                    TextureFormat::Rgba8Unorm,
                    (woff, 2. * hoff, woff, hoff),
                );
                blit_new(
                    &mut encoder,
                    &cat_texture_norm,
                    TextureFormat::Rgba8UnormSrgb,
                    (2. * woff, 2. * hoff, woff, hoff),
                );
                blit_new(
                    &mut encoder,
                    &cat_texture_norm,
                    TextureFormat::Rgba8Unorm,
                    (3. * woff, 2. * hoff, woff, hoff),
                );

                queue.submit(Some(encoder.finish()));
                frame.present();
            }
            Event::WindowEvent {
                event:
                    WindowEvent::Resized(PhysicalSize { width, height })
                    | WindowEvent::ScaleFactorChanged {
                        new_inner_size: &mut PhysicalSize { width, height },
                        ..
                    },
                ..
            } => {
                if width != 0 && height != 0 {
                    surface_config.width = width;
                    surface_config.height = height;
                    surface.configure(&device, &surface_config);
                }
            }
            Event::WindowEvent {
                event:
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    },
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    })
}
