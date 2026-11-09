//! GPU mode ([`Render::Gpu`](crate::platform::Render)): the same Direct2D drawing code as the
//! software canvas ([`Painter`]), on a hardware Direct3D 11 device, presented through a
//! DirectComposition surface. Only the damaged rectangles are drawn (one `BeginDraw` each), and
//! nothing is drawn or committed when nothing changed.
//!
//! The device is made on this thread's first GPU surface and shared by all of them; a process
//! with only CPU surfaces never makes one, so no GPU driver loads (`tests/no_gpu_driver.rs`). The
//! low-power adapter is used unless the app asks otherwise ([`set_power`]).

use std::cell::OnceCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU8, Ordering};

use anyhow::{Context, Result};
use windows::Win32::Foundation::{HMODULE, HWND, POINT, RECT};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
    D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1CreateFactory,
    ID2D1Device, ID2D1DeviceContext, ID2D1Factory, ID2D1Factory1, ID2D1Image,
};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_10_1,
    D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
    D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionEffectGroup,
    IDCompositionSurface, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::DirectWrite::IDWriteRenderingParams;
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_ADAPTER_DESC1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_CREATE_FACTORY_FLAGS,
    DXGI_GPU_PREFERENCE, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE, DXGI_GPU_PREFERENCE_MINIMUM_POWER,
    IDXGIAdapter1, IDXGIDevice, IDXGIFactory6, IDXGISurface,
};
use windows::core::Interface;

use crate::geom::PixelRect;
use crate::render::{Image, Painter, Ui};
use crate::scene::DisplayList;
use crate::text::Text;

/// Which adapter GPU mode uses, on a PC with more than one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Power {
    /// The integrated GPU when there is one (the default): least power, least heat.
    #[default]
    Low,
    /// The discrete GPU.
    High,
}

static POWER: AtomicU8 = AtomicU8::new(0);

/// Picks the adapter for GPU devices made from now on (the default is [`Power::Low`]). Set it
/// before the first GPU surface: devices already made keep theirs.
pub fn set_power(power: Power) {
    POWER.store(power as u8, Ordering::Relaxed);
}

/// The adapter preference set with [`set_power`].
pub fn power() -> Power {
    if POWER.load(Ordering::Relaxed) == Power::High as u8 { Power::High } else { Power::Low }
}

/// The hardware adapter a GPU device uses: its name and its PCI ids (to find the same one in
/// another API, such as wgpu for glass).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adapter {
    /// Name.
    pub name: String,
    /// Vendor.
    pub vendor: u32,
    /// Device.
    pub device: u32,
}

/// The adapter GPU mode would use with `power` (without making a device).
pub fn adapter(power: Power) -> Option<Adapter> {
    pick(power).ok().map(|(_, adapter)| adapter)
}

/// The first hardware adapter in the order `power` prefers.
pub(crate) fn pick(power: Power) -> Result<(IDXGIAdapter1, Adapter)> {
    let preference: DXGI_GPU_PREFERENCE = match power {
        Power::Low => DXGI_GPU_PREFERENCE_MINIMUM_POWER,
        Power::High => DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
    };
    // SAFETY: documented enumeration calls; each out-struct outlives its call.
    unsafe {
        let factory: IDXGIFactory6 =
            CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)).context("DXGI factory")?;
        let mut index = 0;
        while let Ok(adapter) = factory.EnumAdapterByGpuPreference::<IDXGIAdapter1>(index, preference) {
            index += 1;
            let desc: DXGI_ADAPTER_DESC1 = adapter.GetDesc1()?;
            if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                continue;
            }
            let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(128);
            let info = Adapter {
                name: String::from_utf16_lossy(&desc.Description[..len]),
                vendor: desc.VendorId,
                device: desc.DeviceId,
            };
            return Ok((adapter, info));
        }
    }
    anyhow::bail!("no hardware adapter")
}

/// This thread's hardware Direct2D device and its composition device.
pub struct Device {
    /// Adapter.
    pub adapter: Adapter,
    factory: ID2D1Factory,
    dc: ID2D1DeviceContext,
    d3d: ID3D11Device,
    context: ID3D11DeviceContext,
    pub(crate) dcomp: IDCompositionDevice,
    _d2d: ID2D1Device,
}

thread_local! { static DEVICE: OnceCell<Option<Rc<Device>>> = const { OnceCell::new() }; }

/// This thread's GPU device, made on first use; `None` when there is no hardware adapter (the
/// caller then draws on the CPU).
pub fn device() -> Option<Rc<Device>> {
    DEVICE.with(|cell| {
        cell.get_or_init(|| {
            Device::new(power())
                .inspect_err(|e| log::error!("GPU mode unavailable, drawing on the CPU: {e:#}"))
                .ok()
                .map(Rc::new)
        })
        .clone()
    })
}

