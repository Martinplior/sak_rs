use std::sync::Arc;

use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer,
        RenderPassBeginInfo, allocator::CommandBufferAllocator,
    },
    device::Queue,
    pipeline::graphics::viewport::Viewport,
    render_pass::Framebuffer,
    swapchain::Swapchain,
};

pub struct CommandBuilder {
    pub builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    default_viewport: Viewport,
}

impl CommandBuilder {
    #[inline]
    pub fn default_viewport(&self) -> &Viewport {
        &self.default_viewport
    }
}

impl CommandBuilder {
    pub(crate) fn new(
        allocator: Arc<impl CommandBufferAllocator>,
        swapchain: &Swapchain,
        queue: &Queue,
        framebuffer: Arc<Framebuffer>,
        clear_color: [f32; 4],
    ) -> Self {
        let default_viewport = Viewport {
            extent: swapchain.image_extent().map(|x| x as f32),
            ..Default::default()
        };
        let mut builder = AutoCommandBufferBuilder::primary(
            allocator,
            queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .expect("Failed to create command buffer builder");
        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![Some(clear_color.into())],
                    ..RenderPassBeginInfo::framebuffer(framebuffer)
                },
                Default::default(),
            )
            .expect("Failed to begin render pass");
        Self {
            default_viewport,
            builder,
        }
    }

    pub(crate) fn into_command_buffer(mut self) -> Arc<PrimaryAutoCommandBuffer> {
        self.builder
            .end_render_pass(Default::default())
            .expect("Failed to end render pass");
        self.builder
            .build()
            .expect("Failed to build command buffer")
    }
}
