use std::any::Any;
use std::sync::Arc;

use command_builder::CommandBuilder;
use parking_lot::Mutex;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use vulkano::command_buffer::CommandBufferExecFuture;
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::device::{Device, Queue};
use vulkano::image::Image;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::{Framebuffer, RenderPass};
use vulkano::swapchain::{
    PresentFuture, PresentMode, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo,
    SwapchainPresentInfo,
};
use vulkano::sync::GpuFuture;
use vulkano::sync::future::{FenceSignalFuture, JoinFuture};
use vulkano::{Validated, VulkanError};

use crate::sync::spsc::OnceReceiver;
use crate::thread::WorkerThread;

mod device;
mod framebuffers;
mod instance;
mod render_pass;
mod swapchain;

pub trait WindowLike: HasWindowHandle + HasDisplayHandle + Any + Send + Sync {}

impl<T: ?Sized> WindowLike for T where T: HasWindowHandle + HasDisplayHandle + Any + Send + Sync {}

type FenceFuture = FenceSignalFuture<
    PresentFuture<
        CommandBufferExecFuture<JoinFuture<Box<dyn GpuFuture + Send>, SwapchainAcquireFuture>>,
    >,
>;

pub mod command_builder;

struct Allocators {
    command_buffer: Arc<StandardCommandBufferAllocator>,
}

impl Allocators {
    fn new(device: Arc<Device>) -> Self {
        let command_buffer = Arc::new(StandardCommandBufferAllocator::new(
            device,
            Default::default(),
        ));
        Self { command_buffer }
    }
}

struct Shared {
    swapchain: Arc<Swapchain>,
    swapchain_images: Box<[Arc<Image>]>,
    allocators: Allocators,
    framebuffers: Box<[Arc<Framebuffer>]>,
    fences: Box<[Option<Arc<FenceFuture>>]>,
    prev_fence_index: usize,
}

impl Shared {
    fn resize(&mut self, render_pass: &Arc<RenderPass>, new_size: [u32; 2]) {
        self.recreate_swapchain(
            render_pass,
            SwapchainCreateInfo {
                image_extent: new_size,
                ..self.swapchain.create_info()
            },
        );
    }

    fn set_vsync(&mut self, render_pass: &Arc<RenderPass>, vsync: bool) {
        self.recreate_swapchain(
            render_pass,
            SwapchainCreateInfo {
                present_mode: if vsync {
                    PresentMode::Fifo
                } else {
                    PresentMode::Immediate
                },
                ..self.swapchain.create_info()
            },
        );
    }

    fn recreate_swapchain(
        &mut self,
        render_pass: &Arc<RenderPass>,
        create_info: SwapchainCreateInfo,
    ) {
        let (new_swapchain, new_swapchain_images) =
            swapchain::recreate(&self.swapchain, create_info);
        let new_framebuffers = framebuffers::create(&new_swapchain_images, render_pass);
        self.swapchain = new_swapchain;
        self.swapchain_images = new_swapchain_images;
        self.framebuffers = new_framebuffers;
    }

