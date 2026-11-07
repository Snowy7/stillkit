//! In-process Liquid Glass (`stillkit-glass`) for GPU surfaces: a wgpu DX12 swapchain on a
//! DirectComposition visual placed **under** the surface's content visual, in the same window.
//! The glass moves with its window for free (no `place` before a move, unlike an external
//! [`Backdrop`](crate::glass::Backdrop) window), and draws only when its scene changes or
//! animates.
//!
//! What it refracts (docs/ui.md, "Render modes"): the desktop wallpaper under the glass, decoded
//! once onto the GPU. That is what is truly behind desktop-band windows, and a cheap stand-in
//! behind bars and popups. Other apps' windows (Windows Graphics Capture) and the app's own
//! content beneath an in-window panel are later work.
//!
//! [`GlassLayer`] takes any composition device and parent visual, so the separate glass host
//! process of the measurement (`examples/glass_host.rs`) draws with the same code.

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use anyhow::{Context, Result};
use stillkit_glass as liquid;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::DirectComposition::IDCompositionVisual;
use windows::core::Interface;

use crate::glass::{Glass, GlassMaterial, GlassStyle};

/// Posted to a surface's window while its glass animates; the window procedure draws a frame.
pub(crate) const WM_GLASS_FRAME: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 31;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
const VIEW: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

/// This thread's wgpu device for glass, on the same adapter as the GPU surfaces.
pub struct GlassDevice {
    /// The adapter's name, as wgpu reports it.
    pub adapter_name: String,
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Compiled pipelines every layer forks.
    template: liquid::Renderer,
    /// The wallpaper over the primary screen (decoded once), with its size.
    wallpaper: OnceCell<(wgpu::Texture, (u32, u32))>,
}

thread_local! {
    static DEVICE: OnceCell<Option<Rc<GlassDevice>>> = const { OnceCell::new() };
    static LAYERS: RefCell<HashMap<isize, Weak<RefCell<GlassLayer>>>> = RefCell::new(HashMap::new());
}

/// This thread's glass device, made on first use; `None` without a DX12 adapter.
pub fn device() -> Option<Rc<GlassDevice>> {
    DEVICE.with(|cell| {
        cell.get_or_init(|| {
            GlassDevice::new()
                .inspect_err(|e| log::error!("in-process glass unavailable: {e:#}"))
                .ok()
                .map(Rc::new)
        })
        .clone()
    })
}

/// Waits for a wgpu future: native wgpu resolves them without an executor.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        std::thread::yield_now();
    }
}

impl GlassDevice {
    fn new() -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let want = super::gpu::pick(super::gpu::power()).ok().map(|(_, a)| a);
        let adapters = block_on(instance.enumerate_adapters(wgpu::Backends::DX12));
        let adapter = adapters
            .into_iter()
            .find(|a| {
                let info = a.get_info();
                want.as_ref().is_some_and(|w| info.vendor == w.vendor && info.device == w.device)
            })
            .context("the GPU surfaces' adapter in wgpu")?;
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("stillkit glass"),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))
        .context("wgpu device")?;
        let template = liquid::Renderer::new(&device, VIEW);
        Ok(Self {
            adapter_name: adapter.get_info().name,
            instance,
            adapter,
            device,
            queue,
            template,
            wallpaper: OnceCell::new(),
        })
    }

    /// The wallpaper texture, covering the primary screen (a flat desktop colour without one).
    fn wallpaper(&self) -> &(wgpu::Texture, (u32, u32)) {
        self.wallpaper.get_or_init(|| {
            let (w, h) = super::host::screen_size();
            let size = (w.max(1) as u32, h.max(1) as u32);
            let pixels = wallpaper_pixels(size).unwrap_or_else(|e| {
                log::warn!("no wallpaper for glass ({e:#}); using the desktop colour");
                desktop_colour(size)
            });
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("stillkit glass wallpaper"),
                size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: VIEW,
                usage: wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
                texture.as_image_copy(),
                &pixels,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(size.0 * 4), rows_per_image: None },
                texture.size(),
            );
            (texture, size)
        })
    }
}

