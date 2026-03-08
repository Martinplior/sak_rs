pub struct ContextConfig {
    pub instance_descriptor: wgpu::InstanceDescriptor,
    pub target: wgpu::SurfaceTarget<'static>,
    pub power_preference: wgpu::PowerPreference,
    pub device_descriptor: Box<dyn FnMut(&wgpu::Adapter) -> wgpu::DeviceDescriptor>,
}

pub struct Context {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl Context {
    pub async fn new(config: ContextConfig) -> Self {
        let ContextConfig {
            instance_descriptor,
            target,
            power_preference,
            mut device_descriptor,
        } = config;
        let instance = wgpu::Instance::new(&instance_descriptor);
        let surface = instance
            .create_surface(target)
            .expect("failed to create surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("failed to find an appropriate adapter");
        let (device, queue) = adapter
            .request_device(&device_descriptor(&adapter))
            .await
            .expect("failed to create device and queue");

        Self {
            instance,
            adapter,
            device,
            queue,
        }
    }

    #[inline]
    pub fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }

    #[inline]
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    #[inline]
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    #[inline]
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}
