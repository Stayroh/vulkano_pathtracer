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

mod srmiss {
    vulkano_shaders::shader! {
        ty: "miss",
        path: "src/shaders/srmiss.glsl",
        vulkan_version: "1.3",
    }
}


use anyhow::{Context, Error, Result};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use vulkano::acceleration_structure::{AccelerationStructure, AccelerationStructureBuildGeometryInfo, AccelerationStructureBuildRangeInfo, AccelerationStructureBuildType, AccelerationStructureCreateInfo, AccelerationStructureGeometries, AccelerationStructureGeometryInstancesData, AccelerationStructureGeometryInstancesDataType, AccelerationStructureGeometryTrianglesData, AccelerationStructureInstance, AccelerationStructureType, BuildAccelerationStructureFlags, BuildAccelerationStructureMode};
use vulkano::buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, BlitImageInfo, CommandBufferUsage, ImageBlit};
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
use vulkano::descriptor_set::allocator::{DescriptorSetAllocator, StandardDescriptorSetAllocator};
use vulkano::device::DeviceFeatures;
use vulkano::format::Format;
use vulkano::image::sampler::Filter;
use vulkano::image::{Image, ImageAspects, ImageCreateInfo, ImageLayout, ImageSubresourceLayers, ImageType};
use vulkano::image::view::ImageView;
use vulkano::swapchain::SwapchainPresentInfo;
use vulkano::{descriptor_set, memory, swapchain};
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryAllocatePreference, MemoryTypeFilter, StandardMemoryAllocator};
use vulkano::pipeline::layout::PipelineDescriptorSetLayoutCreateInfo;
use vulkano::pipeline::{Pipeline, PipelineBindPoint, PipelineLayout, PipelineShaderStageCreateInfo};
use vulkano::pipeline::ray_tracing::{RayTracingPipeline, RayTracingPipelineCreateInfo, RayTracingShaderGroupCreateInfo, ShaderBindingTable};
use vulkano::shader::spirv::ImageFormat;
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
    sync::{now, GpuFuture}
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

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Camera {
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
}

struct GpuMat4([[f32; 4]; 4]);

impl From<Mat4> for GpuMat4 {
    fn from(mat: Mat4) -> Self {
        GpuMat4(mat.to_cols_array_2d())
    }
}


struct GraphicsState {
    instance: Arc<Instance>,
    window: Arc<Window>,
    surface: Arc<Surface>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    swapchain: Arc<Swapchain>,
    swapchain_images: Vec<Arc<Image>>,
    storage_images: Vec<Arc<ImageView>>,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    memory_allocator: Arc<StandardMemoryAllocator>,
    camera: Camera,
    raytracing_pipeline: Arc<RayTracingPipeline>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    tlas: Arc<AccelerationStructure>,
    camera_buffer: Subbuffer<Camera>,
    shader_binding_table: Arc<ShaderBindingTable>,
}

