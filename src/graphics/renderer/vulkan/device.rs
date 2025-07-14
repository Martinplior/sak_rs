use std::sync::Arc;

use vulkano::device::{
    Device, DeviceCreateInfo, DeviceExtensions, DeviceFeatures, Queue, QueueCreateInfo, QueueFlags,
    physical::PhysicalDevice,
};

/// retrive `device` by `queue.device()`
pub(crate) fn from_phisical_device(
    physical_device: Arc<PhysicalDevice>,
    extensions: DeviceExtensions,
    features: DeviceFeatures,
) -> Arc<Queue> {
    let queue_family_index = physical_device
        .queue_family_properties()
        .iter()
        .position(|queue_family_properties| {
            queue_family_properties
                .queue_flags
                .contains(QueueFlags::GRAPHICS)
        })
        .expect("No queue family with graphics support found") as u32;
    let min_extensions = DeviceExtensions {
        khr_swapchain: true,
        ..Default::default()
    };
    Device::new(
        physical_device,
        DeviceCreateInfo {
            queue_create_infos: vec![QueueCreateInfo {
                queue_family_index,
                ..Default::default()
            }],
            enabled_extensions: extensions.union(&min_extensions),
            enabled_features: features,
            ..Default::default()
        },
    )
    .map(|(_, mut queue_iter)| queue_iter.next().expect("No queue created"))
    .expect("Failed to create device")
}
