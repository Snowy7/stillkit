//! GPU mode (`gpu` feature): Vello Hybrid on Metal through wgpu, made only by a GPU request,
//! and in-process glass (`stillkit-glass`), as the Linux GPU path does it. A surface's Metal
//! layer is a sublayer wgpu puts on the view. Without the feature none of this is compiled.
use crate::{DisplayList, Image, PixelRect, Ui};
use anyhow::{Context, Result, ensure};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use vello_hybrid::kurbo::Affine;

pub(super) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

thread_local! {
    static DEVICE: RefCell<Weak<Device>> = const { RefCell::new(Weak::new()) };
    /// This thread's GPU windows, for presenting animating glass between app frames.
    static WINDOWS: RefCell<Vec<Weak<RefCell<Window>>>> = const { RefCell::new(Vec::new()) };
}

/// The thread's Metal device, shared by its GPU surfaces and dropped with the last.
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
            backends: wgpu::Backends::METAL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            ..Default::default()
        }))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        let gpu = Rc::new(Device { instance, adapter, device, queue });
        *slot = Rc::downgrade(&gpu);
        Ok(gpu)
    })
}

pub(super) fn texture(gpu: &Device, size: (u32, u32), label: &str) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
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

/// One retained GPU bitmap, redrawn only in damaged bounds.
pub struct Canvas {
    gpu: Rc<Device>,
    renderer: vello_hybrid::Renderer,
    resources: vello_hybrid::Resources,
    pixels: wgpu::Texture,
    size: (u32, u32),
}

impl Canvas {
    /// Creates a new instance.
    pub fn new(width: u32, height: u32) -> Result<Self> {
        ensure!(width > 0 && height > 0 && width <= u16::MAX as u32 && height <= u16::MAX as u32, "invalid GPU bitmap size");
        let gpu = device()?;
        let max = gpu.device.limits().max_texture_dimension_2d;
        ensure!(width <= max && height <= max, "GPU bitmap exceeds device limits");
        let (renderer, resources) = vello_hybrid::Renderer::new(
            &gpu.device,
            &vello_hybrid::RenderTargetConfig { format: FORMAT, width: width as u16, height: height as u16 },
        );
        let pixels = texture(&gpu, (width, height), "stillkit retained content");
        Ok(Self { gpu, renderer, resources, pixels, size: (width, height) })
    }

    /// Draws into the current display list.
    pub fn draw(&mut self, ui: &mut Ui, list: &DisplayList, area: PixelRect) -> Result<()> {
        ensure!(area.x + area.width <= self.size.0 && area.y + area.height <= self.size.1, "GPU damage outside bitmap");
        if area.width == 0 || area.height == 0 {
            return Ok(());
        }
        let mut scene = vello_hybrid::Scene::new(area.width as u16, area.height as u16);
        scene.set_transform(Affine::translate((-(area.x as f64), -(area.y as f64))));
        let mut uploads = self.gpu.device.create_command_encoder(&Default::default());
        let mut images = Vec::new();
        super::gpu_scene::draw_ops(&mut scene, &mut self.resources, &mut ui.text, &list.ops, Affine::translate((-(area.x as f64), -(area.y as f64))), &mut |resources, pix| {
            let id = self.renderer.upload_image(resources, &self.gpu.device, &self.gpu.queue, &mut uploads, &pix);
            images.push(id);
            vello_hybrid::ImageSource::opaque_id(id)
        })?;
        self.gpu.queue.submit([uploads.finish()]);
        let target = texture(&self.gpu, (area.width, area.height), "stillkit damage");
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        self.renderer.render(
            &scene,
            &mut self.resources,
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            &vello_hybrid::RenderSize { width: area.width as u16, height: area.height as u16 },
            &target.create_view(&Default::default()),
            None,
            &vello_hybrid::TextureBindings::new(),
            vello_hybrid::TargetInit::Clear(Default::default()),
        )?;
        encoder.copy_texture_to_texture(
            target.as_image_copy(),
            wgpu::TexelCopyTextureInfo { origin: wgpu::Origin3d { x: area.x, y: area.y, z: 0 }, ..self.pixels.as_image_copy() },
            target.size(),
        );
        for id in images {
            self.renderer.destroy_image(&mut self.resources, &mut encoder, id);
        }
        self.gpu.queue.submit([encoder.finish()]);
        Ok(())
    }

    /// Reads premultiplied BGRA pixels, waiting for the GPU; for snapshots.
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
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(stride), rows_per_image: None },
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
        for row in buffer.slice(..).get_mapped_range()?.chunks_exact(stride as usize) {
            pixels.extend_from_slice(&row[..width as usize * 4]);
        }
        buffer.unmap();
        Ok(Image { width, height, pixels })
    }
}

/// Headless GPU rendering, with the same CoreText glyphs as CPU mode.
pub fn render(ui: &mut Ui, list: &DisplayList, width: u32, height: u32) -> Result<Image> {
    let mut canvas = Canvas::new(width, height)?;
    canvas.draw(ui, list, PixelRect::full(width, height))?;
    canvas.image()
}

/// What a GPU surface's glass should be; `revision` counts changes.
#[derive(Default)]
pub(super) struct GlassState {
    /// Glasses.
    pub glasses: Vec<crate::Glass>,
    /// Style.
    pub style: crate::glass::GlassStyle,
    /// Spacing.
    pub spacing: f32,
    /// Backdrop.
    pub backdrop: Option<Image>,
    /// Revision.
    pub revision: u64,
    /// Backdrop revision.
    pub backdrop_revision: u64,
}