impl Device {
    fn new(power: Power) -> Result<Self> {
        let (adapter, info) = pick(power)?;
        // SAFETY: documented creation calls; every out-pointer outlives its call.
        unsafe {
            let (mut d3d, mut context) = (None, None);
            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[
                    D3D_FEATURE_LEVEL_11_1,
                    D3D_FEATURE_LEVEL_11_0,
                    D3D_FEATURE_LEVEL_10_1,
                    D3D_FEATURE_LEVEL_10_0,
                ]),
                D3D11_SDK_VERSION,
                Some(&mut d3d),
                None,
                Some(&mut context),
            )
            .context("hardware Direct3D 11 device")?;
            let d3d = d3d.context("no Direct3D device")?;
            let context = context.context("no Direct3D context")?;
            let dxgi: IDXGIDevice = d3d.cast()?;
            let factory: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).context("Direct2D 1.1")?;
            let d2d = factory.CreateDevice(&dxgi).context("Direct2D device")?;
            let dc = d2d.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            let dcomp: IDCompositionDevice =
                DCompositionCreateDevice(&dxgi).context("DirectComposition device")?;
            Ok(Self {
                adapter: info,
                factory: factory.cast()?,
                dc,
                d3d,
                context,
                dcomp,
                _d2d: d2d,
            })
        }
    }

    /// A painter on this device's context, drawing text with `params`.
    fn painter(&self, params: &IDWriteRenderingParams) -> Result<Painter> {
        let mut painter = Painter::new(self.dc.cast()?, &self.factory, params)?;
        painter.native_filters = true;
        Ok(painter)
    }

    /// Points the context at `surface` (a texture of this device) for one draw.
    fn bind(&self, surface: &IDXGISurface) -> Result<()> {
        let properties = D2D1_BITMAP_PROPERTIES1 {
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
            colorContext: std::mem::ManuallyDrop::new(None),
        };
        // SAFETY: a surface of this device; the bitmap is released when the target is unset.
        unsafe {
            let bitmap = self.dc.CreateBitmapFromDxgiSurface(surface, Some(&properties))?;
            self.dc.SetTarget(&bitmap);
        }
        Ok(())
    }

    fn unbind(&self) {
        // SAFETY: clears the context's target, releasing the bitmap of `bind`.
        unsafe { self.dc.SetTarget(None::<&ID2D1Image>) };
    }
}

/// One GPU surface's composition: a root visual on its window, its content (a DirectComposition
/// surface Direct2D draws into) on top, and room below for glass (`gpu_glass`).
pub(crate) struct Layer {
    pub(crate) device: Rc<Device>,
    _target: IDCompositionTarget,
    pub(crate) root: IDCompositionVisual,
    pub(crate) content: IDCompositionVisual,
    effect: IDCompositionEffectGroup,
    surface: Option<IDCompositionSurface>,
    painter: Painter,
    opacity: f32,
    /// Uncommitted visual changes.
    dirty: bool,
}

impl Layer {
    pub(crate) fn new(hwnd: HWND, device: Rc<Device>, params: &IDWriteRenderingParams) -> Result<Self> {
        // SAFETY: documented calls on this thread's composition device.
        unsafe {
            let target = device.dcomp.CreateTargetForHwnd(hwnd, true)?;
            let root = device.dcomp.CreateVisual()?;
            let effect = device.dcomp.CreateEffectGroup()?;
            root.SetEffect(&effect)?;
            let content = device.dcomp.CreateVisual()?;
            root.AddVisual(&content, true, None)?;
            target.SetRoot(&root)?;
            let painter = device.painter(params)?;
            Ok(Self {
                device,
                _target: target,
                root,
                content,
                effect,
                surface: None,
                painter,
                opacity: 1.0,
                dirty: true,
            })
        }
    }

    /// Whether a content surface exists (none before the first present).
    pub(crate) fn has_content(&self) -> bool {
        self.surface.is_some()
    }

    /// A new, empty content surface of `size` (everything must be drawn again).
    pub(crate) fn resize(&mut self, size: (u32, u32)) -> Result<()> {
        self.surface = None;
        // SAFETY: documented calls on this thread's composition device.
        unsafe {
            let surface = self.device.dcomp.CreateSurface(
                size.0.max(1),
                size.1.max(1),
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_ALPHA_MODE_PREMULTIPLIED,
            )?;
            self.content.SetContent(&surface)?;
            self.surface = Some(surface);
        }
        self.dirty = true;
        Ok(())
    }

