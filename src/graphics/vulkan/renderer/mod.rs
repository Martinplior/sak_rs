use std::{any::Any, num::NonZero, sync::Arc};

use crossbeam_queue::SegQueue;
use parking_lot::Mutex;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use vulkano::{
    Validated, VulkanError,
    command_buffer::{CommandBufferExecFuture, allocator::StandardCommandBufferAllocator},
    descriptor_set::allocator::StandardDescriptorSetAllocator,
    device::{Device, Queue},
    format::Format,
    image::Image,
    memory::allocator::StandardMemoryAllocator,
    pipeline::graphics::color_blend::{AttachmentBlend, BlendFactor, BlendOp},
    render_pass::{Framebuffer, RenderPass},
    swapchain::{
        PresentFuture, PresentMode, Swapchain, SwapchainAcquireFuture, SwapchainCreateInfo,
        SwapchainPresentInfo,
    },
    sync::{
        GpuFuture,
        future::{FenceSignalFuture, JoinFuture},
    },
};

use crate::{
    graphics::vulkan::{context::Context, renderer::command_builder::CommandBuilder},
    sync::spsc::OnceReceiver,
    thread::WorkerThread,
};

mod framebuffers;
mod render_pass;
mod swapchain;

pub mod command_builder;
pub mod mipmap;

pub const PREMUL_ALPHA: AttachmentBlend = AttachmentBlend {
    color_blend_op: BlendOp::Add,
    src_color_blend_factor: BlendFactor::One,
    dst_color_blend_factor: BlendFactor::OneMinusSrcAlpha,
    alpha_blend_op: BlendOp::Add,
    src_alpha_blend_factor: BlendFactor::One,
    dst_alpha_blend_factor: BlendFactor::OneMinusSrcAlpha,
};

pub trait WindowLike: HasWindowHandle + HasDisplayHandle + Any + Send + Sync {}

impl<T: ?Sized> WindowLike for T where T: HasWindowHandle + HasDisplayHandle + Any + Send + Sync {}

type FenceFuture = FenceSignalFuture<
    PresentFuture<
        CommandBufferExecFuture<JoinFuture<Box<dyn GpuFuture + Send>, SwapchainAcquireFuture>>,
    >,
>;

#[derive(Debug, Clone)]
pub struct Allocators {
    memory: Arc<StandardMemoryAllocator>,
    command_buffer: Arc<StandardCommandBufferAllocator>,
    descriptor_set: Arc<StandardDescriptorSetAllocator>,
}

impl Allocators {
    fn new(context: &Context) -> Self {
        let allocators = context.allocators();
        Self {
            memory: allocators.memory().clone(),
            command_buffer: allocators.command_buffer().clone(),
            descriptor_set: allocators.descriptor_set().clone(),
        }
    }

    #[inline]
    pub fn command_buffer(&self) -> &Arc<StandardCommandBufferAllocator> {
        &self.command_buffer
    }

    #[inline]
    pub fn descriptor_set(&self) -> &Arc<StandardDescriptorSetAllocator> {
        &self.descriptor_set
    }

    #[inline]
    pub fn memory(&self) -> &Arc<StandardMemoryAllocator> {
        &self.memory
    }
}

enum RecreateSwapchainType {
    Size([u32; 2]),
    Vsync(bool),
}

struct Shared {
    window_inner_size: Box<dyn Fn() -> [u32; 2] + Send + Sync>,
    queue: Arc<Queue>,
    render_pass: Arc<RenderPass>,
    allocators: Allocators,
    recreate_swapchain_queue: SegQueue<RecreateSwapchainType>,
}

impl Shared {
    #[inline(always)]
    fn device(&self) -> &Arc<Device> {
        self.queue.device()
    }

    fn resize_outdated(&self) {
        let new_size = (self.window_inner_size)();
        self.recreate_swapchain_queue
            .push(RecreateSwapchainType::Size(new_size));
    }
}

