//! An app's own 40-px title bar with the system's window controls centred in it (macOS
//! `Caption::Compact`) and a button in the title band. `cargo run -p stillkit --example caption`.
use std::time::{Duration, Instant};

use stillkit::el::{El, Runtime, col, el, row, text, token};
use stillkit::platform::current::host::{modifiers, wait, wake_at};
use stillkit::{Caption, Chrome, Event, Kind, Rect, Rgba, Role, Scheme, Style, Surface, Theme, next_event};

struct S {
    inset: f32,
    clicks: u32,
}

fn view(s: &S) -> El<S> {
    col().w_full().h_full()
        .child(
            row().h(40.0).pad_x(8.0).gap(8.0).align(stillkit::el::Align::Center).fill(token::CARD)
                .child(el().w(s.inset))
                .child(el().key("button").pad_x(10.0).h(26.0).radius(6.0).fill(token::FILL).align(stillkit::el::Align::Center)
                    .on_press(|s: &mut S| s.clicks += 1).child(text(format!("Title-bar button: {}", s.clicks)))),
        )
        .child(el().pad(20.0).child(el().key("body").pad_x(10.0).h(26.0).radius(6.0).fill(token::FILL).align(stillkit::el::Align::Center)
            .on_press(|s: &mut S| s.clicks += 100).child(text("Body button (+100)"))))
        .child(el().grow(1.0))
}

fn main() -> anyhow::Result<()> {
    let mut surface = Surface::new(Kind::App { min: (480, 300) }, "caption", Role::Window)?;
    surface.set_caption(Caption::Compact);
    let s = surface.scale();
    surface.place((300, 300, (640.0 * s) as i32, (360.0 * s) as i32));
    let inset = surface.system_controls().map_or(0.0, |r| r.right / s);
    println!("system controls: {:?} (device px), inset {inset}", surface.system_controls());
    let theme = Theme::new(Style::Solid, Scheme::Dark, stillkit::theme::SYSTEM_BLUE);
    let mut rt = Runtime::new(S { inset, clicks: 0 }, view, theme, surface.client_size(), s)?;
    rt.set_background(Some(Rgba::rgb8([24, 24, 27], 1.0)));
    rt.frame(Duration::ZERO);
    rt.present(&mut surface, (0, 0))?;
    surface.set_chrome(&[(Rect::new(0.0, 0.0, rt.size().0 as f32, 40.0 * s), Chrome::Caption)]);
    surface.show_active(false);
    let started = Instant::now();
    let quit = std::env::args().nth(1).and_then(|a| a.parse::<f64>().ok()).map(Duration::from_secs_f64);
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
                event => rt.handle(&event),
            }
        }
        if rt.frame(started.elapsed().saturating_sub(rt.now())) {
            rt.present(&mut surface, (0, 0))?;
            println!("clicks: {}", rt.state().clicks);
        }
        if quit.is_some_and(|q| started.elapsed() > q) {
            return Ok(());
        }
        wake_at(Some(Instant::now() + Duration::from_millis(200)));
    }
}
