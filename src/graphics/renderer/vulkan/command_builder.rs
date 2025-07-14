use std::sync::Arc;

use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer,
        RenderPassBeginInfo, allocator::CommandBufferAllocator,
    },
    device::Queue,
    pipeline::graphics::viewport::Viewport,
    render_pass::Framebuffer,
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
        default_viewport: Viewport,
        allocator: Arc<impl CommandBufferAllocator>,
        queue: &Queue,
        framebuffers: &[Arc<Framebuffer>],
        image_index: usize,
        clear_color: [f32; 4],
    ) -> Self {
        let framebuffer = unsafe { framebuffers.get(image_index).unwrap_unchecked() };
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
                    ..RenderPassBeginInfo::framebuffer(framebuffer.clone())
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
