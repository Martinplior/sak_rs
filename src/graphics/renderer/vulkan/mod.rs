pub mod mipmap;

mod device;
mod framebuffers;
mod instance;
mod render_pass;
mod swapchain;

use std::any::Any;
use std::num::NonZero;
use std::sync::Arc;

use command_builder::CommandBuilder;
use parking_lot::Mutex;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use vulkano::command_buffer::CommandBufferExecFuture;
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::{Device, DeviceExtensions, DeviceFeatures, Queue};
use vulkano::format::Format;
use vulkano::image::Image;
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::pipeline::graphics::color_blend::{AttachmentBlend, BlendFactor, BlendOp};
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

pub trait WindowLike: HasWindowHandle + HasDisplayHandle + Any + Send + Sync {}

impl<T: ?Sized> WindowLike for T where T: HasWindowHandle + HasDisplayHandle + Any + Send + Sync {}

type FenceFuture = FenceSignalFuture<
    PresentFuture<
        CommandBufferExecFuture<JoinFuture<Box<dyn GpuFuture + Send>, SwapchainAcquireFuture>>,
    >,
>;

pub mod command_builder;

pub const PREMUL_ALPHA: AttachmentBlend = AttachmentBlend {
    color_blend_op: BlendOp::Add,
    src_color_blend_factor: BlendFactor::One,
    dst_color_blend_factor: BlendFactor::OneMinusSrcAlpha,
    alpha_blend_op: BlendOp::Add,
    src_alpha_blend_factor: BlendFactor::One,
    dst_alpha_blend_factor: BlendFactor::OneMinusSrcAlpha,
};

#[derive(Debug, Clone)]
pub struct Allocators {
    command_buffer: Arc<StandardCommandBufferAllocator>,
    descriptor_set: Arc<StandardDescriptorSetAllocator>,
    memory: Arc<StandardMemoryAllocator>,
}

impl Allocators {
    fn new(device: Arc<Device>) -> Self {
        let command_buffer = Arc::new(StandardCommandBufferAllocator::new(
            device.clone(),
            Default::default(),
        ));
        let descriptor_set = Arc::new(StandardDescriptorSetAllocator::new(
            device.clone(),
            Default::default(),
        ));
        let memory = Arc::new(StandardMemoryAllocator::new_default(device));
        Self {
            command_buffer,
            descriptor_set,
            memory,
        }
    }

    pub fn command_buffer(&self) -> &Arc<StandardCommandBufferAllocator> {
        &self.command_buffer
    }

    pub fn descriptor_set(&self) -> &Arc<StandardDescriptorSetAllocator> {
        &self.descriptor_set
    }

    pub fn memory(&self) -> &Arc<StandardMemoryAllocator> {
        &self.memory
    }
}

struct Shared {
    window_inner_size: Box<dyn Fn() -> [u32; 2] + Send + Sync>,
    queue: Arc<Queue>,
    render_pass: Arc<RenderPass>,
    allocators: Allocators,
    need_recreate_swapchain: Mutex<Option<RecreateSwapchain>>,
}

#[derive(Default)]
struct RecreateSwapchain {
    new_size: Option<[u32; 2]>,
    new_vsync: Option<bool>,
}

impl RecreateSwapchain {
    fn with_new_size(mut self, new_size: [u32; 2]) -> Self {
        self.new_size = Some(new_size);
        self
    }

    fn with_new_vsync(mut self, new_vsync: bool) -> Self {
        self.new_vsync = Some(new_vsync);
        self
    }
}

impl Shared {
    #[inline(always)]
    fn device(&self) -> &Arc<Device> {
        self.queue.device()
    }
}

struct SharedMut {
    swapchain: Arc<Swapchain>,
    swapchain_images: Box<[Arc<Image>]>,
    framebuffers: Box<[Arc<Framebuffer>]>,
    fences: Box<[Option<Arc<FenceFuture>>]>,
    prev_fence_index: usize,
    need_resize: bool,
}

impl SharedMut {
    fn render(
        &mut self,
        shared: &Shared,
        clear_color: [f32; 4],
        add_commands: impl FnOnce(&mut CommandBuilder),
    ) {
        let device = shared.device();
        let queue = &shared.queue;
        let render_pass = &shared.render_pass;
        let allocators = &shared.allocators;

        self.recreate_swapchain_when_need(shared, render_pass);

        let (image_index, suboptimal, acquire_future) =
            match vulkano::swapchain::acquire_next_image(self.swapchain.clone(), None)
                .map_err(Validated::unwrap)
            {
                Ok(r) => r,
                Err(VulkanError::OutOfDate) => {
                    self.need_resize = true;
                    return;
                }
                Err(e) => panic!("failed to acquire next image: {e}"),
            };
        let image_index = image_index as usize;

        suboptimal.then(|| self.need_resize = true);

        let prev_future = match self
            .fences
            .get(self.prev_fence_index)
            .expect("unreachable")
            .clone()
        {
            None => {
                let mut now = vulkano::sync::now(device.clone());
                now.cleanup_finished();
                now.boxed_send()
            }
            Some(fence) => fence.boxed_send(),
        };

        let mut command_builder = CommandBuilder::new(
            allocators.command_buffer.clone(),
            &self.swapchain,
            queue,
            self.framebuffers
                .get(image_index)
                .expect("unreachable")
                .clone(),
            clear_color,
        );
        add_commands(&mut command_builder);
        let command_buffer = command_builder.into_command_buffer();
        let future = prev_future
            .join(acquire_future)
            .then_execute(queue.clone(), command_buffer)
            .expect("failed to execute command buffer")
            .then_swapchain_present(
                queue.clone(),
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
                self.need_resize = true;
                None
            }
            Err(e) => {
                eprintln!("failed to flush future: {e}");
                None
            }
        };