/// A view's Metal layer (wgpu's surface), the retained content and its glass.
pub(super) struct Window {
    // Dropped before the view: the surface goes first.
    surface: wgpu::Surface<'static>,
    canvas: Canvas,
    config: wgpu::SurfaceConfiguration,
    glass: Option<super::gpu_glass::Layer>,
    scale: f32,
    revision: u64,
    output: Option<wgpu::Texture>,
    animating: bool,
    backdrop_revision: u64,
}

impl Window {
    /// # Safety
    /// The caller keeps `view` alive until this value is dropped.
    pub unsafe fn new(view: &super::view::View, size: (u32, u32)) -> Result<Self> {
        let canvas = Canvas::new(size.0, size.1)?;
        let handle = raw_window_handle::AppKitWindowHandle::new(std::ptr::NonNull::from(view).cast());
        // SAFETY: the owning surface drops this window before its view.
        let surface = unsafe {
            canvas.gpu.instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_window_handle::RawDisplayHandle::AppKit(raw_window_handle::AppKitDisplayHandle::new())),
                raw_window_handle: raw_window_handle::RawWindowHandle::AppKit(handle),
            })
        }?;
        let mut config = surface.get_default_config(&canvas.gpu.adapter, size.0, size.1).context("Metal layer configuration")?;
        let caps = surface.get_capabilities(&canvas.gpu.adapter);
        ensure!(caps.usages.contains(wgpu::TextureUsages::COPY_DST), "Metal layer cannot receive retained content");
        config.format = FORMAT;
        config.usage = wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT;
        // A transparent layer: the window server composites our premultiplied pixels.
        config.alpha_mode = [wgpu::CompositeAlphaMode::PreMultiplied, wgpu::CompositeAlphaMode::PostMultiplied]
            .into_iter()
            .find(|m| caps.alpha_modes.contains(m))
            .unwrap_or(config.alpha_mode);
        config.desired_maximum_frame_latency = 1;
        surface.configure(&canvas.gpu.device, &config);
        Ok(Self { surface, canvas, config, glass: None, scale: 1., revision: u64::MAX, output: None, animating: false, backdrop_revision: 0 })
    }

    /// Draws into the current display list.
    pub fn draw(&mut self, ui: &mut Ui, list: &DisplayList, size: (u32, u32), area: PixelRect, glass: &GlassState, scale: f32) -> Result<()> {
        if self.canvas.size != size {
            self.canvas = Canvas::new(size.0, size.1)?;
            self.glass = None;
            self.revision = u64::MAX;
            self.output = None;
            self.config.width = size.0;
            self.config.height = size.1;
            self.surface.configure(&self.canvas.gpu.device, &self.config);
        }
        self.canvas.draw(ui, list, area)?;
        if self.backdrop_revision != glass.backdrop_revision {
            self.glass = None;
            self.backdrop_revision = glass.backdrop_revision;
        }
        if self.revision != glass.revision || self.scale != scale {
            self.set_glass(glass, scale);
            self.revision = glass.revision;
        }
        self.present()
    }

    /// Image.
    pub fn image(&self) -> Result<Image> {
        self.canvas.read_texture(self.output.as_ref().unwrap_or(&self.canvas.pixels))
    }

    /// Glass revision.
    pub fn glass_revision(&self) -> u64 {
        self.revision
    }

    fn set_glass(&mut self, glass: &GlassState, scale: f32) {
        self.scale = scale;
        if glass.glasses.is_empty() {
            self.glass = None;
            self.output = None;
            self.animating = false;
        } else {
            let layer = self
                .glass
                .get_or_insert_with(|| super::gpu_glass::Layer::new(&self.canvas.gpu, self.canvas.size, glass.backdrop.as_ref()));
            layer.set(&glass.glasses, &glass.style, glass.spacing, scale);
            self.animating = true;
        }
    }

    /// Present.
    pub fn present(&mut self) -> Result<()> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            other => anyhow::bail!("Metal layer acquire: {other:?}"),
        };
        let mut encoder = self.canvas.gpu.device.create_command_encoder(&Default::default());
        if let Some(glass) = &mut self.glass {
            let output = self.output.get_or_insert_with(|| texture(&self.canvas.gpu, self.canvas.size, "stillkit composed frame"));
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
        encoder.copy_texture_to_texture(output.as_image_copy(), frame.texture.as_image_copy(), output.size());
        self.canvas.gpu.queue.submit([encoder.finish()]);
        self.canvas.gpu.queue.present(frame);
        Ok(())
    }
}

/// Tracks a surface's GPU window for [`tick`].
pub(super) fn register(window: &Rc<RefCell<Window>>) {
    WINDOWS.with_borrow_mut(|w| {
        w.retain(|w| w.strong_count() > 0);
        w.push(Rc::downgrade(window));
    });
}

/// Whether any glass still moves (its springs settle on their own).
pub(super) fn animating() -> bool {
    WINDOWS.with_borrow(|w| w.iter().filter_map(Weak::upgrade).any(|w| w.try_borrow().is_ok_and(|w| w.animating)))
}

/// Presents every animating window's next glass frame.
pub(super) fn tick() {
    let windows: Vec<_> = WINDOWS.with_borrow(|w| w.iter().filter_map(Weak::upgrade).collect());
    for window in windows {
        if let Ok(mut w) = window.try_borrow_mut()
            && w.animating
            && let Err(e) = w.present()
        {
            log::error!("Metal glass frame: {e}");
        }
    }
}