/// The wallpaper file, scaled to cover `size` and cropped to its centre (Windows' "Fill").
fn wallpaper_pixels(size: (u32, u32)) -> Result<Vec<u8>> {
    use windows::Win32::Foundation::GENERIC_READ;
    use windows::Win32::Graphics::Imaging::{
        CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
        WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom,
        WICDecodeMetadataCacheOnDemand, WICRect,
    };
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETDESKWALLPAPER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };
    let mut path = [0u16; 260];
    // SAFETY: the buffer and its length in characters match; WIC objects are local.
    unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            path.len() as u32,
            Some(path.as_mut_ptr().cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )?;
        let len = path.iter().position(|&c| c == 0).unwrap_or(path.len());
        anyhow::ensure!(len > 0, "no wallpaper file");
        // WIC needs COM; an apartment that already exists (of either kind) is kept.
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let decoder = wic.CreateDecoderFromFilename(
            windows::core::PCWSTR(path.as_ptr()),
            None,
            GENERIC_READ,
            WICDecodeMetadataCacheOnDemand,
        )?;
        let frame = decoder.GetFrame(0)?;
        let (mut w, mut h) = (0, 0);
        frame.GetSize(&mut w, &mut h)?;
        anyhow::ensure!(w > 0 && h > 0, "empty wallpaper");
        let scale = (size.0 as f64 / w as f64).max(size.1 as f64 / h as f64);
        let (sw, sh) = (
            ((w as f64 * scale).ceil() as u32).max(size.0),
            ((h as f64 * scale).ceil() as u32).max(size.1),
        );
        let scaler = wic.CreateBitmapScaler()?;
        scaler.Initialize(&frame, sw, sh, WICBitmapInterpolationModeFant)?;
        let converter = wic.CreateFormatConverter()?;
        converter.Initialize(&scaler, &GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)?;
        let crop = WICRect {
            X: ((sw - size.0) / 2) as i32,
            Y: ((sh - size.1) / 2) as i32,
            Width: size.0 as i32,
            Height: size.1 as i32,
        };
        let mut pixels = vec![0u8; size.0 as usize * size.1 as usize * 4];
        converter.CopyPixels(&crop, size.0 * 4, &mut pixels)?;
        Ok(pixels)
    }
}

fn desktop_colour(size: (u32, u32)) -> Vec<u8> {
    use windows::Win32::Graphics::Gdi::{COLOR_DESKTOP, GetSysColor};
    // SAFETY: a plain colour query.
    let c = unsafe { GetSysColor(COLOR_DESKTOP) };
    let bgra = [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255];
    bgra.repeat(size.0 as usize * size.1 as usize)
}

/// Our glass optics for liquid-rust: refraction, frost (`blur` is physical px, liquid-rust
/// wants points), the rim highlight, and the stained tint over `Material::regular`.
fn material(m: &GlassMaterial, style: &GlassStyle, scale: f32) -> liquid::Material {
    let mut out = liquid::Material::regular();
    out.refraction = m.refraction;
    out.frost = m.blur / scale;
    out.specular = m.highlight;
    out.brightness = style.brightness;
    out.saturation = style.saturation;
    out.inner_shadow = style.inner_shadow;
    if let Some(depth) = style.depth {
        out.depth = depth;
    }
    if m.tint.a > 0.0 {
        out.tint = Some(liquid::Color { r: m.tint.r, g: m.tint.g, b: m.tint.b, a: m.tint.a });
    }
    out
}

/// One window's glass: a scene of one container (glass in it liquid-merges), drawn into a
/// premultiplied swapchain the size of the window.
pub struct GlassLayer {
    gpu: Rc<GlassDevice>,
    _visual: IDCompositionVisual,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: liquid::Renderer,
    scene: liquid::Scene,
    container: liquid::ContainerId,
    ids: Vec<liquid::GlassId>,
    /// The wallpaper under the window, the size of the window.
    content: (wgpu::Texture, wgpu::TextureView),
    content_at: Option<(i32, i32)>,
    content_changed: bool,
    scale: f32,
    last: Instant,
    /// While it animates: the thread posting [`WM_GLASS_FRAME`].
    frames: Option<Arc<AtomicBool>>,
}

