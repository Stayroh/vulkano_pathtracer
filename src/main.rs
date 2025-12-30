mod rgen {
    vulkano_shaders::shader! {
        ty: "raygen",
        path: "src/shaders/rgen.glsl",
        vulkan_version: "1.3",
    }
}

mod rchit {
    vulkano_shaders::shader! {
        ty: "closesthit",
        path: "src/shaders/rchit.glsl",
        vulkan_version: "1.3",  
    }
}

mod rmiss {
    vulkano_shaders::shader! {
        ty: "miss",
        path: "src/shaders/rmiss.glsl",
        vulkan_version: "1.3",
    }
}


use anyhow::{Context, Error, Result};
use vulkano::image::{Image, ImageCreateInfo, ImageType};
use vulkano::image::view::ImageView;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator};
use std::default;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::{Duration, Instant};
use vulkano::{
    VulkanLibrary,
    buffer::BufferContents,
    device::{
        Device, DeviceCreateInfo, DeviceExtensions, Queue, QueueCreateInfo, QueueFlags,
        physical::PhysicalDeviceType,
    },
    image::ImageUsage,
    instance::{Instance, InstanceExtensions},
    pipeline::graphics::vertex_input::Vertex,
    swapchain::{Surface, Swapchain, SwapchainCreateInfo},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{self, Window, WindowAttributes, WindowId},
};



#[derive(BufferContents, Vertex)]
#[repr(C)]
struct MyVertex {
    #[format(R32G32B32_SFLOAT)]
    position: [f32; 3],
}

const VERTICES: [MyVertex; 3] = [
    MyVertex {
        position: [0.0, -0.5, 0.0],
    },
    MyVertex {
        position: [0.5, 0.5, 0.0],
    },
    MyVertex {
        position: [-0.5, 0.5, 0.0],
    },
];

struct GraphicsState {
    instance: Arc<Instance>,
    window: Arc<Window>,
    surface: Arc<Surface>,
    device: Arc<Device>,
    queue: Arc<Queue>,
}

impl GraphicsState {
    fn new(window: Arc<Window>, required_extensions: InstanceExtensions) -> Result<Self> {
        let vulkan_library = VulkanLibrary::new().context("Failed to load Vulkan library")?;
        let instance = Instance::new(
            vulkan_library,
            vulkano::instance::InstanceCreateInfo {
                enabled_extensions: required_extensions,
                ..Default::default()
            },
        )
        .context("Failed to create Vulkan Instance")?;

        let surface = Surface::from_window(instance.clone(), window.clone())
            .context("Failed to create Surface from window")?;

        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            khr_ray_tracing_pipeline: true,
            khr_acceleration_structure: true,
            khr_deferred_host_operations: true,
            khr_buffer_device_address: true,
            khr_spirv_1_4: true,
            khr_shader_float_controls: true,
            ..DeviceExtensions::empty()
        };

