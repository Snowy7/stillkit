//! A native layer under el content (macOS; Windows with `--features gpu`): a toolbar, a
//! growing `native("viewport")` element with a floating pill drawn over it, and a status bar.
//! The host loop places the surface's layer where `Runtime::natives` says. Here the layer is
//! only coloured (macOS: its `backgroundColor`); a real host presents to it with Metal or
//! Vulkan. `cargo run -p stillkit --example native_layer`.
use std::time::{Duration, Instant};

use anyhow::Result;
use stillkit::el::{El, Runtime, col, el, native, row, text, token};
use stillkit::platform::current::host::{modifiers, wait};
use stillkit::{Event, Kind, NativeHandle, Rgba, Role, Scheme, Style, Surface, Theme, TextRole, next_event};

#[derive(Default)]
struct Viewer {
    clicks: u32,
}

fn view(s: &Viewer) -> El<Viewer> {
    col().w_full().h_full()
        .child(row().h(40.0).pad_x(12.0).align(stillkit::el::Align::Center).fill(token::CARD).child(text("Scene").font(TextRole::Headline)))
        .child(
            native("viewport").grow(1.0).on_pointer(|s: &mut Viewer, e, _| {
                if e.phase == stillkit::el::PointerPhase::Down {
                    s.clicks += 1;
                    return true;
                }
                false
            })
            .child(
                el().absolute(16.0, 16.0).pad_x(10.0).h(28.0).radius(14.0).fill(Rgba::new(0.0, 0.0, 0.0, 0.55))
                    .align(stillkit::el::Align::Center)
                    .child(text(format!("Clicks on the native layer: {}", s.clicks)).color(Rgba::new(1.0, 1.0, 1.0, 1.0))),
            ),
        )
        .child(row().h(24.0).pad_x(12.0).align(stillkit::el::Align::Center).fill(token::CARD).child(text("Native layer under a hole").font(TextRole::Footnote)))
}

fn main() -> Result<()> {
    stillkit::platform::set_render(if cfg!(windows) { stillkit::platform::Render::Gpu } else { stillkit::platform::Render::Cpu });
    let mut surface = Surface::new(Kind::App { min: (480, 320) }, "native layer", Role::Window)?;
    let s = surface.scale();
    surface.place((200, 200, (720.0 * s) as i32, (480.0 * s) as i32));
    let theme = Theme::new(Style::Solid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE);
    let mut rt = Runtime::new(Viewer::default(), view, theme, surface.client_size(), s)?;
    rt.set_background(Some(Rgba::rgb8([24, 24, 27], 1.0)));
    surface.set_background(Rgba::rgb8([24, 24, 27], 1.0));
    rt.frame(Duration::ZERO);
    let started = Instant::now();
    let mut coloured = false;
    rt.present(&mut surface, (0, 0))?;
    surface.show_active(false);
    let quit_after = std::env::args().nth(1).and_then(|a| a.parse::<f64>().ok()).map(Duration::from_secs_f64);
    loop {
        wait();
        while let Some((id, event)) = next_event() {
            if id != surface.id() {
                continue;
            }
            match event {
                Event::Close => return Ok(()),
                Event::Resized(w, h) => rt.resize((w, h)),
                Event::Key(vk) => rt.key_with(vk, modifiers()),
                event => {
                    rt.set_modifiers(modifiers());
                    rt.handle(&event);
                }
            }
        }
        if rt.frame(started.elapsed().saturating_sub(rt.now())) {
            rt.present(&mut surface, (0, 0))?;
        }
        // Place each native layer where its element shows; hide the rest.
        for (name, rect) in rt.natives() {
            let handle = surface.native_layer(&name)?;
            if !coloured {
                colour(handle);
                coloured = true;
            }
            surface.place_native_layer(&name, Some(rect));
        }
        if quit_after.is_some_and(|q| started.elapsed() > q) {
            return Ok(());
        }
        stillkit::platform::current::host::wake_at(quit_after.map(|q| Instant::now() + q.min(Duration::from_millis(250))));
    }
}

/// Stands in for a renderer: a gradient-free, recognisable colour on the layer.
fn colour(handle: NativeHandle) {
    #[cfg(target_os = "macos")]
    if let NativeHandle::Metal(layer) = handle {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;
        let color = objc2_core_graphics::CGColor::new_srgb(0.20, 0.42, 0.95, 1.0);
        // SAFETY: a live CALayer; backgroundColor takes a CGColor.
        unsafe {
            let layer = &*(layer as *const AnyObject);
            let _: () = msg_send![layer, setBackgroundColor: &*color];
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = handle;
}
