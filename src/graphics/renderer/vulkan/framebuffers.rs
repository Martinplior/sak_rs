use std::sync::Arc;

use vulkano::{
    image::{Image, view::ImageView},
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass},
};

pub(crate) fn create(
    swapchain_images: &[Arc<Image>],
    render_pass: &Arc<RenderPass>,
) -> Box<[Arc<Framebuffer>]> {
    swapchain_images
        .iter()
        .map(|image| {
            let view = ImageView::new_default(image.clone()).expect("Failed to create image view");
            Framebuffer::new(
                render_pass.clone(),
                FramebufferCreateInfo {
                    attachments: vec![view],
                    ..Default::default()
                },
            )
            .expect("Failed to create framebuffer")
        })
        .collect()
}
