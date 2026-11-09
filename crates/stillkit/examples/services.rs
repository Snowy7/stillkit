//! A tray-only app: no app window, timer or background thread.
use stillkit::{
    platform::current::host,
    tray::{Event, Icon, Tray},
};
fn main() -> anyhow::Result<()> {
    let mut tray = Tray::new(
        "stillkit.services",
        "stillkit: right-click to exit",
        Icon::rgba(16, 16, vec![255; 16 * 16 * 4])?,
    )?;
    loop {
        while let Some(event) = tray.next_event() {
            println!("{event:?}");
            if matches!(event, Event::Right(..)) {
                return Ok(());
            }
        }
        host::wait();
    }
}
