//! Vello Hybrid (now named vello_gpu) on Vulkan, initialized only by GPU requests.
use crate::{DisplayList, Image, PixelRect, Ui};
use anyhow::{Context, Result, ensure};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use vello_cpu::kurbo::Affine;
mod scene {
    const NATIVE_FILTERS: bool = true;
    type Scene = vello_hybrid::Scene;
    type Resources = vello_hybrid::Resources;
    include!("scene.rs");
}
pub(super) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
thread_local! { static DEVICE: RefCell<Weak<Device>> = const { RefCell::new(Weak::new()) }; }
pub(super) struct Device {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
}
fn device() -> Result<Rc<Device>> {
    DEVICE.with_borrow_mut(|slot| {
        if let Some(gpu) = slot.upgrade() {
            return Ok(gpu);
        }
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            ..Default::default()
        }))?;
        ensure!(
            adapter.get_info().device_type != wgpu::DeviceType::Cpu,
            "GPU mode requires a hardware Vulkan adapter"
        );
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        let gpu = Rc::new(Device {
            instance,
            adapter,
            device,
            queue,
        });
        *slot = Rc::downgrade(&gpu);
        Ok(gpu)
    })
}
pub(super) fn texture(gpu: &Device, size: (u32, u32), label: &str) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}
/// One retained GPU bitmap; geometry and shaping come from the CPU renderer's scene builder.
pub struct Canvas {
    gpu: Rc<Device>,
    renderer: vello_hybrid::Renderer,
    resources: vello_hybrid::Resources,
    pixels: wgpu::Texture,
    size: (u32, u32),
}
impl Canvas {
    /// Allocates a retained bitmap on this thread's lazily shared Vulkan device.
    pub fn new(width: u32, height: u32) -> Result<Self> {
        ensure!(
            width > 0 && height > 0 && width <= u16::MAX as u32 && height <= u16::MAX as u32,
            "invalid GPU bitmap size"
        );
        let gpu = device()?;
        ensure!(
            width <= gpu.device.limits().max_texture_dimension_2d
                && height <= gpu.device.limits().max_texture_dimension_2d,
            "GPU bitmap exceeds device limits"
        );
        let (renderer, resources) = vello_hybrid::Renderer::new(
            &gpu.device,
            &vello_hybrid::RenderTargetConfig {
                format: FORMAT,
                width: width as u16,
                height: height as u16,
            },
        );
        let pixels = texture(&gpu, (width, height), "stillkit retained content");
        Ok(Self {
            gpu,
            renderer,
            resources,
            pixels,
            size: (width, height),
        })
    }
    /// Rerasterizes only these physical bounds, preserving the rest of the bitmap.
    pub fn draw(&mut self, ui: &mut Ui, list: &DisplayList, area: PixelRect) -> Result<()> {
        ensure!(
            area.x + area.width <= self.size.0 && area.y + area.height <= self.size.1,
            "GPU damage outside bitmap"
        );
        if area.width == 0 || area.height == 0 {
            return Ok(());
        }
        let mut scene = vello_hybrid::Scene::new(area.width as u16, area.height as u16);
        scene.set_transform(Affine::translate((-(area.x as f64), -(area.y as f64))));
        let mut uploads = self.gpu.device.create_command_encoder(&Default::default());
        let mut images = Vec::new();
        scene::draw_ops(
            &mut scene,
            &mut self.resources,
            &mut ui.text,
            &list.ops,
            Affine::translate((-(area.x as f64), -(area.y as f64))),
            &mut |resources, pix| {
                let id = self.renderer.upload_image(
                    resources,
                    &self.gpu.device,
                    &self.gpu.queue,
                    &mut uploads,
                    &pix,
                );
                images.push(id);
                vello_cpu::ImageSource::opaque_id(id)
            },
        )?;
        self.gpu.queue.submit([uploads.finish()]);
        let target = texture(&self.gpu, (area.width, area.height), "stillkit damage");
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        self.renderer.render(
            &scene,
            &mut self.resources,
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            &vello_hybrid::RenderSize {
                width: area.width as u16,
                height: area.height as u16,
            },
            &target.create_view(&Default::default()),
            None,
            &vello_hybrid::TextureBindings::new(),
            vello_hybrid::TargetInit::Clear(Default::default()),
        )?;
        encoder.copy_texture_to_texture(
            target.as_image_copy(),
            wgpu::TexelCopyTextureInfo {
                origin: wgpu::Origin3d {
                    x: area.x,
                    y: area.y,
                    z: 0,
                },
                ..self.pixels.as_image_copy()
            },
            target.size(),
        );
        for id in images {
            self.renderer
                .destroy_image(&mut self.resources, &mut encoder, id);
        }
        self.gpu.queue.submit([encoder.finish()]);
        Ok(())
    }
    /// Reads premultiplied BGRA pixels, waiting for the GPU; intended for snapshots.
    pub fn image(&self) -> Result<Image> {
        self.read_texture(&self.pixels)
    }
    fn read_texture(&self, texture: &wgpu::Texture) -> Result<Image> {
        let (width, height) = self.size;
        let stride = (width * 4).next_multiple_of(256);
        let buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("stillkit readback"),
            size: stride as u64 * height as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: None,
                },
            },
            self.pixels.size(),
        );
        self.gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv()??;
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
        for row in buffer
            .slice(..)
            .get_mapped_range()?
            .chunks_exact(stride as usize)
        {
            pixels.extend_from_slice(&row[..width as usize * 4]);
        }
        buffer.unmap();
        Ok(Image {
            width,
            height,
            pixels,
        })
    }
}
/// Headless GPU rendering, including the same glyph outlines as CPU mode.
pub fn render(ui: &mut Ui, list: &DisplayList, width: u32, height: u32) -> Result<Image> {
    let mut canvas = Canvas::new(width, height)?;
    canvas.draw(ui, list, PixelRect::full(width, height))?;
    canvas.image()
}

