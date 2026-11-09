//! Persist an accepted domain edit so cargo-watch restarts can restore it.
use std::path::PathBuf;
use stillkit::el::{self, App, col, text};
use stillkit::{Kind, Scheme, Theme};

struct State {
    count: u64,
    path: PathBuf,
    error: Option<String>,
}
fn main() -> anyhow::Result<()> {
    let path = std::env::var_os("STILLKIT_STATE")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("set STILLKIT_STATE to a writable state-file path"))?;
    let count = match std::fs::read_to_string(&path) {
        Ok(value) => value.trim().parse()?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
        Err(e) => return Err(e.into()),
    };
    el::run(App {
        title: "Restart with state",
        size: (400., 200.),
        kind: Kind::App { min: (250, 150) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: State {
            count,
            path,
            error: None,
        },
        view: |s: &State| {
            col()
                .pad(24.)
                .child(
                    text(format!("Count: {}", s.count))
                        .key("increment")
                        .on_press(|s: &mut State| {
                            let next = s.count.saturating_add(1);
                            // Commit the domain edit only after persistence succeeds.
                            match std::fs::write(&s.path, next.to_string()) {
                                Ok(()) => {
                                    s.count = next;
                                    s.error = None;
                                }
                                Err(e) => s.error = Some(e.to_string()),
                            }
                        }),
                )
                .child(text(
                    s.error
                        .as_deref()
                        .unwrap_or("Click the count, edit Rust, then restart."),
                ))
        },
    })
}
