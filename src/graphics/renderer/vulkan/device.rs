use std::sync::Arc;

use vulkano::device::{
    Device, DeviceCreateInfo, DeviceExtensions, DeviceFeatures, Queue, QueueCreateInfo, QueueFlags,
    physical::PhysicalDevice,
};

pub(crate) fn from_phisical_device(
    physical_device: Arc<PhysicalDevice>,
) -> (Arc<Device>, Arc<Queue>) {
    let queue_family_index = physical_device
        .queue_family_properties()
        .iter()
        .position(|queue_family_properties| {
            queue_family_properties
                .queue_flags
                .contains(QueueFlags::GRAPHICS)
        })
        .expect("No queue family with graphics support found") as u32;
    Device::new(
        physical_device,
        DeviceCreateInfo {
            queue_create_infos: vec![QueueCreateInfo {
                queue_family_index,
                ..Default::default()
            }],
            enabled_extensions: DeviceExtensions {
                khr_swapchain: true,
                ..Default::default()
            },
            enabled_features: DeviceFeatures {
                shader_float64: true,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .map(|(device, mut queue_iter)| (device, queue_iter.next().expect("No queue created")))
    .expect("Failed to create device")
}
