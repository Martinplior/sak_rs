use std::{num::NonZero, sync::Arc};

use crossbeam_queue::SegQueue;
use parking_lot::Mutex;

use crate::{sync::spsc::OnceReceiver, thread::WorkerThread};

use super::Context;

pub struct RendererCreateInfo<'context> {
    pub context: &'context Context,
    pub target: wgpu::SurfaceTarget<'static>,
    pub target_size: Box<dyn Fn() -> [u32; 2] + Send + Sync>,
    pub desired_maxium_frame_latency: Option<NonZero<u32>>,
}

enum SurfaceConfigType {
    Size([u32; 2]),
    Vsync(bool),
}

struct Shared {
    target_size: Box<dyn Fn() -> [u32; 2] + Send + Sync>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config_queue: SegQueue<SurfaceConfigType>,
}

impl Shared {
    fn resize_outdated(&self) {
        let new_size = (self.target_size)();
        self.surface_config_queue
            .push(SurfaceConfigType::Size(new_size));
    }
}

struct SharedMut {
    surface_config: wgpu::SurfaceConfiguration,
}

impl SharedMut {
    fn render(
        &mut self,
        shared: &Shared,
        clear_color: wgpu::Color,
        add_commands: impl FnOnce(&mut wgpu::RenderPass),
    ) {
        let surface = &shared.surface;
        let device = &shared.device;
        let queue = &shared.queue;

        self.set_surface_config_when_need(shared);

        let output = surface
            .get_current_texture()
            .expect("failed to acquire next swap chain texture");
        output.suboptimal.then(|| shared.resize_outdated());
        let view = output.texture.create_view(&Default::default());
        let mut command_encoder = device.create_command_encoder(&Default::default());
        let mut render_pass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            ..Default::default()
        });
        add_commands(&mut render_pass);
        drop(render_pass);
        let command_buffer = command_encoder.finish();
        queue.submit([command_buffer]);
        output.present();
    }

    fn set_surface_config_when_need(&mut self, shared: &Shared) {
        let mut new_size = None;
        let mut new_vsync = None;
        while let Some(r) = shared.surface_config_queue.pop() {
            match r {
                SurfaceConfigType::Size(size) => new_size = Some(size),
                SurfaceConfigType::Vsync(vsync) => new_vsync = Some(vsync),
            }
        }
        let need_set_config = new_size.is_some() || new_vsync.is_some();
        need_set_config.then(|| {
            let size =
                new_size.unwrap_or_else(|| [self.surface_config.width, self.surface_config.height]);
            let present_mode = if let Some(v) = new_vsync {
                if v {
                    wgpu::PresentMode::AutoVsync
                } else {
                    wgpu::PresentMode::AutoNoVsync
                }
            } else {
                self.surface_config.present_mode
            };
            self.surface_config.width = size[0];
            self.surface_config.height = size[1];
            self.surface_config.present_mode = present_mode;
            shared
                .surface
                .configure(&shared.device, &self.surface_config);
        });
    }
}

pub struct Renderer {
    pub clear_color: wgpu::Color,

    shared: Arc<(Shared, Mutex<SharedMut>)>,
    render_worker: WorkerThread,
    render_receiver: Option<OnceReceiver<()>>,
}

impl Renderer {
    pub const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

    pub async fn new(create_info: RendererCreateInfo<'_>) -> Self {
        let RendererCreateInfo {
            context,
            target,
            target_size,
            desired_maxium_frame_latency,
        } = create_info;
        let instance = context.instance();
        let device = context.device();
        let queue = context.queue();

        let surface = instance
            .create_surface(target)
            .expect("failed to create surface");
        let size = target_size();
        let desired_maximum_frame_latency =
            desired_maxium_frame_latency.map(|x| x.get()).unwrap_or(2);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: Self::TEXTURE_FORMAT,
            width: size[0],
            height: size[1],
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency,
        };
        surface.configure(&device, &surface_config);
        let clear_color = wgpu::Color::TRANSPARENT;
        let render_worker = {
            let builder = std::thread::Builder::new().name("render".into());
            WorkerThread::with_builder(builder).expect("failed to create render thread")
        };

        let shared = {
            let shared = Shared {
                target_size,
                surface,
                device: device.clone(),
                queue: queue.clone(),
                surface_config_queue: SegQueue::new(),
            };
            let shared_mut = Mutex::new(SharedMut { surface_config });
            Arc::new((shared, shared_mut))
        };

        Self {
            clear_color,
            shared,
            render_worker,
            render_receiver: None,
        }
    }

    #[inline]
    pub fn device(&self) -> &wgpu::Device {
        &self.shared.0.device
    }

    #[inline]
    pub fn queue(&self) -> &wgpu::Queue {
        &self.shared.0.queue
    }

    pub fn set_vsync(&self, vsync: bool) {
        self.shared
            .0
            .surface_config_queue
            .push(SurfaceConfigType::Vsync(vsync));
    }

    /// blocking until render is finished.
    ///
    /// if you don't want to block, use [`try_render`](Self::try_render) instead.
    pub fn render(&mut self, add_commands: impl FnOnce(&mut wgpu::RenderPass) + Send + 'static) {
        let shared = self.shared.clone();
        let clear_color = self.clear_color;
        let new_render_receiver = self.render_worker.add_task_sync(move || {
            let (shared, shared_mut) = &*shared;
            shared_mut.lock().render(shared, clear_color, add_commands);
        });
        self.render_receiver
            .replace(new_render_receiver)
            .map(|r| r.recv());
    }

    /// never blocks, returns true if render is successful.
    ///
    /// if you want to block, use [`render`](Self::render) instead.
    pub fn try_render(
        &mut self,
        add_commands: impl FnOnce(&mut wgpu::RenderPass) + Send + 'static,
    ) -> bool {
        if let Some(r) = self.render_receiver.take()
            && let Err(r) = r.try_recv()
        {
            self.render_receiver = Some(r);
            return false;
        }
        // self.render_receiver is None here, so self.render never blocks
        self.render(add_commands);
        true
    }
}
