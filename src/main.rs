use std::borrow::Cow;
use wgpu::{Instance, Backends, PowerPreference, Features, Limits};
use wgpu::{SurfaceConfiguration, PresentMode, TextureUsages};
use wgpu::{RenderPipelineDescriptor, RequestAdapterOptions, DeviceDescriptor};
use wgpu::{ShaderModuleDescriptor, ShaderSource, VertexState, PrimitiveState};
use wgpu::{TextureViewDescriptor, FragmentState, MultisampleState};
use wgpu::{CommandEncoderDescriptor, RenderPassDescriptor};
use wgpu::{RenderPassColorAttachment, LoadOp, Operations, Color};
use wgpu::util::{DeviceExt, BufferInitDescriptor};
use winit::event::{Event, WindowEvent};
use winit::window::WindowBuilder;
use winit::event_loop::{EventLoop, ControlFlow};

fn main() {
    // Create the logger to use
    env_logger::init();

    // Create an event loop for window events
    let event_loop = EventLoop::new();

    // Create a window
    let window = WindowBuilder::new()
        .with_resizable(false)
        .build(&event_loop).unwrap();

    // Create new instance of WGPU using a first-tier supported backend
    // Eg: Vulkan + Metal + DX12 + Browser WebGPU
    let instance = Instance::new(Backends::PRIMARY);

    // Create a surface for our Window. Unsafe since this uses raw window
    // handles and the window must remain valid for the lifetime of the
    // created surface
    //
    // A Surface represents a platform-specific surface (e.g. a window) onto
    // which rendered images may be presented.
    let surface = unsafe { instance.create_surface(&window) };

    // Get a handle to a physical graphics and/or compute device
    let adapter = pollster::block_on(async {
        instance.request_adapter(&RequestAdapterOptions {
            // Request the high performance graphics adapter, eg. pick the
            // discrete GPU over the integrated GPU
            power_preference: PowerPreference::HighPerformance,

            // Don't force fallback, we don't want software rendering :D
            force_fallback_adapter: false,

            // Make sure the adapter we request can render on `surface`
            compatible_surface: Some(&surface),
        }).await.expect("Failed to find an appropriate adapter")
    });

    // Display renderer information
    let adapter_info = adapter.get_info();
    println!("Renderer: {:04x}:{:04x} | {} | {:?} | {:?}",
        adapter_info.vendor, adapter_info.device,
        adapter_info.name,
        adapter_info.device_type, adapter_info.backend);

    // Create the logical device and command queue
    let (device, queue) = pollster::block_on(async {
        adapter.request_device(&DeviceDescriptor {
            // Debug label for the device
            label: None,

            // Features that the device should support
            features: Features::empty(),

            // Limits that the device should support. If any limit is "better"
            // than the limit exposed by the adapter, creating a device will
            // panic.
            limits: Limits::default(),
        }, None).await.expect("Failed to create device")
    });

    // Load the shaders from disk
    let shader = device.create_shader_module(&ShaderModuleDescriptor {
        label:  None,
        source: ShaderSource::Wgsl(
            Cow::Borrowed(include_str!("shader.wgsl"))),
    });

    // Get the preferred texture format for the swapchain with the surface and
    // adapter we are using
    let swapchain_format = surface.get_preferred_format(&adapter).unwrap();

    #[repr(C)]
    #[derive(Copy, Clone, Debug)]
    struct Vertex {
        position: [f32; 3],
        color: [f32; 3],
    }

    let mut verts = Vec::new();
    for _ in 0..1_000_000 {
        verts.push(Vertex { position: [0.0, 0.5, 0.0], color: [1.0, 0.0, 0.0] });
        verts.push(Vertex { position: [-0.5, -0.5, 0.0], color: [0.0, 1.0, 0.0] });
        verts.push(Vertex { position: [0.5, -0.5, 0.0], color: [0.0, 0.0, 1.0] });
    }

    // Create a vertex buffer
    let vertex_buffer = device.create_buffer_init(
        &BufferInitDescriptor {
            label: None,
            contents: unsafe {
                std::slice::from_raw_parts(
                    verts.as_ptr() as *const u8,
                    std::mem::size_of_val(verts.as_slice()))
            },
            usage: wgpu::BufferUsages::VERTEX,
        }
    );

    // Create the vertex buffer layout, describing the shape of the vertex
    // buffer
    let vbl = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3,
            }
        ]
    };

    // Create a pipeline which applies the passes needed for rendering
    let render_pipeline = device
            .create_render_pipeline(&RenderPipelineDescriptor {
        // Debug label of the pipeline. This will show up in graphics debuggers
        // for easy identification.
        label:  None,

        // The layout of bind groups for this pipeline.
        layout: None,

        // The compiled vertex stage, its entry point, and the input buffers
        // layout.
        vertex: VertexState {
            // Compiled shader
            module: &shader,

            // Name of the function for the entry point
            entry_point: "vs_main",

            // Buffers to pass in
            buffers: &[vbl],
        },

        // The properties of the pipeline at the primitive assembly and
        // rasterization level.
        primitive: PrimitiveState::default(),

        // The compiled fragment stage, its entry point, and the color targets.
        fragment: Some(FragmentState {
            // Compiled shader
            module: &shader,

            // Name of the function for the entry point
            entry_point: "fs_main",

            // Type of output for the fragment shader (the correct texture
            // format that our GPU wants)
            targets: &[swapchain_format.into()],
        }),

        // The effect of draw calls on the depth and stencil aspects of the
        // output target, if any.
        depth_stencil: None,

        // The multi-sampling properties of the pipeline.
        multisample: MultisampleState::default(),

        // If the pipeline will be used with a multiview render pass, this
        // indicates how many array layers the attachments will have.
        multiview: None,
    });

    // Configure the swap buffers
    let size = window.inner_size();
    surface.configure(&device, &SurfaceConfiguration {
        // Usage for the swap chain. In this case, this is currently the only
        // supported option.
        usage: TextureUsages::RENDER_ATTACHMENT,

        // Set the preferred texture format for the swap chain to be what the
        // surface and adapter want.
        format: surface.get_preferred_format(&adapter).unwrap(),

        // Set the width of the swap chain
        width: size.width,

        // Set the height of the swap chain
        height: size.height,

        // The way data is presented to the screen
        // `Immediate` (no vsync)
        // `Mailbox`   (no vsync for rendering, but frames synced on vsync)
        // `Fifo`      (full vsync)
        present_mode: PresentMode::Immediate,
    });

    // Run the event loop
    let it = std::time::Instant::now();
    let mut frames = 0u64;
    event_loop.run(move |event, _, control_flow| {
        // ControlFlow::Wait pauses the event loop if no events are available
        // to process.  This is ideal for non-game applications that only
        // update in response to user input, and uses significantly less
        // power/CPU time than ControlFlow::Poll.
        *control_flow = ControlFlow::Wait;

        // Handle events
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                // Exit when the user closes the window
                *control_flow = ControlFlow::Exit;
            }
            Event::RedrawRequested(_) => {
                println!("[{:16.6}] redraw req", it.elapsed().as_secs_f64());

                // Redraw the application
                let frame = surface
                    .get_current_texture()
                    .expect("Failed to acquire next swap chain texture");
                println!("[{:16.6}] got frame", it.elapsed().as_secs_f64());

                // Create a view of the texture used in the frame
                let view = frame.texture
                    .create_view(&TextureViewDescriptor::default());
                println!("[{:16.6}] got view", it.elapsed().as_secs_f64());

                // An encoder for a series of GPU operations
                let mut encoder = device.create_command_encoder(
                    &CommandEncoderDescriptor::default());
                println!("[{:16.6}] got encoder", it.elapsed().as_secs_f64());

                {
                    // Start a render pass
                    let mut render_pass = encoder.begin_render_pass(
                        &RenderPassDescriptor {
                            label: None,
                            color_attachments: &[RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: Operations {
                                    load: LoadOp::Clear(Color::BLACK),
                                    store: true,
                                },
                            }],
                            depth_stencil_attachment: None,
                        });
                    println!("[{:16.6}] render pass",
                        it.elapsed().as_secs_f64());

                    // Pick the pipeline to use for rendering
                    render_pass.set_pipeline(&render_pipeline);
                    println!("[{:16.6}] set pipeline",
                        it.elapsed().as_secs_f64());

                    // Set the vertex buffer
                    render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                    println!("[{:16.6}] set vertex buffer",
                        it.elapsed().as_secs_f64());

                    // Draw!
                    render_pass.draw(0..verts.len() as u32, 0..1);
                    println!("[{:16.6}] drew",
                        it.elapsed().as_secs_f64());
                }

                // Finalize the encoder and submit the buffer for execution
                queue.submit(Some(encoder.finish()));
                println!("[{:16.6}] submit", it.elapsed().as_secs_f64());

                frame.present();
                println!("[{:16.6}] present", it.elapsed().as_secs_f64());

                frames += 1;
                println!("Frame {} | {}",
                    frames, frames as f64 / it.elapsed().as_secs_f64());
            },
            Event::MainEventsCleared => {
                window.request_redraw();
            },
            _ => {
                // Unhandled event
            }
        }
    });
}

