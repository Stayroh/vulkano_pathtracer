use anyhow::{Context, Error, Result};
use vulkano::{VulkanLibrary, instance::{Instance, InstanceExtensions}, swapchain::Surface};
use std::default;
use std::sync::Arc;
use std::time::{Instant, Duration};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{self, Window, WindowAttributes, WindowId},
};

struct GraphicsState {
    instance: Arc<Instance>,
}

impl GraphicsState {
    fn new(window: Arc<Window>, required_extensions: InstanceExtensions) -> Result<Self> {
        let vulkan_library = VulkanLibrary::new()
            .context("Failed to load Vulkan library")?;
        let instance = Instance::new(
            vulkan_library,
            vulkano::instance::InstanceCreateInfo {
                enabled_extensions: required_extensions,
                ..Default::default()
            },

        )
        .context("Failed to create Vulkan Instance")?;

        Ok(Self {
            instance,
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
        let result = ( || -> Result<()> {
            let window = Arc::new(
                event_loop
                    .create_window(Window::default_attributes())
                    .context("Failed to create window")?
            );

            let required_extensions = Surface::required_extensions(event_loop)
                .context("Failed to get required extensions")?;

            self.graphics_state =
                Some(GraphicsState::new(window.clone(), required_extensions)?);

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
                        if true {//elapsed >= Duration::from_secs(1) {
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