impl GraphicsState {
    fn update(&mut self) -> Result<()> {
        let (image_index, _suboptimal, acquire_future) = swapchain::acquire_next_image(self.swapchain.clone(), None)
            .context("Failed to acquire next image from swapchain")?;

        let descriptor_set_layout = self.raytracing_pipeline.layout().set_layouts().get(0).context("No descriptor set layout found")?;

        let descriptor_set = DescriptorSet::new(
            self.descriptor_set_allocator.clone(),
            descriptor_set_layout.clone(),
            [
                WriteDescriptorSet::acceleration_structure(0, self.tlas.clone()),
                WriteDescriptorSet::image_view(1, self.storage_images[image_index as usize].clone()),
                WriteDescriptorSet::buffer(2, self.camera_buffer.clone())
            ],
            [],
        )
        .context("Failed to create descriptor set")?;

        let mut builder = AutoCommandBufferBuilder::primary(
            self.command_buffer_allocator.clone(),
            self.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .context("Failed to create command buffer builder")?;



        builder
            .bind_pipeline_ray_tracing(self.raytracing_pipeline.clone())
            .context("Failed to bind raytracing pipeline")?
            .bind_descriptor_sets(
                PipelineBindPoint::RayTracing,
                self.raytracing_pipeline.layout().clone(),
                0,
                descriptor_set,
            )
            .context("Failed to bind descriptor sets")?;

        unsafe {
            builder
                .trace_rays(self.shader_binding_table.addresses().clone(), self.swapchain_images[image_index as usize].extent())
                .context("Failed to record trace rays command")?;
        }

        let storage_image = self.storage_images[image_index as usize].image();
        let swapchain_image = &self.swapchain_images[image_index as usize];

        // Blit from storage image to swapchain image
        builder
            .blit_image(BlitImageInfo {
                src_image_layout: ImageLayout::General,
                dst_image_layout: ImageLayout::TransferDstOptimal,
                regions: [ImageBlit {
                    src_subresource: ImageSubresourceLayers {
                        aspects: ImageAspects::COLOR,
                        mip_level: 0,
                        array_layers: 0..1,
                    },
                    src_offsets: [
                        [0, 0, 0],
                        [storage_image.extent()[0] as u32, storage_image.extent()[1] as u32, 1],
                    ],
                    dst_subresource: ImageSubresourceLayers {
                        aspects: ImageAspects::COLOR,
                        mip_level: 0,
                        array_layers: 0..1,
                    },
                    dst_offsets: [
                        [0, 0, 0],
                        [swapchain_image.extent()[0] as u32, swapchain_image.extent()[1] as u32, 1],
                    ],
                    ..Default::default()
                }]
                .into(),
                filter: Filter::Linear,
                ..BlitImageInfo::images(storage_image.clone(), swapchain_image.clone())
            })
            .context("Failed to blit image")?;

        let command_buffer = builder.build().context("Failed to build command buffer")?;

        let future = now(self.device.clone())
            .join(acquire_future)
            .then_execute(self.queue.clone(), command_buffer)
            .context("Failed to execute command buffer")?
            .then_swapchain_present(
                self.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(self.swapchain.clone(), image_index)
            )
            .then_signal_fence_and_flush()
            .context("Failed to signal fence and flush")?;

        future.wait(None).context("Failed to wait for future")?;

        Ok(())
    }

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
            khr_ray_tracing_position_fetch: true,
            ..DeviceExtensions::empty()
        };