        let fence = self.fences.get_mut(image_index).expect("unreachable");
        core::mem::replace(fence, new_fence)
            .map(|f| f.wait(None).expect("failed to wait for fence"));

        self.prev_fence_index = image_index;
    }

    fn recreate_swapchain_when_need(&mut self, shared: &Shared, render_pass: &Arc<RenderPass>) {
        let need_recreate_swapchain = shared.need_recreate_swapchain.lock().take();
        let need_resize = core::mem::take(&mut self.need_resize);
        let need_recreate_swapchain = match (need_recreate_swapchain, need_resize) {
            (Some(n), true) => Some(n.with_new_size((*shared.window_inner_size)())),
            (None, true) => {
                Some(RecreateSwapchain::default().with_new_size((*shared.window_inner_size)()))
            }
            (n, false) => n,
        };
        need_recreate_swapchain.map(|r| {
            let image_extent = r.new_size.unwrap_or_else(|| self.swapchain.image_extent());
            let present_mode = if let Some(v) = r.new_vsync {
                if v {
                    PresentMode::Fifo
                } else {
                    PresentMode::Immediate
                }
            } else {
                self.swapchain.present_mode()
            };
            let create_info = SwapchainCreateInfo {
                image_extent,
                present_mode,
                ..self.swapchain.create_info()
            };
            self.recreate_swapchain(render_pass, create_info);
        });
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
}

pub struct RendererCreateInfo<Window, WindowInnerSize>
where
    Window: WindowLike,
    WindowInnerSize: Fn() -> [u32; 2] + Send + Sync + 'static,
{
    pub window: Arc<Window>,
    pub window_inner_size: WindowInnerSize,
    pub desire_image_format: Option<Format>,
    pub desire_image_count: Option<NonZero<u32>>,
    pub device_extensions: DeviceExtensions,
    pub device_features: DeviceFeatures,
}

pub struct Renderer {
    shared: Arc<(Shared, Mutex<SharedMut>)>,
    render_worker: WorkerThread,
    render_receiver: Option<OnceReceiver<()>>,

    pub clear_color: [f32; 4],
}

impl Renderer {
    pub fn new<A, B>(create_info: RendererCreateInfo<A, B>) -> Self
    where
        A: WindowLike,
        B: Fn() -> [u32; 2] + Send + Sync + 'static,
    {
        let RendererCreateInfo {
            window,
            window_inner_size,
            desire_image_format,
            desire_image_count,
            device_extensions,
            device_features,
        } = create_info;
        let instance = instance::from_event_loop(&window);
        let physical_device = instance
            .enumerate_physical_devices()
            .expect("No physical device available")
            .next()
            .expect("No physical device available");
        let queue = device::from_phisical_device(
            physical_device.clone(),
            device_extensions,
            device_features,
        );
        let device = queue.device();
        let (swapchain, swapchain_images) = swapchain::create(
            window,
            window_inner_size(),
            instance.clone(),
            device.clone(),
            &physical_device,
            desire_image_format,
            desire_image_count,
        );
        let allocators = Allocators::new(device.clone());
        let render_pass = render_pass::create(device.clone(), &swapchain);
        let framebuffers = framebuffers::create(&swapchain_images, &render_pass);
        let fences = (0..swapchain_images.len()).map(|_| None).collect();
        let shared = {
            let shared = Shared {
                window_inner_size: Box::new(window_inner_size),
                queue,
                render_pass,
                allocators,
                need_recreate_swapchain: Mutex::new(None),
            };
            let shared_mut = Mutex::new(SharedMut {
                swapchain,
                swapchain_images,
                framebuffers,
                fences,
                prev_fence_index: 0,
                need_resize: false,
            });
            Arc::new((shared, shared_mut))
        };
        let render_worker = WorkerThread::new();
        Self {
            shared,
            render_worker,
            render_receiver: None,
            clear_color: [0.0; 4],
        }
    }

    pub fn device(&self) -> &Arc<Device> {
        self.shared.0.device()
    }

    pub fn queue(&self) -> &Arc<Queue> {
        &self.shared.0.queue
    }

    pub fn render_pass(&self) -> &Arc<RenderPass> {
        &self.shared.0.render_pass
    }

    pub fn allocators(&self) -> &Allocators {
        &self.shared.0.allocators
    }

    pub fn set_vsync(&mut self, vsync: bool) {
        let mut guard = self.shared.0.need_recreate_swapchain.lock();
        let n = if let Some(n) = guard.take() {
            n.with_new_vsync(vsync)
        } else {
            RecreateSwapchain::default().with_new_vsync(vsync)
        };
        *guard = Some(n);
    }

    pub fn render(&mut self, add_commands: impl FnOnce(&mut CommandBuilder) + Send + 'static) {
        let shared = self.shared.clone();
        let clear_color = self.clear_color;
        let new_render_receiver = self.render_worker.add_task_sync(move || {
            let (shared, shared_mut) = &*shared;
            shared_mut.lock().render(shared, clear_color, add_commands);
        });
        self.render_receiver
            .replace(new_render_receiver)
            .map(|r| r.recv());
    }
}