    fn render(
        &mut self,
        device: Arc<Device>,
        queue: Arc<Queue>,
        render_pass: &Arc<RenderPass>,
        clear_color: [f32; 4],
        mut add_commands: impl FnMut(&mut CommandBuilder),
    ) {
        let (image_index, suboptimal, acquire_future) =
            match vulkano::swapchain::acquire_next_image(self.swapchain.clone(), None)
                .map_err(Validated::unwrap)
            {
                Ok(r) => r,
                Err(VulkanError::OutOfDate) => {
                    self.resize(render_pass, self.swapchain.image_extent());
                    return;
                }
                Err(e) => panic!("failed to acquire next image: {e}"),
            };
        let image_index = image_index as usize;

        suboptimal.then(|| self.resize(render_pass, self.swapchain.image_extent()));

        unsafe {
            std::hint::assert_unchecked(image_index < self.fences.len());
            std::hint::assert_unchecked(self.prev_fence_index < self.fences.len());
        }

        let prev_future = match self
            .fences
            .get(self.prev_fence_index)
            .expect("unreachable")
            .clone()
        {
            None => {
                let mut now = vulkano::sync::now(device);
                now.cleanup_finished();
                now.boxed_send()
            }
            Some(fence) => fence.boxed_send(),
        };

        let mut command_builder = CommandBuilder::new(
            Viewport {
                extent: self.swapchain.image_extent().map(|x| x as f32),
                ..Default::default()
            },
            self.allocators.command_buffer.clone(),
            &queue,
            &self.framebuffers,
            image_index,
            clear_color,
        );
        add_commands(&mut command_builder);
        let command_buffer = command_builder.into_command_buffer();
        let future = prev_future
            .join(acquire_future)
            .then_execute(queue.clone(), command_buffer)
            .expect("failed to execute command buffer")
            .then_swapchain_present(
                queue,
                SwapchainPresentInfo::swapchain_image_index(
                    self.swapchain.clone(),
                    image_index as u32,
                ),
            )
            .then_signal_fence_and_flush()
            .map_err(Validated::unwrap);

        let new_fence = match future {
            Ok(fence) => Some(Arc::new(fence)),
            Err(VulkanError::OutOfDate) => {
                self.resize(render_pass, self.swapchain.image_extent());
                None
            }
            Err(e) => {
                eprintln!("failed to flush future: {}", e);
                None
            }
        };

        let fence = self.fences.get_mut(image_index).expect("unreachable");
        std::mem::replace(fence, new_fence)
            .map(|f| f.wait(None).expect("failed to wait for fence"));

        self.prev_fence_index = image_index;
    }
}

pub struct Renderer {
    device: Arc<Device>,
    queue: Arc<Queue>,
    render_pass: Arc<RenderPass>,

    shared: Arc<Mutex<Shared>>,
    render_worker: WorkerThread,
    render_receiver: Option<OnceReceiver<()>>,

    pub clear_color: [f32; 4],
}

impl Renderer {
    pub fn new(
        window: Arc<impl WindowLike>,
        window_inner_size: [u32; 2],
        desire_image_count: u32,
        event_loop: &impl HasDisplayHandle,
    ) -> Self {
        let instance = instance::from_event_loop(event_loop);
        let physical_device = instance
            .enumerate_physical_devices()
            .expect("No physical device available")
            .next()
            .expect("No physical device available");
        let (device, queue) = device::from_phisical_device(physical_device.clone());
        let (swapchain, swapchain_images) = swapchain::create(
            window,
            window_inner_size,
            instance.clone(),
            device.clone(),
            &physical_device,
            desire_image_count,
        );
        let allocators = Allocators::new(device.clone());
        let render_pass = render_pass::create(device.clone(), &swapchain);
        let framebuffers = framebuffers::create(&swapchain_images, &render_pass);
        let fences = (0..swapchain_images.len()).map(|_| None).collect();
        let shared = Arc::new(Mutex::new(Shared {
            swapchain,
            swapchain_images,
            allocators,
            framebuffers,
            fences,
            prev_fence_index: 0,
        }));
        let render_worker = WorkerThread::new();
        Self {
            device,
            queue,
            render_pass,
            shared,
            render_worker,
            render_receiver: None,
            clear_color: [0.0; 4],
        }
    }

    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    pub fn render_pass(&self) -> &Arc<RenderPass> {
        &self.render_pass
    }

    pub fn resize(&mut self, new_size: [u32; 2]) {
        let [width, height] = new_size;
        let new_size = [width.max(1), height.max(1)];
        self.shared.lock().resize(&self.render_pass, new_size);
    }

    pub fn set_vsync(&mut self, vsync: bool) {
        self.shared.lock().set_vsync(&self.render_pass, vsync);
    }

    pub fn render(&mut self, add_commands: impl FnMut(&mut CommandBuilder) + Send + 'static) {
        let shared = self.shared.clone();
        let device = self.device.clone();
        let queue = self.queue.clone();
        let render_pass = self.render_pass.clone();
        let clear_color = self.clear_color;
        let new_render_receiver = self.render_worker.add_task_sync(move || {
            shared
                .lock()
                .render(device, queue, &render_pass, clear_color, add_commands);
        });
        self.render_receiver
            .replace(new_render_receiver)
            .map(|r| r.recv());
    }
}
