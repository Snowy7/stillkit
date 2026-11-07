use std::time::Duration;
use stillkit::el::{Runtime, Script, col, text};
use stillkit::{Scheme, Theme};
fn main() -> anyhow::Result<()> {
    let mut rt = Runtime::new(
        0u32,
        |count| {
            col().pad(20.).child(
                text(format!("Count: {count}"))
                    .key("count")
                    .on_press(|count: &mut u32| *count += 1),
            )
        },
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (320, 180),
        1.,
    )?;
    rt.frame(Duration::ZERO);
    rt.play(&Script::parse(r#"[{"at":0.1,"click":"count"}]"#)?, None);
    assert!(rt.find("count").is_some());
    assert_eq!(*rt.state(), 1);
    Ok(())
}
