use std::sync::Arc;

use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler, event::{ElementState, KeyEvent, WindowEvent}, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, keyboard::{KeyCode, PhysicalKey}, window::{CursorGrabMode, Window, WindowId}
};

use crate::{camera::{Camera, CameraController, CameraUniform}, model::{DrawModel, Model, Vertex}, texture::Texture};

mod camera;
mod texture;
mod model;
mod resources;

struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: Arc<Window>,

    gbuf_render_pipeline: wgpu::RenderPipeline,
    depth_texture: Texture,
    normal_texture: Texture,
    color_texture: Texture,
    gbuf_bind_group: wgpu::BindGroup,
    lighting_render_pipeline: wgpu::RenderPipeline,

    camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    camera_controller: CameraController,

    model: Model
}

impl<'a> State<'a> {
    async fn new(window: Arc<Window>) -> State<'a> {
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(not(target_arch="wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch="wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).expect("Failed to create surface!");

        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false
            },
        ).await.expect("Failed to request adapter!");

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(), // Full list: https://docs.rs/wgpu/latest/wgpu/struct.Features.html
                // WebGL doesn't support all of wgpu's features, so if
                // we're building for the web, we'll have to disable some.
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                label: None,
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            },
        ).await.expect("Failed to get device!");

        let size = window.inner_size();

        let surface_caps = surface.get_capabilities(&adapter);
        // Shader code in this tutorial assumes an sRGB surface texture. Using a different
        // one will result in all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps.formats.iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        

        let camera = Camera::new(
            config.width as f32 / config.height as f32,
            45., 0.1, 100.
        );
        
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera);

        let camera_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[camera_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
            label: Some("camera_bind_group_layout"),
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }
            ],
            label: Some("camera_bind_group"),
        });

        let depth_texture = texture::Texture::create_gbuf_texture(&device, &config, "depth_texture", true);
        let normal_texture = texture::Texture::create_gbuf_texture(&device, &config, "normal_texture", false);
        let color_texture = texture::Texture::create_gbuf_texture(&device, &config, "color_texture", false);
        
        let g_buffer_shader = device.create_shader_module(wgpu::include_wgsl!("shaders/gBufferShader.wgsl"));
        let gbuf_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("G-Buffer Render Pipeline Layout"),
            bind_group_layouts: &[
                &camera_bind_group_layout
            ],
            push_constant_ranges: &[],
        });

        let gbuf_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("G-Buffer Render Pipeline"),
            layout: Some(&gbuf_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &g_buffer_shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    model::ModelVertex::desc()
                ],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &g_buffer_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None
        });

        let gbuf_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("G-Buffer Bind Group Layout"),
            entries: &[
                // 0: normal sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // 1: normal texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },

                // 2: color sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // 3: color texture
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                }
            ]
        });
        let gbuf_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &gbuf_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&normal_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&color_texture.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&color_texture.view),
                }
            ],
            label: Some("G-Buffer Bind Group"),
        });

        let lighting_shader = device.create_shader_module(wgpu::include_wgsl!("shaders/lightingShader.wgsl"));
        let lighting_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lighting Pipeline Layout"),
            bind_group_layouts: &[
                &gbuf_bind_group_layout
            ],
            push_constant_ranges: &[],
        });
        let lighting_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Lighting Pipeline"),
            layout: Some(&lighting_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &lighting_shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    model::ModelVertex::desc()
                ],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &lighting_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    // TODO
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None
        });

        let model = Model::load("teapot.obj", &device).await.expect("Failed to load model");

        State {
            surface,
            window,
            device,
            queue,
            size,
            config,

            gbuf_render_pipeline,
            depth_texture,
            normal_texture,
            color_texture,
            gbuf_bind_group,
            lighting_render_pipeline,

            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            camera_controller: CameraController::new(5.),

            model
        }
    }

    fn get_window(&self) -> &Window {
        &self.window
    }

    fn handle_event(&mut self, event: WindowEvent) {
        self.camera_controller.handle_event(&event, self.size);
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);

            self.camera.update_aspect(new_size.width as f32 / new_size.height as f32);
            self.camera_uniform.update_view_proj(&self.camera);
            self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));

            self.depth_texture = texture::Texture::create_gbuf_texture(&self.device, &self.config, "depth_texture", true);
            self.normal_texture = texture::Texture::create_gbuf_texture(&self.device, &self.config, "normal_texture", false);
            self.color_texture = texture::Texture::create_gbuf_texture(&self.device, &self.config, "color_texture", false);
        }
    }

    fn update(&mut self, delta_time: f32) {
        self.camera_controller.update_camera(&mut self.camera, delta_time);
        self.camera_uniform.update_view_proj(&self.camera);
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });
 
        // Create the renderpass which will clear the screen.
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        // If you wanted to call any drawing commands, they would go here.
        render_pass.set_pipeline(&self.gbuf_render_pipeline);

        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        
        render_pass.draw_model(&self.model);

        // End the renderpass.
        drop(render_pass);

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[derive(Default)]
struct App<'a> {
    state: Option<State<'a>>,
    window: Option<Arc<Window>>,
    last_draw: Option<std::time::Instant>
}