    /// Redraws each of `regions` (cleared first) with the operations of `list` that touch it.
    pub(crate) fn draw(&mut self, text: &mut Text, list: &DisplayList, regions: &[PixelRect]) -> Result<()> {
        let surface = self.surface.as_ref().context("no content surface")?;
        self.painter.retain(list);
        let (device, painter) = (&self.device, &self.painter);
        for area in regions {
            let rect = RECT {
                left: area.x as i32,
                top: area.y as i32,
                right: (area.x + area.width) as i32,
                bottom: (area.y + area.height) as i32,
            };
            let mut at = POINT::default();
            // SAFETY: one BeginDraw/EndDraw pair per region on our own surface; the context
            // draws only between them and lets go of the surface before EndDraw.
            unsafe {
                let target: IDXGISurface = surface.BeginDraw(Some(&rect), &mut at)?;
                let drawn = (|| -> Result<()> {
                    device.bind(&target)?;
                    let (dx, dy) = ((at.x - rect.left) as f32, (at.y - rect.top) as f32);
                    painter.base.set((dx, dy));
                    device.dc.BeginDraw();
                    device.dc.SetTransform(&windows_numerics::Matrix3x2::translation(dx, dy));
                    painter.region(text, list, *area);
                    let ended = device.dc.EndDraw(None, None).context("GPU drawing");
                    device.dc.SetTransform(&windows_numerics::Matrix3x2::identity());
                    ended
                })();
                painter.base.set((0.0, 0.0));
                device.unbind();
                surface.EndDraw()?;
                drawn?;
            }
        }
        self.dirty |= !regions.is_empty();
        Ok(())
    }

    /// Fades the whole surface (and its glass) without redrawing.
    pub(crate) fn set_opacity(&mut self, opacity: f32) -> Result<()> {
        if opacity != self.opacity {
            // SAFETY: an effect of this device.
            unsafe { self.effect.SetOpacity2(opacity)? };
            self.opacity = opacity;
            self.dirty = true;
        }
        Ok(())
    }

    /// Marks visual changes made from outside (glass) for the next [`commit`](Self::commit).
    pub(crate) fn touch(&mut self) {
        self.dirty = true;
    }

    /// Hands what changed to DWM in one commit; nothing when nothing changed.
    pub(crate) fn commit(&mut self) -> Result<()> {
        if std::mem::take(&mut self.dirty) {
            // SAFETY: this thread's composition device.
            unsafe { self.device.dcomp.Commit().context("DirectComposition commit")? };
        }
        Ok(())
    }
}

/// Draws `list` into a new `width`×`height` image on this thread's GPU device, the same way a
/// GPU surface shows it (for comparing the two modes). `None` without a hardware adapter.
pub fn render(ui: &mut Ui, list: &DisplayList, width: u32, height: u32) -> Result<Option<Image>> {
    let Some(device) = device() else { return Ok(None) };
    let painter = device.painter(ui.params())?;
    let desc = |usage, bind: u32, cpu: u32| D3D11_TEXTURE2D_DESC {
        Width: width.max(1),
        Height: height.max(1),
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: usage,
        BindFlags: bind,
        CPUAccessFlags: cpu,
        MiscFlags: 0,
    };
    // SAFETY: textures of this device; the mapped rows are read within the map's bounds and
    // pitch, then unmapped.
    unsafe {
        let mut texture: Option<ID3D11Texture2D> = None;
        device.d3d.CreateTexture2D(
            &desc(D3D11_USAGE_DEFAULT, D3D11_BIND_RENDER_TARGET.0 as u32, 0),
            None,
            Some(&mut texture),
        )?;
        let texture = texture.context("texture")?;
        device.bind(&texture.cast()?)?;
        device.dc.BeginDraw();
        painter.region(&mut ui.text, list, PixelRect::full(width, height));
        let ended = device.dc.EndDraw(None, None);
        device.unbind();
        ended.context("GPU drawing")?;
        let mut staging: Option<ID3D11Texture2D> = None;
        device.d3d.CreateTexture2D(
            &desc(D3D11_USAGE_STAGING, 0, D3D11_CPU_ACCESS_READ.0 as u32),
            None,
            Some(&mut staging),
        )?;
        let staging = staging.context("staging texture")?;
        device.context.CopyResource(&staging, &texture);
        let mut mapped = Default::default();
        device.context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        for row in 0..height as usize {
            let from = (mapped.pData as *const u8).add(row * mapped.RowPitch as usize);
            let at = row * width as usize * 4;
            std::ptr::copy_nonoverlapping(from, pixels.as_mut_ptr().add(at), width as usize * 4);
        }
        device.context.Unmap(&staging, 0);
        Ok(Some(Image { width, height, pixels }))
    }
}
