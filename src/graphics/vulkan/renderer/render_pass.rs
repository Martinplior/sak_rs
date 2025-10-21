use std::sync::Arc;

use vulkano::{device::Device, render_pass::RenderPass, swapchain::Swapchain};

pub(crate) fn create(device: Arc<Device>, swapchain: &Swapchain) -> Arc<RenderPass> {
    vulkano::single_pass_renderpass!(
        device,
        attachments: {
            color: {
                format: swapchain.image_format(),
                samples: 1,
                load_op: Clear,
                store_op: Store,
            },
        },
        pass: {
            color: [color],
            depth_stencil: {},
        }
    )
    .expect("Failed to create render pass")
}