        let (physical_device, queue_family_index) = instance
            .enumerate_physical_devices()
            .context("Failed to enumerate physical devices")?
            .filter(|p| p.supported_extensions().contains(&device_extensions))
            .filter_map(|p| {
                p.queue_family_properties()
                    .iter()
                    .enumerate()
                    .position(|(i, q)| {
                        q.queue_flags
                            .intersects(QueueFlags::GRAPHICS | QueueFlags::COMPUTE)
                            && p.surface_support(i as u32, &surface).unwrap_or(false)
                    })
                    .map(|q| (p, q as u32))
            })
            .min_by_key(|(p, _)| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                _ => 4,
            })
            .context("No suitable device found")?;

        let (device, mut queues) = Device::new(
            physical_device.clone(),
            DeviceCreateInfo {
                queue_create_infos: vec![QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],
                enabled_extensions: device_extensions,
                ..Default::default()
            },
        )
        .context("Failed to create logical device")?;

        let queue = queues
            .next()
            .context("Failed to extract first queue out of queues")?;

        let (mut swapchain, swapchain_images) = {
            let caps = physical_device
                .surface_capabilities(&surface, Default::default())
                .context("Failed to get surface capabilities")?;

            let dimensions = window.inner_size();

            let composite_alpha = caps
                .supported_composite_alpha
                .into_iter()
                .next()
                .context("No supported composite alpha")?;
            let image_format = physical_device
                .surface_formats(&surface, Default::default())
                .context("Failed to get surface formats")?
                .get(0)
                .context("No surface formats found")?
                .0;

            Swapchain::new(
                device.clone(),
                surface.clone(),
                SwapchainCreateInfo {
                    min_image_count: caps.min_image_count,
                    image_format,
                    image_extent: dimensions.into(),
                    image_usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_DST,
                    composite_alpha,
                    ..Default::default()
                },
            )
            .context("Failed to create swapchain")?
        };

        let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));

        let storage_images = swapchain_images
            .iter()
            .map(|image| {
                ImageView::new_default(
                    Image::new(
                        memory_allocator.clone(),
                        ImageCreateInfo {
                            image_type: ImageType::Dim2d,
                            format: image.format(),
                            extent: image.extent(),
                            usage: ImageUsage::STORAGE | ImageUsage::TRANSFER_SRC,
                            ..Default::default()
                        },
                        AllocationCreateInfo {
                            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                            ..Default::default()
                        }
                    )
                    .context("Failed to create storage image")?
                )
                .context("Failed to create image view for storage image")
            })
            .collect::<Result<Vec<_>>>()
            .context("Failed to create storage images")?;


        Ok(Self {
            instance,
            window,
            surface,
            device,
            queue,
        })
    }
}

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    graphics_state: Option<GraphicsState>,
    error: Option<Error>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let result = (|| -> Result<()> {
            let window = Arc::new(
                event_loop
                    .create_window(Window::default_attributes())
                    .context("Failed to create window")?,
            );

            let required_extensions = Surface::required_extensions(event_loop)
                .context("Failed to get required extensions")?;

            self.graphics_state = Some(GraphicsState::new(window.clone(), required_extensions)?);

            self.window = Some(window);

            Ok(())
        })();

        if let Err(e) = result {
            self.error = Some(e);
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("Requested to close window");
                event_loop.exit();
            }

            WindowEvent::RedrawRequested => {
                // Static variables to track frame count and time
                static mut LAST_TIME: Option<Instant> = None;
                static mut FRAME_COUNT: u32 = 0;

                unsafe {
                    let now = Instant::now();
                    if let Some(last_time) = LAST_TIME {
                        FRAME_COUNT += 1;
                        let elapsed = now.duration_since(last_time);
                        if elapsed >= Duration::from_secs(1) {
                            let fps = FRAME_COUNT as f64 / elapsed.as_secs_f64();
                            println!("FPS: {:.2}", fps);
                            LAST_TIME = Some(now);
                            FRAME_COUNT = 0;
                        }
                    } else {
                        LAST_TIME = Some(now);
                        FRAME_COUNT = 0;
                    }
                }
                if let Some(_graphics_state) = self.graphics_state.as_ref() {
                    // Empty for now
                } else {
                    self.error = Some(anyhow::anyhow!("Graphics state not initialized"));
                    event_loop.exit();
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                } else {
                    self.error = Some(anyhow::anyhow!("Window is not yet created"));
                    event_loop.exit();
                }
            }

            _ => (),
        }
    }
}

fn main() {
    if let Err(e) = run() {
        let error_message = format!("{e:#}");
        eprintln!("Error: {error_message}");

        rfd::MessageDialog::new()
            .set_title("Fatal Error")
            .set_description(&error_message)
            .set_level(rfd::MessageLevel::Error)
            .show();

        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let event_loop = EventLoop::new().context("Failed to create event loop")?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    event_loop.run_app(&mut app).context("Event loop error")?;

    if let Some(err) = app.error {
        return Err(err);
    }

    Ok(())
}