        let device_features = DeviceFeatures {
            ray_tracing_pipeline: true,
            acceleration_structure: true,
            buffer_device_address: true,
            ray_tracing_position_fetch: true,
            ..Default::default()
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
                enabled_features: device_features,
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
                            format: Format::R16G16B16A16_SFLOAT,
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

        let command_buffer_allocator = Arc::new(
            StandardCommandBufferAllocator::new(device.clone(), Default::default())
        );

        let raytracing_pipeline = {
            let raygen = rgen::load(device.clone())
                .context("Failed to load raygen shader module")?
                .entry_point("main")
                .context("Failed to set entry point")?;

            let closest_hit = rchit::load(device.clone())
                .context("Failed to load closest hit shader module")?
                .entry_point("main")
                .context("Failed to set entry point")?;

            let miss = rmiss::load(device.clone())
                .context("Failed to load miss shader module")?
                .entry_point("main")
                .context("Failed to set entry point")?;

            let shadow_miss = srmiss::load(device.clone())
                .context("Failed to load shadow miss shader module")?
                .entry_point("main")
                .context("Failed to set entry point")?;

            let stages = [
                PipelineShaderStageCreateInfo::new(raygen),
                PipelineShaderStageCreateInfo::new(miss),
                PipelineShaderStageCreateInfo::new(closest_hit),
                PipelineShaderStageCreateInfo::new(shadow_miss),
            ];

            let groups = [
                RayTracingShaderGroupCreateInfo::General { general_shader: 0 },
                RayTracingShaderGroupCreateInfo::General { general_shader: 1 },
                RayTracingShaderGroupCreateInfo::TrianglesHit { closest_hit_shader: Some(2), any_hit_shader: None },
                RayTracingShaderGroupCreateInfo::General { general_shader: 3 }
            ];

            let layout = PipelineLayout::new(
                device.clone(),
                PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
                    .into_pipeline_layout_create_info(device.clone())
                    .context("Failed to create pipeline layout")?
            )
            .context("Failed to create pipeline layout")?;


            RayTracingPipeline::new(
                device.clone(),
                None,
                RayTracingPipelineCreateInfo {
                    stages: stages.into_iter().collect(),
                    groups: groups.into_iter().collect(),
                    max_pipeline_ray_recursion_depth: 4,
                    ..RayTracingPipelineCreateInfo::layout(layout)
                }
            )
            .context("Failed to create raytracing pipeline")?
        };

        let mut vertices = [
            MyVertex { position: [0.0, 0.5, 0.0] },
            MyVertex { position: [-0.5, -0.25, 0.0] },
            MyVertex { position: [0.25, -0.1, 0.0] },
            MyVertex { position: [-0.5, -0.0, 0.0] },
            MyVertex { position: [-0.1, -0.2, 0.7] },
            MyVertex { position: [0.55, -0.1, -0.4] },
        ];

        let offset = Vec3::new(0.0, 0.0, -2.0);

        for vertex in &mut vertices {
            let pos = Vec3::from(vertex.position) + offset;
            vertex.position = pos.to_array();
        }


        let vertex_buffer = Buffer::from_iter(
            memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER | BufferUsage::SHADER_DEVICE_ADDRESS | BufferUsage::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            vertices,
        )
        .context("Failed to create vertex buffer")?;

        let blas = unsafe {
            build_acceleration_structure_triangles(
                &vertex_buffer,
                memory_allocator.clone(),
                &command_buffer_allocator,
                device.clone(),
                queue.clone(),
            )
        };

        let tlas = unsafe {
            build_top_level_acceleration_structure(
                vec![AccelerationStructureInstance {
                    acceleration_structure_reference: blas.device_address().into(),
                    ..Default::default()
                }],
                memory_allocator.clone(),
                &command_buffer_allocator,
                device.clone(),
                queue.clone(),
            )
        };

        let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_2, 4.0 / 3.0, 0.01, 100.0);
        let view = Mat4::look_at_rh(
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
        );

        let camera = Camera {
            proj: GpuMat4::from(proj).0,
            view: GpuMat4::from(view).0,
        };

        let camera_buffer = Buffer::from_data(
            memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            camera,
        )
        .context("Failed to create camera buffer")?;

