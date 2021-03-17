use chrono::Timelike;
use egui_wgpu_backend::epi::backend::AppOutput;
use egui_wgpu_backend::epi::IntegrationInfo;
use egui_wgpu_backend::wgpu::{
    BackendBit, CommandEncoderDescriptor, DeviceDescriptor, Features, Instance, Limits,
    PowerPreference, PresentMode, RequestAdapterOptions, SwapChainDescriptor, TextureFormat,
    TextureUsage,
};
use egui_wgpu_backend::{epi, wgpu, RenderPass, ScreenDescriptor};
use egui_winit_platform::{Platform, PlatformDescriptor};
use futures_lite::future::block_on;
use std::time::Instant;
use winit::event::WindowEvent;

struct RequestRepaintEvent;
struct WgpuRepaintSignal(std::sync::Mutex<winit::event_loop::EventLoopProxy<RequestRepaintEvent>>);
impl epi::RepaintSignal for WgpuRepaintSignal {
    fn request_repaint(&self) {
        self.0.lock().unwrap().send_event(RequestRepaintEvent).ok();
    }
}

pub fn run(mut app: Box<dyn epi::App>) -> ! {
    let event_loop = winit::event_loop::EventLoop::with_user_event();
    let name = app.name();
    let window = winit::window::WindowBuilder::new()
        .with_title(name)
        .build(&event_loop)
        .unwrap();
    let instance = Instance::new(BackendBit::PRIMARY);

    let surface = unsafe { instance.create_surface(&window) };

    let adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
    }))
    .unwrap();

    let (device, queue) = block_on(adapter.request_device(
        &DeviceDescriptor {
            features: Features::default(),
            limits: Limits::default(),
            label: None,
        },
        None,
    ))
    .unwrap();

    let size = window.inner_size();
    let mut sc_desc = SwapChainDescriptor {
        usage: TextureUsage::RENDER_ATTACHMENT,
        format: TextureFormat::Rgba8UnormSrgb,
        width: size.width,
        height: size.height,
        present_mode: PresentMode::Mailbox,
    };
    let mut swap_chain = device.create_swap_chain(&surface, &sc_desc);

    let repaint_signal = std::sync::Arc::new(WgpuRepaintSignal(std::sync::Mutex::new(
        event_loop.create_proxy(),
    )));

    let mut platform = Platform::new(PlatformDescriptor {
        physical_width: size.width,
        physical_height: size.height,
        scale_factor: window.scale_factor(),
        font_definitions: Default::default(),
        style: Default::default(),
    });
    let mut previous_frame_time = None;
    let mut egui_render_pass = RenderPass::new(&device, TextureFormat::Rgba8UnormSrgb);
    let start_time = Instant::now();
    #[cfg(feature = "http")]
    let http = std::sync::Arc::new(epi_http::EpiHttp {});

    event_loop.run(move |event, _, control_flow| {
        let mut redraw = || {
            platform.update_time(start_time.elapsed().as_secs_f64());

            let output_frame = match swap_chain.get_current_frame() {
                Ok(frame) => frame,
                Err(e) => {
                    eprintln!("Dropped frame with error: {}", e);
                    return;
                }
            };

            let pixel_pre_point = window.scale_factor();
            let frame_start = Instant::now();
            let mut app_output = epi::backend::AppOutput::default();
            let mut frame = epi::backend::FrameBuilder {
                info: IntegrationInfo {
                    web_info: None,
                    cpu_usage: previous_frame_time,
                    seconds_since_midnight: Some(seconds_since_midnight()),
                    native_pixels_per_point: Some(pixel_pre_point as _),
                },
                tex_allocator: &mut egui_render_pass,
                #[cfg(feature = "http")]
                http: http.clone(),
                output: &mut app_output,
                repaint_signal: repaint_signal.clone(),
            }
            .build();
            app.update(&platform.context(), &mut frame);
            let (egui_output, shapes) = platform.end_frame();
            let clipped_meshes = platform.context().tessellate(shapes);
            let frame_time = (Instant::now() - frame_start).as_secs_f32();
            previous_frame_time = Some(frame_time);
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("egui encoder"),
            });
            let screen_descriptor = ScreenDescriptor {
                physical_width: sc_desc.width,
                physical_height: sc_desc.height,
                scale_factor: window.scale_factor() as f32,
            };
            egui_render_pass.update_texture(&device, &queue, &platform.context().texture());
            egui_render_pass.update_user_textures(&device, &queue);
            egui_render_pass.update_buffers(&device, &queue, &clipped_meshes, &screen_descriptor);
            egui_render_pass.execute(
                &mut encoder,
                &output_frame.output.view,
                &clipped_meshes,
                &screen_descriptor,
                Some(wgpu::Color::BLACK),
            );
            queue.submit(std::iter::once(encoder.finish()));
            {
                let AppOutput { quit, window_size } = app_output;
                if let Some(window_size) = window_size {
                    window.set_inner_size(
                        winit::dpi::PhysicalSize {
                            width: (platform.context().pixels_per_point() * window_size.x).round(),
                            height: (platform.context().pixels_per_point() * window_size.y).round(),
                        }
                        .to_logical::<f32>(window.scale_factor()),
                    );
                }
                *control_flow = if quit {
                    winit::event_loop::ControlFlow::Exit
                } else if egui_output.needs_repaint {
                    window.request_redraw();
                    winit::event_loop::ControlFlow::Poll
                } else {
                    winit::event_loop::ControlFlow::Wait
                }
            }
        };

        match event {
            winit::event::Event::RedrawEventsCleared if cfg!(windows) => redraw(),
            winit::event::Event::RedrawRequested(_) if !cfg!(windows) => redraw(),
            winit::event::Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                sc_desc.width = size.width;
                sc_desc.height = size.height;
                swap_chain = device.create_swap_chain(&surface, &sc_desc);
            }
            winit::event::Event::MainEventsCleared
            | winit::event::Event::UserEvent(RequestRepaintEvent) => {
                platform.handle_event(&event);
                window.request_redraw()
            }
            _ => (),
        }
    });
}
/// Time of day as seconds since midnight. Used for clock in demo app.
pub fn seconds_since_midnight() -> f64 {
    let time = chrono::Local::now().time();
    time.num_seconds_from_midnight() as f64 + 1e-9 * (time.nanosecond() as f64)
}
