//! A tray app with optional --hotkey, --notify and --pick demonstrations.
use stillkit::{
    platform::current::host,
    tray::{Event, Icon, Tray},
};
fn main() -> anyhow::Result<()> {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    if has("--pick") {
        println!(
            "{:?}",
            stillkit::dialog::pick_files("Choose files", None, true)
        );
        return Ok(());
    }
    let Some(mut instance) =
        stillkit::instance::Instance::claim("stillkit.services", args.clone())?
    else {
        return Ok(());
    };
    let icon = Icon::rgba(16, 16, vec![255; 16 * 16 * 4])?;
    let mut tray = Tray::new(
        "stillkit.services",
        "stillkit: right-click for menu",
        icon.clone(),
    )?;
    let mut hotkey = if has("--hotkey") {
        Some(stillkit::hotkey::Hotkey::new(
            "CTRL+ALT+K".parse()?,
            "Print an activation",
        )?)
    } else {
        None
    };
    let mut notifications = if has("--notify") {
        let mut n = stillkit::notify::Notifications::new("stillkit.services", icon)?;
        n.show("stillkit", "Click to deliver an action event")?;
        Some(n)
    } else {
        None
    };
    let mut popup: Option<(stillkit::Ui, stillkit::Menu<()>)> = None;
    loop {
        while hotkey.as_mut().is_some_and(|h| h.activated()) {
            println!("global hotkey activated");
        }
        while let Some(event) = tray.next_event() {
            println!("{event:?}");
            if matches!(event, Event::Right(..)) {
                if popup.is_none() {
                    popup = Some((stillkit::Ui::new()?, stillkit::Menu::new("Services")?));
                }
                let (ui, menu) = popup.as_mut().unwrap();
                tray.open_menu(
                    event,
                    menu,
                    ui,
                    vec![stillkit::Entry::action("Quit", ())],
                    &stillkit::Tokens::default(),
                )?;
            }
        }
        while let Some((id, event)) = stillkit::next_event() {
            if let Some((ui, menu)) = &mut popup
                && matches!(
                    menu.handle(ui, id, &event),
                    Some(stillkit::Outcome::Chosen(()))
                )
            {
                return Ok(());
            }
        }
        if let Some(n) = &mut notifications {
            while let Some(action) = n.next_action() {
                println!("{action:?}");
            }
        }
        while let Some(args) = instance.next_args() {
            println!("activation: {args:?}");
        }
        host::wait();
    }
}