impl<'a> ApplicationHandler for App<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Create window object
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );

        let state = pollster::block_on(State::new(window.clone()));
        self.state = Some(state);
        self.window = Some(window.clone());

        window.set_cursor_grab(CursorGrabMode::Confined).expect("Failed to grab cursor");
        window.set_cursor_visible(false);

        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = self.state.as_mut().unwrap();
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                // Tell winit we want another frame after this
                state.get_window().request_redraw();

                let delta_time = match self.last_draw {
                    Some(last) => {
                        let now = std::time::Instant::now();
                        let delta = now.duration_since(last).as_secs_f32();
                        self.last_draw = Some(now);
                        delta
                    }
                    None => {
                        self.last_draw = Some(std::time::Instant::now());
                        0.0 // First frame, no delta time
                    }
                };
                
                state.update(delta_time);
                match state.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if it's lost or outdated
                    Err(
                        wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated,
                    ) => state.resize(state.size),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory | wgpu::SurfaceError::Other) => {
                        log::error!("OutOfMemory");
                        event_loop.exit();
                    }

                    // This happens when the a frame takes too long to present
                    Err(wgpu::SurfaceError::Timeout) => {
                        log::warn!("Surface timeout")
                    }
                }
            }
            WindowEvent::Resized(size) => {
                // Reconfigures the size of the surface. We do not re-render
                // here as this event is always followed up by redraw request.
                state.resize(size);
            }
            WindowEvent::KeyboardInput { event, .. } if event.physical_key == PhysicalKey::Code(KeyCode::Escape) => {
                // If the Escape key is pressed, we exit the application.
                println!("Escape key pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event: KeyEvent {
                physical_key: PhysicalKey::Code(KeyCode::F11), state: ElementState::Pressed, repeat: false, ..
            }, .. } => {
                // Toggle fullscreen mode
                if let Some(window) = self.window.as_ref() {
                    if window.fullscreen().is_some() {
                        window.set_fullscreen(None);
                        window.set_cursor_grab(CursorGrabMode::Confined).expect("Failed to grab cursor");
                    } else {
                        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                        window.set_cursor_grab(CursorGrabMode::None).expect("Failed to release cursor");
                    }
                }
            }
            WindowEvent::CursorMoved { .. } => {
                let center = winit::dpi::PhysicalPosition::new(
                    state.size.width as f64 / 2.0,
                    state.size.height as f64 / 2.0,
                );
                let _ = self.window.as_ref().unwrap().set_cursor_position(center); // Don't unwrap; this can fail.
                // Forward the event to state
                state.handle_event(event);
            }
            _ => state.handle_event(event),
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
fn main() {
    // wgpu uses `log` for logging, so initialize a logger with `env_logger`
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();

    // When the current loop iteration finishes, immediately begin a new
    // iteration regardless of whether or not new events are available to
    // process. Preferred for applications that want to render as fast as
    // possible, like games.
    event_loop.set_control_flow(ControlFlow::Poll);

    // When the current loop iteration finishes, suspend the thread until
    // another event arrives. Helps keeping CPU utilization low if nothing
    // is happening, which is preferred if the application might be idling in
    // the background.
    // event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}