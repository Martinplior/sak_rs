use std::sync::{Arc, LazyLock};

use raw_window_handle::HasDisplayHandle;
use vulkano::{
    VulkanLibrary,
    command_buffer::allocator::StandardCommandBufferAllocator,
    descriptor_set::allocator::StandardDescriptorSetAllocator,
    device::{
        Device, DeviceCreateInfo, DeviceExtensions, DeviceFeatures, Queue, QueueCreateInfo,
        QueueFlags,
        physical::{PhysicalDevice, PhysicalDeviceType},
    },
    instance::{Instance, InstanceCreateInfo, InstanceExtensions},
    memory::allocator::StandardMemoryAllocator,
    swapchain::Surface,
};

pub struct ContextConfig<DisplayHandle>
where
    DisplayHandle: HasDisplayHandle,
{
    pub display_handle: DisplayHandle,
    pub device_extensions: DeviceExtensions,
    pub device_features: DeviceFeatures,
}

pub struct Allocators {
    memory:
        LazyLock<Arc<StandardMemoryAllocator>, Box<dyn FnOnce() -> Arc<StandardMemoryAllocator>>>,
    command_buffer: LazyLock<
        Arc<StandardCommandBufferAllocator>,
        Box<dyn FnOnce() -> Arc<StandardCommandBufferAllocator>>,
    >,
    descriptor_set: LazyLock<
        Arc<StandardDescriptorSetAllocator>,
        Box<dyn FnOnce() -> Arc<StandardDescriptorSetAllocator>>,
    >,
}

impl Allocators {
    #[inline]
    pub fn memory(&self) -> &Arc<StandardMemoryAllocator> {
        &self.memory
    }

    #[inline]
    pub fn command_buffer(&self) -> &Arc<StandardCommandBufferAllocator> {
        &self.command_buffer
    }

    #[inline]
    pub fn descriptor_set(&self) -> &Arc<StandardDescriptorSetAllocator> {
        &self.descriptor_set
    }

    fn new(device: Arc<Device>) -> Self {
        let memory = {
            let device_1 = device.clone();
            LazyLock::new(
                Box::new(move || Arc::new(StandardMemoryAllocator::new_default(device_1))) as _,
            )
        };
        let command_buffer = {
            let device_1 = device.clone();
            LazyLock::new(Box::new(move || {
                Arc::new(StandardCommandBufferAllocator::new(
                    device_1,
                    Default::default(),
                ))
            }) as _)
        };
        let descriptor_set = {
            let device_1 = device;
            LazyLock::new(Box::new(move || {
                Arc::new(StandardDescriptorSetAllocator::new(
                    device_1,
                    Default::default(),
                ))
            }) as _)
        };
        Self {
            memory,
            command_buffer,
            descriptor_set,
        }
    }
}

struct Queues {
    render_queue: Arc<Queue>,
    compute_queue: Arc<Queue>,
    transfer_queue: Option<Arc<Queue>>,
}

pub struct Context {
    instance: Arc<Instance>,
    device: Arc<Device>,
    queues: Queues,
    allocators: Allocators,
}

impl Context {
    pub fn new<DisplayHandle>(config: ContextConfig<DisplayHandle>) -> Self
    where
        DisplayHandle: HasDisplayHandle,
    {
        let ContextConfig {
            display_handle,
            device_extensions,
            device_features,
        } = config;
        let instance = create_instance(&display_handle);
        let physical_device = instance
            .enumerate_physical_devices()
            .expect("No physical device available")
            .next()
            .expect("No physical device available");
        let (device, queues) =
            create_device_and_queues(physical_device, device_extensions, device_features);
        let allocators = Allocators::new(device.clone());
        Self {
            instance,
            device,
            queues,
            allocators,
        }
    }

    #[inline]
    pub fn device_name(&self) -> &str {
        &self.device.physical_device().properties().device_name
    }

    #[inline]
    pub fn device_type(&self) -> PhysicalDeviceType {
        self.device.physical_device().properties().device_type
    }

    #[inline]
    pub fn max_memory(&self) -> u32 {
        self.device
            .physical_device()
            .properties()
            .max_memory_allocation_count
    }

    #[inline]
    pub fn instance(&self) -> &Arc<Instance> {
        &self.instance
    }

    #[inline]
    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    #[inline]
    pub fn render_queue(&self) -> &Arc<Queue> {
        &self.queues.render_queue
    }

    #[inline]
    pub fn compute_queue(&self) -> &Arc<Queue> {
        &self.queues.compute_queue
    }

    #[inline]
    pub fn transfer_queue(&self) -> Option<&Arc<Queue>> {
        self.queues.transfer_queue.as_ref()
    }

    #[inline]
    pub fn allocators(&self) -> &Allocators {
        &self.allocators
    }
}

fn create_instance(display_handle: &impl HasDisplayHandle) -> Arc<Instance> {
    Instance::new(
        VulkanLibrary::new().expect("Failed to load vulkan library"),
        InstanceCreateInfo {
            enabled_extensions: InstanceExtensions {
                ..Surface::required_extensions(&display_handle)
                    .expect("Failed to get surface extensions")
            },
            ..InstanceCreateInfo::application_from_cargo_toml()
        },
    )
    .expect("Failed to create instance")
}

fn create_device_and_queues(
    physical_device: Arc<PhysicalDevice>,
    extensions: DeviceExtensions,
    features: DeviceFeatures,
) -> (Arc<Device>, Queues) {
    let queue_family_properties = physical_device.queue_family_properties();
    let render_queue_family_index = queue_family_properties
        .iter()
        .position(|q| q.queue_flags.intersects(QueueFlags::GRAPHICS))
        .expect("No queue family with graphics support found")
        as u32;
    let compute_queue_family_index = queue_family_properties
        .iter()
        .enumerate()
        .map(|(i, q)| (i as u32, q))
        .find(|(i, q)| {
            q.queue_flags.intersects(QueueFlags::COMPUTE) && *i != render_queue_family_index
        })
        .map(|(i, _)| i);
    let transfer_queue_family_index = queue_family_properties
        .iter()
        .position(|q| {
            q.queue_flags.intersects(QueueFlags::TRANSFER)
                && !q
                    .queue_flags
                    .intersects(QueueFlags::GRAPHICS | QueueFlags::COMPUTE)
        })
        .map(|i| i as u32);
    let queue_create_infos: Vec<_> = [
        Some(QueueCreateInfo {
            queue_family_index: render_queue_family_index,
            ..Default::default()
        }),
        compute_queue_family_index.map(|queue_family_index| QueueCreateInfo {
            queue_family_index,
            ..Default::default()
        }),
        transfer_queue_family_index.map(|queue_family_index| QueueCreateInfo {
            queue_family_index,
            ..Default::default()
        }),
    ]
    .into_iter()
    .flatten()
    .collect();
    let (device, mut queue_iter) = Device::new(
        physical_device,
        DeviceCreateInfo {
            queue_create_infos,
            enabled_extensions: extensions,
            enabled_features: features,
            ..Default::default()
        },
    )
    .expect("Failed to create device");
    let render_queue = queue_iter.next().expect("unreachable");
    let compute_queue = compute_queue_family_index
        .is_some()
        .then(|| queue_iter.next().expect("unreachable"))
        .unwrap_or_else(|| render_queue.clone());
    let transfer_queue = transfer_queue_family_index
        .is_some()
        .then(|| queue_iter.next().expect("unreachable"));
    let queues = Queues {
        render_queue,
        compute_queue,
        transfer_queue,
    };
    (device, queues)
}
