use std::sync::Arc;

use raw_window_handle::HasDisplayHandle;
use vulkano::{
    VulkanLibrary,
    instance::{Instance, InstanceCreateInfo, InstanceExtensions},
    swapchain::Surface,
};

pub(crate) fn from_event_loop(event_loop: &impl HasDisplayHandle) -> Arc<Instance> {
    Instance::new(
        VulkanLibrary::new().expect("Failed to load vulkan"),
        InstanceCreateInfo {
            enabled_extensions: InstanceExtensions {
                ..Surface::required_extensions(&event_loop)
                    .expect("Failed to get surface extensions")
            },
            ..InstanceCreateInfo::application_from_cargo_toml()
        },
    )
    .expect("Failed to create instance")
}