impl GlassLayer {
    /// Glass drawn into `visual` (already in its window's tree), `size` pixels, at `scale`
    /// pixels per point. The caller commits its composition device afterwards.
    pub fn new(gpu: Rc<GlassDevice>, visual: IDCompositionVisual, size: (u32, u32), scale: f32) -> Result<Self> {
        // SAFETY: wgpu takes its own reference to the visual for as long as the surface lives.
        let surface = unsafe {
            gpu.instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CompositionVisual(visual.as_raw()))
                .context("wgpu surface on a composition visual")?
        };
        let size = (size.0.max(1), size.1.max(1));
        let mut config = surface
            .get_default_config(&gpu.adapter, size.0, size.1)
            .context("glass surface configuration")?;
        config.format = FORMAT;
        config.view_formats = vec![VIEW];
        config.alpha_mode = wgpu::CompositeAlphaMode::PreMultiplied;
        config.present_mode = wgpu::PresentMode::Fifo;
        config.desired_maximum_frame_latency = 1;
        surface.configure(&gpu.device, &config);
        let mut scene = liquid::Scene::new();
        let container = scene.add_container(16.0, 0);
        let content = content_texture(&gpu.device, size);
        Ok(Self {
            renderer: gpu.template.fork(&gpu.device),
            gpu,
            _visual: visual,
            surface,
            config,
            scene,
            container,
            ids: Vec::new(),
            content,
            content_at: None,
            content_changed: true,
            scale,
            last: Instant::now(),
            frames: None,
        })
    }

    /// Shows exactly `glasses` (window pixels) with `style`, fusing those within `spacing`
    /// (window pixels) of each other. Glass that was there already morphs to its new frame.
    pub fn set(&mut self, glasses: &[Glass], style: &GlassStyle, spacing: f32) {
        let s = self.scale;
        self.scene.set_container_spacing(self.container, spacing / s);
        while self.ids.len() > glasses.len() {
            let id = self.ids.pop().expect("longer than glasses");
            self.scene.remove(id);
        }
        for (i, g) in glasses.iter().enumerate() {
            let frame = liquid::Rect::new(g.body.left / s, g.body.top / s, g.body.width() / s, g.body.height() / s);
            let shape = if style.smoothing > 0.0 {
                liquid::Shape::Smooth { radius: g.radius / s, smoothing: style.smoothing }
            } else {
                liquid::Shape::Circular(g.radius / s)
            };
            let look = material(&g.material, style, s);
            // Dark glass under light ink (stillkit's default scheme), unless a light tint says
            // otherwise; the material carries no scheme of its own.
            let t = g.material.tint;
            let dark = t.a <= 0.0 || 0.2126 * t.r + 0.7152 * t.g + 0.0722 * t.b < 0.5;
            let appearance = liquid::Appearance {
                dark,
                transparency: 1.0 - g.material.clarity,
                adapt_to_content: false,
                ..Default::default()
            };
            match self.ids.get(i) {
                Some(&id) => {
                    if self.scene.glass(id).is_some_and(|old| old.frame != frame) {
                        self.scene.set_frame(id, frame);
                    }
                    self.scene.set_shape(id, shape);
                    self.scene.set_material(id, look);
                    self.scene.set_appearance(id, Some(appearance));
                }
                None => {
                    let glass = liquid::Glass::new(frame)
                        .shape(shape)
                        .material(look)
                        .appearance(appearance)
                        .container(self.container);
                    self.ids.push(self.scene.add(glass));
                }
            }
        }
    }

    /// Where the window is on screen, so the glass refracts the wallpaper under it.
    pub fn place(&mut self, origin: (i32, i32)) {
        if self.content_at == Some(origin) {
            return;
        }
        let (wallpaper, (ww, wh)) = self.gpu.wallpaper();
        let (w, h) = (self.config.width as i32, self.config.height as i32);
        let (left, top) = (origin.0.max(0), origin.1.max(0));
        let (right, bottom) = ((origin.0 + w).min(*ww as i32), (origin.1 + h).min(*wh as i32));
        if right > left && bottom > top {
            let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: wallpaper,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: left as u32, y: top as u32, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &self.content.0,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: (left - origin.0) as u32, y: (top - origin.1) as u32, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d { width: (right - left) as u32, height: (bottom - top) as u32, depth_or_array_layers: 1 },
            );
            self.gpu.queue.submit([encoder.finish()]);
        }
        self.content_at = Some(origin);
        self.content_changed = true;
    }

    /// A new window size (the swapchain and its backdrop follow).
    pub fn resize(&mut self, size: (u32, u32)) {
        let size = (size.0.max(1), size.1.max(1));
        if (self.config.width, self.config.height) == size {
            return;
        }
        (self.config.width, self.config.height) = size;
        self.surface.configure(&self.gpu.device, &self.config);
        self.content = content_texture(&self.gpu.device, size);
        if let Some(at) = self.content_at.take() {
            self.place(at);
        }
    }

    /// Steps the springs and draws one frame. Returns whether the glass still moves (call again
    /// next display frame).
    pub fn render(&mut self) -> bool {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.last = now;
        let animating = self.scene.update(dt);
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            _ => return animating,
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor { format: Some(VIEW), ..Default::default() });
        let gpu = &self.gpu;
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("stillkit glass clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
            })],
            ..Default::default()
        }));
        let size = (self.config.width, self.config.height);
        let content_changed = std::mem::take(&mut self.content_changed);
        self.renderer.render(
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &liquid::Frame { content: &self.content.1, target: &view, size, scale: self.scale, content_changed, copy_content: false },
            &self.scene,
        );
        gpu.queue.submit([encoder.finish()]);
        gpu.queue.present(frame);
        animating
    }

    /// Asks for frames on `hwnd` while the glass animates, and stops asking once it rests.
    fn drive(&mut self, hwnd: HWND, animating: bool) {
        match (animating, &self.frames) {
            (true, None) => {
                let running = Arc::new(AtomicBool::new(true));
                self.frames = Some(running.clone());
                let target = hwnd.0 as isize;
                let _ = std::thread::Builder::new().name("glass-motion".into()).spawn(move || {
                    let Ok(mut ticker) = crate::Ticker::new() else { return };
                    while running.load(Ordering::Relaxed) {
                        ticker.schedule(true);
                        // SAFETY: this worker owns its timer; a destroyed window refuses posts.
                        unsafe {
                            windows::Win32::System::Threading::WaitForSingleObject(ticker.handle(), 100);
                            let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                                Some(HWND(target as _)),
                                WM_GLASS_FRAME,
                                WPARAM(0),
                                LPARAM(0),
                            );
                        }
                    }
                });
            }
            (false, Some(_)) => {
                if let Some(running) = self.frames.take() {
                    running.store(false, Ordering::Relaxed);
                }
            }
            _ => {}
        }
    }
}