#[derive(Default)]
pub(super) struct GlassState {
    pub glasses: Vec<crate::Glass>,
    pub style: crate::glass::GlassStyle,
    pub spacing: f32,
    pub backdrop: Option<Image>,
    pub revision: u64,
    pub backdrop_revision: u64,
}
pub(super) struct Window {
    // Drop the Vulkan surface before the connection / wl_surface owner releases its handles.
    surface: wgpu::Surface<'static>,
    canvas: Canvas,
    config: wgpu::SurfaceConfiguration,
    glass: Option<super::gpu_glass::Layer>,
    scale: f32,
    revision: u64,
    output: Option<wgpu::Texture>,
    animating: bool,
    alpha: Option<super::gpu_glass::Blit>,
    backdrop_revision: u64,
}
impl Window {
    /// The caller keeps the native display and wl_surface alive until this value is dropped.
    pub unsafe fn new(
        display: std::ptr::NonNull<std::ffi::c_void>,
        surface: std::ptr::NonNull<std::ffi::c_void>,
        size: (u32, u32),
    ) -> Result<Self> {
        let canvas = Canvas::new(size.0, size.1)?;
        // SAFETY: the owning Wayland Native drops Window before destroying these handles.
        let surface = unsafe {
            canvas
                .gpu
                .instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(raw_window_handle::RawDisplayHandle::Wayland(
                        raw_window_handle::WaylandDisplayHandle::new(display),
                    )),
                    raw_window_handle: raw_window_handle::RawWindowHandle::Wayland(
                        raw_window_handle::WaylandWindowHandle::new(surface),
                    ),
                })
        }?;
        let mut config = surface
            .get_default_config(&canvas.gpu.adapter, size.0, size.1)
            .context("Vulkan Wayland configuration")?;
        ensure!(
            surface
                .get_capabilities(&canvas.gpu.adapter)
                .usages
                .contains(wgpu::TextureUsages::COPY_DST),
            "Wayland swapchain cannot receive retained content"
        );
        config.format = FORMAT;
        config.usage = wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT;
        config.alpha_mode = wgpu::CompositeAlphaMode::PreMultiplied;
        config.desired_maximum_frame_latency = 1;
        surface.configure(&canvas.gpu.device, &config);
        let output = None;
        Ok(Self {
            surface,
            canvas,
            config,
            output,
            animating: false,
            alpha: None,
            backdrop_revision: 0,
            glass: None,
            scale: 1.,
            revision: u64::MAX,
        })
    }
    pub fn draw(
        &mut self,
        ui: &mut Ui,
        list: &DisplayList,
        size: (u32, u32),
        area: PixelRect,
        glass: &GlassState,
        scale: f32,
    ) -> Result<()> {
        if self.canvas.size != size {
            self.canvas = Canvas::new(size.0, size.1)?;
            self.glass = None;
            self.revision = u64::MAX;
            self.output = None;
            self.config.width = size.0;
            self.config.height = size.1;
            self.surface
                .configure(&self.canvas.gpu.device, &self.config);
        }
        self.canvas.draw(ui, list, area)?;
        if self.backdrop_revision != glass.backdrop_revision {
            self.glass = None;
            self.backdrop_revision = glass.backdrop_revision;
        }
        if self.revision != glass.revision {
            self.set_glass(
                &glass.glasses,
                glass.style,
                glass.spacing,
                scale,
                glass.backdrop.as_ref(),
            );
            self.revision = glass.revision;
        }
        self.present()
    }
    pub fn image(&self) -> Result<Image> {
        self.canvas
            .read_texture(self.output.as_ref().unwrap_or(&self.canvas.pixels))
    }
    pub fn opacity(&mut self, alpha: u8) {
        self.alpha = if alpha == 255 {
            None
        } else {
            Some(super::gpu_glass::Blit::new(
                &self.canvas.gpu,
                alpha as f32 / 255.,
            ))
        };
    }
    pub fn set_alpha(&mut self, alpha: u8) -> Result<()> {
        self.opacity(alpha);
        self.present()
    }
    pub fn glass_revision(&self) -> u64 {
        self.revision
    }
    pub fn set_glass(
        &mut self,
        glasses: &[crate::Glass],
        style: crate::glass::GlassStyle,
        spacing: f32,
        scale: f32,
        backdrop: Option<&Image>,
    ) {
        self.scale = scale;
        if glasses.is_empty() {
            self.glass = None;
            self.output = None;
            self.animating = false;
        } else {
            let layer = self.glass.get_or_insert_with(|| {
                super::gpu_glass::Layer::new(&self.canvas.gpu, self.canvas.size, backdrop)
            });
            layer.set(glasses, &style, spacing, scale);
            self.animating = true;
        }
    }
    pub fn animating(&self) -> bool {
        self.animating
    }
    pub fn present(&mut self) -> Result<()> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            other => anyhow::bail!("Wayland acquire: {other:?}"),
        };
        let mut encoder = self
            .canvas
            .gpu
            .device
            .create_command_encoder(&Default::default());
        if let Some(glass) = &mut self.glass {
            let output = self.output.get_or_insert_with(|| {
                texture(
                    &self.canvas.gpu,
                    self.canvas.size,
                    "stillkit composed frame",
                )
            });
            self.animating = glass.render(
                &self.canvas.gpu,
                &mut encoder,
                &output.create_view(&Default::default()),
                &self.canvas.pixels,
                self.canvas.size,
                self.scale,
            );
        }
        let output = self.output.as_ref().unwrap_or(&self.canvas.pixels);
        if let Some(alpha) = &self.alpha {
            alpha.draw(
                &self.canvas.gpu,
                &mut encoder,
                &frame.texture.create_view(&Default::default()),
                output,
                false,
            );
        } else {
            encoder.copy_texture_to_texture(
                output.as_image_copy(),
                frame.texture.as_image_copy(),
                output.size(),
            );
        }
        self.canvas.gpu.queue.submit([encoder.finish()]);
        self.canvas.gpu.queue.present(frame);
        Ok(())
    }
}