struct SharedMut {
    swapchain: Arc<Swapchain>,
    swapchain_images: Box<[Arc<Image>]>,
    framebuffers: Box<[Arc<Framebuffer>]>,
    fences: Box<[Option<Arc<FenceFuture>>]>,
    prev_fence_index: usize,
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
                    shared.resize_outdated();
                    return;
                }
                Err(e) => panic!("failed to acquire next image: {e}"),
            };
        let image_index = image_index as usize;

        suboptimal.then(|| shared.resize_outdated());

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
                shared.resize_outdated();
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
        let mut new_size = None;
        let mut new_vsync = None;
        while let Some(r) = shared.recreate_swapchain_queue.pop() {
            match r {
                RecreateSwapchainType::Size(size) => new_size = Some(size),
                RecreateSwapchainType::Vsync(vsync) => new_vsync = Some(vsync),
            }
        }
        let need_recreate_swapchain = new_size.is_some() || new_vsync.is_some();
        need_recreate_swapchain.then(|| {
            let image_extent = new_size.unwrap_or_else(|| self.swapchain.image_extent());
            let present_mode = if let Some(v) = new_vsync {
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

pub struct RendererCreateInfo<'context, Window, WindowInnerSizeFn>
where
    Window: WindowLike,
    WindowInnerSizeFn: Fn() -> [u32; 2] + Send + Sync + 'static,
{
    pub context: &'context Context,
    pub window: Arc<Window>,
    pub window_inner_size: WindowInnerSizeFn,
    pub desire_image_format: Option<Format>,
    pub desire_image_count: Option<NonZero<u32>>,
}

pub struct Renderer {
    pub clear_color: [f32; 4],

    shared: Arc<(Shared, Mutex<SharedMut>)>,
    render_worker: WorkerThread,
    render_receiver: Option<OnceReceiver<()>>,
}

impl Renderer {
    pub fn new<Window, WindowInnerSizeFn>(
        create_info: RendererCreateInfo<Window, WindowInnerSizeFn>,
    ) -> Self
    where
        Window: WindowLike,
        WindowInnerSizeFn: Fn() -> [u32; 2] + Send + Sync + 'static,
    {
        let RendererCreateInfo {
            context,
            window,
            window_inner_size,
            desire_image_format,
            desire_image_count,
        } = create_info;
        let instance = context.instance();
        let physical_device = instance
            .enumerate_physical_devices()
            .expect("No physical device available")
            .next()
            .expect("No physical device available");
        let queue = context.render_queue();
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
        let allocators = Allocators::new(context);
        let render_pass = render_pass::create(device.clone(), &swapchain);
        let framebuffers = framebuffers::create(&swapchain_images, &render_pass);
        let fences = (0..swapchain_images.len()).map(|_| None).collect();
        let shared = {
            let shared = Shared {
                window_inner_size: Box::new(window_inner_size),
                queue: queue.clone(),
                render_pass,
                allocators,
                recreate_swapchain_queue: SegQueue::new(),
            };
            let shared_mut = Mutex::new(SharedMut {
                swapchain,
                swapchain_images,
                framebuffers,
                fences,
                prev_fence_index: 0,
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

    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        self.shared.0.device()
    }

    #[inline]
    pub fn queue(&self) -> &Arc<Queue> {
        &self.shared.0.queue
    }

    #[inline]
    pub fn render_pass(&self) -> &Arc<RenderPass> {
        &self.shared.0.render_pass
    }

    #[inline]
    pub fn allocators(&self) -> &Allocators {
        &self.shared.0.allocators
    }

    pub fn set_vsync(&mut self, vsync: bool) {
        self.shared
            .0
            .recreate_swapchain_queue
            .push(RecreateSwapchainType::Vsync(vsync))
    }

    /// blocking until render is finished.
    ///
    /// if you don't want to block, use [`try_render`](Self::try_render) instead.
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

    /// never blocks, returns true if render is successful.
    ///
    /// if you want to block, use [`render`](Self::render) instead.
    pub fn try_render(
        &mut self,
        add_commands: impl FnOnce(&mut CommandBuilder) + Send + 'static,
    ) -> bool {
        if let Some(r) = self.render_receiver.take()
            && let Err(r) = r.try_recv()
        {
            self.render_receiver = Some(r);
            return false;
        }
        // self.render_receiver is None here, so self.render never blocks
        self.render(add_commands);
        true
    }
}
