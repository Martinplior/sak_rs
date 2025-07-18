use std::{num::NonZero, sync::Arc};

use vulkano::{
    device::{Device, physical::PhysicalDevice},
    format::Format,
    image::{Image, ImageUsage},
    instance::Instance,
    swapchain::{PresentMode, Surface, Swapchain, SwapchainCreateInfo},
};

use crate::graphics::renderer::vulkan::WindowLike;

pub(crate) fn create(
    window: Arc<impl WindowLike>,
    window_inner_size: [u32; 2],
    instance: Arc<Instance>,
    device: Arc<Device>,
    physical_device: &PhysicalDevice,
    desire_image_format: Option<Format>,
    desire_image_count: Option<NonZero<u32>>,
) -> (Arc<Swapchain>, Box<[Arc<Image>]>) {
    let surface = Surface::from_window(instance, window).expect("Failed to create surface");
    let capabilities = physical_device
        .surface_capabilities(&surface, Default::default())
        .expect("Failed to get surface capabilities");
    let composite_alpha = capabilities
        .supported_composite_alpha
        .into_iter()
        .next()
        .expect("Failed to get composite alpha mode");
    let image_format = {
        let image_formats = physical_device
            .surface_formats(&surface, Default::default())
            .expect("Failed to get surface formats");
        let first_image_format = image_formats.first().expect("unreachable").0;
        let desire_image_format = desire_image_format.unwrap_or(first_image_format);
        image_formats
            .iter()
            .find_map(|&(f, _)| (f == desire_image_format).then(|| f))
            .unwrap_or_else(|| {
                eprintln!("Failed to find desire image format: {desire_image_format:?}");
                first_image_format
            })
    };
    let min_image_count = desire_image_count.map_or(2, |x| x.get()).clamp(
        capabilities.min_image_count,
        capabilities.max_image_count.unwrap_or(u32::MAX),
    );
    Swapchain::new(
        device,
        surface,
        SwapchainCreateInfo {
            min_image_count,
            image_format,
            image_extent: window_inner_size,
            image_usage: ImageUsage::COLOR_ATTACHMENT,
            composite_alpha,
            present_mode: PresentMode::Fifo,
            ..Default::default()
        },
    )
    .map(|(swapchain, images)| (swapchain, images.into_boxed_slice()))
    .expect("Failed to create swapchain")
}

pub(crate) fn recreate(
    swapchain: &Arc<Swapchain>,
    create_info: SwapchainCreateInfo,
) -> (Arc<Swapchain>, Box<[Arc<Image>]>) {
    swapchain
        .recreate(create_info)
        .map(|(swapchain, images)| (swapchain, images.into_boxed_slice()))
        .expect("Failed to recreate swapchain")
}
