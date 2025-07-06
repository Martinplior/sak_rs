use std::{ops::Range, sync::Arc};

use vulkano::{
    ValidationError,
    buffer::BufferContents,
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer,
        RenderPassBeginInfo, allocator::CommandBufferAllocator,
    },
    device::Queue,
    pipeline::{ComputePipeline, GraphicsPipeline, PipelineLayout, graphics::viewport::Viewport},
    render_pass::Framebuffer,
};

pub struct CommandBuilder {
    builder: AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    default_viewport: Viewport,
}

impl CommandBuilder {
    /// `None` means using the default viewport
    pub fn set_viewport(
        &mut self,
        viewport: Option<Viewport>,
    ) -> Result<&mut Self, Box<ValidationError>> {
        let viewport = viewport.unwrap_or_else(|| self.default_viewport.clone());
        let r = self
            .builder
            .set_viewport(0, [viewport].into_iter().collect());
        match r {
            Ok(_) => Ok(self),
            Err(e) => Err(e),
        }
    }

    pub fn bind_pipeline_graphics(
        &mut self,
        pipeline: Arc<GraphicsPipeline>,
    ) -> Result<&mut Self, Box<ValidationError>> {
        let r = self.builder.bind_pipeline_graphics(pipeline);
        match r {
            Ok(_) => Ok(self),
            Err(e) => Err(e),
        }
    }

    pub fn bind_pipeline_compute(
        &mut self,
        pipeline: Arc<ComputePipeline>,
    ) -> Result<&mut Self, Box<ValidationError>> {
        let r = self.builder.bind_pipeline_compute(pipeline);
        match r {
            Ok(_) => Ok(self),
            Err(e) => Err(e),
        }
    }

    pub fn push_constants<Pc: BufferContents>(
        &mut self,
        pipeline_layout: Arc<PipelineLayout>,
        offset: u32,
        push_constants: Pc,
    ) -> Result<&mut Self, Box<ValidationError>> {
        let r = self
            .builder
            .push_constants(pipeline_layout, offset, push_constants);
        match r {
            Ok(_) => Ok(self),
            Err(e) => Err(e),
        }
    }

    pub fn draw(
        &mut self,
        vertex_range: Range<u32>,
        instance_range: Range<u32>,
    ) -> Result<&mut Self, Box<ValidationError>> {
        let first_vertex = vertex_range.start;
        let first_instance = instance_range.start;
        let vertex_count = vertex_range.end - vertex_range.start;
        let instance_count = instance_range.end - instance_range.start;
        let r = unsafe {
            self.builder
                .draw(vertex_count, instance_count, first_vertex, first_instance)
        };
        match r {
            Ok(_) => Ok(self),
            Err(e) => Err(e),
        }
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