        let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(device.clone(), Default::default()));

        let shader_binding_table = Arc::new(ShaderBindingTable::new(memory_allocator.clone(), &raytracing_pipeline)
            .context("Failed to create shader binding table")?);

        Ok(Self {
            instance,
            window,
            surface,
            device,
            queue,
            swapchain,
            swapchain_images,
            storage_images,
            command_buffer_allocator,
            memory_allocator,
            camera,
            raytracing_pipeline,
            descriptor_set_allocator,
            tlas,
            camera_buffer,
            shader_binding_table,
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
                if let Some(graphics_state) = self.graphics_state.as_mut() {
                    if let Err(e) = graphics_state.update() {
                        self.error = Some(e);
                        event_loop.exit();
                    }
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




unsafe fn build_acceleration_structure_common(
    geometries: AccelerationStructureGeometries,
    primitive_count: u32,
    ty: AccelerationStructureType,
    memory_allocator: Arc<StandardMemoryAllocator>,
    command_buffer_allocator: &Arc<StandardCommandBufferAllocator>,
    device: Arc<Device>,
    queue: Arc<Queue>,
) -> Arc<AccelerationStructure> {
    let mut as_build_geometry_info = AccelerationStructureBuildGeometryInfo {
        mode: BuildAccelerationStructureMode::Build,
        flags: BuildAccelerationStructureFlags::PREFER_FAST_TRACE,
        ..AccelerationStructureBuildGeometryInfo::new(geometries)
    };

    let as_build_sizes_info = device
        .acceleration_structure_build_sizes(
            AccelerationStructureBuildType::Device,
            &as_build_geometry_info,
            &[primitive_count],
        )
        .unwrap();

    // We create a new scratch buffer for each acceleration structure for simplicity. You may want
    // to reuse scratch buffers if you need to build many acceleration structures.
    let scratch_buffer = Buffer::new_slice::<u8>(
        memory_allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::SHADER_DEVICE_ADDRESS | BufferUsage::STORAGE_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo::default(),
        as_build_sizes_info.build_scratch_size,
    )
    .unwrap();

    let acceleration = unsafe {
        AccelerationStructure::new(
            device.clone(),
            AccelerationStructureCreateInfo {
                ty,
                ..AccelerationStructureCreateInfo::new(
                    Buffer::new_slice::<u8>(
                        memory_allocator,
                        BufferCreateInfo {
                            usage: BufferUsage::ACCELERATION_STRUCTURE_STORAGE
                                | BufferUsage::SHADER_DEVICE_ADDRESS,
                            ..Default::default()
                        },
                        AllocationCreateInfo::default(),
                        as_build_sizes_info.acceleration_structure_size,
                    )
                    .unwrap(),
                )
            },
        )
    }
    .unwrap();

    as_build_geometry_info.dst_acceleration_structure = Some(acceleration.clone());
    as_build_geometry_info.scratch_data = Some(scratch_buffer);

    let as_build_range_info = AccelerationStructureBuildRangeInfo {
        primitive_count,
        ..Default::default()
    };

    // For simplicity, we build a single command buffer that builds the acceleration structure,
    // then waits for its execution to complete.
    let mut builder = AutoCommandBufferBuilder::primary(
        command_buffer_allocator.clone(),
        queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    unsafe {
    builder
        .build_acceleration_structure(
            as_build_geometry_info,
            vec![as_build_range_info].into(),
        )
        .unwrap();
    }

    let command_buffer = builder.build().unwrap();

    let build_future = now(device.clone())
        .then_execute(queue.clone(), command_buffer)
        .unwrap()
        .then_signal_fence_and_flush()
        .unwrap();
    
    build_future.wait(None).unwrap();


    acceleration
}

unsafe fn build_acceleration_structure_triangles(
    vertex_buffer: &Subbuffer<[MyVertex]>,
    memory_allocator: Arc<StandardMemoryAllocator>,
    command_buffer_allocator: &Arc<StandardCommandBufferAllocator>,
    device: Arc<Device>,
    queue: Arc<Queue>,
) -> Arc<AccelerationStructure> {
    let primitive_count = (vertex_buffer.len() / 3) as u32;
    let as_geometry_triangles_data = AccelerationStructureGeometryTrianglesData {
        max_vertex: vertex_buffer.len() as _,
        vertex_data: Some(vertex_buffer.clone().into_bytes()),
        vertex_stride: size_of::<MyVertex>() as _,
        ..AccelerationStructureGeometryTrianglesData::new(Format::R32G32B32_SFLOAT)
    };

    let geometries = AccelerationStructureGeometries::Triangles(vec![as_geometry_triangles_data]);

    unsafe { build_acceleration_structure_common(
        geometries,
        primitive_count,
        AccelerationStructureType::BottomLevel,
        memory_allocator,
        command_buffer_allocator,
        device,
        queue,
    )}
}

unsafe fn build_top_level_acceleration_structure(
    as_instances: Vec<AccelerationStructureInstance>,
    memory_allocator: Arc<StandardMemoryAllocator>,
    command_buffer_allocator: &Arc<StandardCommandBufferAllocator>,
    device: Arc<Device>,
    queue: Arc<Queue>,
) -> Arc<AccelerationStructure> {
    let primitive_count = as_instances.len() as u32;

    let instance_buffer = Buffer::from_iter(
        memory_allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::SHADER_DEVICE_ADDRESS
                | BufferUsage::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ..Default::default()
        },
        as_instances,
    )
    .unwrap();

    let as_geometry_instances_data = AccelerationStructureGeometryInstancesData::new(
        AccelerationStructureGeometryInstancesDataType::Values(Some(instance_buffer)),
    );

    let geometries = AccelerationStructureGeometries::Instances(as_geometry_instances_data);

    unsafe {build_acceleration_structure_common(
        geometries,
        primitive_count,
        AccelerationStructureType::TopLevel,
        memory_allocator,
        command_buffer_allocator,
        device,
        queue,
    )}
}
