//! A tray-only app: no app window, timer or background thread.
use stillkit::{
    platform::current::host,
    tray::{Event, Icon, Tray},
};
fn main() -> anyhow::Result<()> {
    let Some(mut instance)=stillkit::instance::Instance::claim("stillkit.services",std::env::args_os().collect())? else { return Ok(()) };
    let mut tray = Tray::new(
        "stillkit.services",
        "stillkit: right-click to exit",
        Icon::rgba(16, 16, vec![255; 16 * 16 * 4])?,
    )?;
    let mut hotkey=stillkit::hotkey::Hotkey::new("CTRL+ALT+K".parse()?, "Print an activation")?;
    let mut notifications=stillkit::notify::Notifications::new("stillkit.services", Icon::rgba(16,16,vec![255;16*16*4])?)?;
    loop {
        if hotkey.activated() { notifications.show("stillkit", "Global hotkey activated")?; }
        while let Some(event) = tray.next_event() {
            println!("{event:?}");
            if matches!(event, Event::Right(..)) {
                return Ok(());
            }
        }
        while let Some(action)=notifications.next_action() { println!("{action:?}"); }
        while let Some(args)=instance.next_args() { println!("activation: {args:?}"); }
        host::wait();
    }
}