impl Drop for GlassLayer {
    fn drop(&mut self) {
        if let Some(running) = self.frames.take() {
            running.store(false, Ordering::Relaxed);
        }
    }
}

fn content_texture(device: &wgpu::Device, size: (u32, u32)) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("stillkit glass backdrop"),
        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: VIEW,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    (texture, view)
}

/// A surface's glass as the window procedure reaches it (to draw animation frames).
pub(crate) type Shared = Rc<RefCell<GlassLayer>>;

/// Draws a frame of `hwnd`'s glass now and keeps it animating as needed.
pub(crate) fn draw(hwnd: HWND, glass: &Shared) {
    if let Ok(mut layer) = glass.try_borrow_mut() {
        let animating = layer.render();
        layer.drive(hwnd, animating);
    }
}

/// Lets the window procedure find `hwnd`'s glass.
pub(crate) fn register(hwnd: HWND, glass: &Shared) {
    LAYERS.with_borrow_mut(|layers| {
        layers.insert(hwnd.0 as isize, Rc::downgrade(glass));
    });
}

pub(crate) fn unregister(hwnd: HWND) {
    LAYERS.with_borrow_mut(|layers| {
        layers.remove(&(hwnd.0 as isize));
    });
}

/// [`WM_GLASS_FRAME`]: the next animation frame of `hwnd`'s glass.
pub(crate) fn tick(hwnd: HWND) {
    let glass = LAYERS.with_borrow(|layers| layers.get(&(hwnd.0 as isize)).and_then(Weak::upgrade));
    if let Some(glass) = glass {
        draw(hwnd, &glass);
    }
}
