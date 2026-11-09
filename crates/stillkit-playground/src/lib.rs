#![doc = include_str!("../../../docs/guide/ten-minutes.md")]
//! Named views with model-owned knobs. The browser uses `el::run`, including its script,
//! snapshot and tree CLI. `--goldens DIR` renders each story in every look and scheme.
use anyhow::{Result, ensure};
use std::{path::Path, sync::Arc, time::Duration};
use stillkit::el::inspect::PreviewState;
use stillkit::el::{self, App, Axis, El, Runtime, Tx, col, row, text, token};
use stillkit::{Kind, Scheme, Style, Theme};
use stillkit_kit::{Button, ButtonKind};

/// A named view. Use slash-separated paths to group stories in the browser.
pub struct Story<S: 'static> {
    /// Unique human-readable story path.
    pub name: &'static str,
    /// Builds the story and its controls from model-owned state.
    pub view: fn(&Session<S>) -> El<Session<S>>,
    /// Optional deterministic input before headless capture (for example a tooltip hover).
    pub before_snapshot: Option<fn(&mut Runtime<Session<S>>)>,
}
/// Knobs shared by the browser and deterministic headless rendering.
#[derive(Clone)]
pub struct Knobs {
    /// Base look.
    pub style: Style,
    /// Palette scheme.
    pub dark: bool,
    /// Optional independent material preset.
    pub material: Option<Style>,
    /// Optional independent shape preset.
    pub shape: Option<Style>,
    /// Raster scale.
    pub scale: f32,
    /// Motion override.
    pub reduced: bool,
    /// Disabled story state.
    pub disabled: bool,
    /// Invalid story state.
    pub invalid: bool,
}
impl Default for Knobs {
    fn default() -> Self {
        Self {
            style: Style::Solid,
            dark: true,
            material: None,
            shape: None,
            scale: 1.,
            reduced: false,
            disabled: false,
            invalid: false,
        }
    }
}
impl Knobs {
    /// Resolves mixed parts through stillkit's existing look parser.
    pub fn theme(&self) -> Theme {
        let mut t = Theme::new(
            self.style,
            if self.dark {
                Scheme::Dark
            } else {
                Scheme::Light
            },
            stillkit::theme::SYSTEM_BLUE,
        );
        if let Some(s) = self.material {
            let material = Theme::new(s, t.scheme, t.color.accent).material;
            t = t.material(material);
        }
        if let Some(s) = self.shape {
            let shape = match s {
                Style::Liquid => stillkit::theme::Shape::liquid(),
                Style::OneUi => stillkit::theme::Shape::one_ui(),
                Style::Solid => stillkit::theme::Shape::solid(),
                Style::Fluent => stillkit::theme::Shape::fluent(),
                Style::Mono => stillkit::theme::Shape::mono(),
            };
            t = t.shape(shape);
        }
        t
    }
}
fn preset(s: Style) -> &'static str {
    match s {
        Style::Liquid => "liquid",
        Style::OneUi => "one_ui",
        Style::Solid => "solid",
        Style::Fluent => "fluent",
        Style::Mono => "mono",
    }
}
/// Application state survives look, scale and story changes. Keep story-specific knobs here.
pub struct Session<S: 'static> {
    /// User-owned model.
    pub model: S,
    /// Current environment and state knobs.
    pub knobs: Knobs,
    selected: usize,
    stories: Arc<[Story<S>]>,
}
fn change<S: 'static>(s: &mut Session<S>, tx: &mut Tx, f: impl FnOnce(&mut Knobs)) {
    f(&mut s.knobs);
    tx.set_theme(s.knobs.theme());
    tx.set_scale(s.knobs.scale);
    tx.set_reduced_motion(s.knobs.reduced);
}
fn browser<S: 'static>(s: &Session<S>) -> El<Session<S>> {
    let mut nav = col()
        .w(220.)
        .gap(4.)
        .pad(12.)
        .scroll(Axis::Vertical)
        .h_full()
        .child(text("STILLKIT / STORIES"));
    for (i, story) in s.stories.iter().enumerate() {
        nav = nav.child(
            Button::new(story.name)
                .kind(if i == s.selected {
                    ButtonKind::Tinted
                } else {
                    ButtonKind::Plain
                })
                .key(format!("story-{i}"))
                .on_press(move |s: &mut Session<S>| s.selected = i)
                .build(),
        );
    }
    let mut controls = col()
        .w(240.)
        .pad(12.)
        .gap(5.)
        .scroll(Axis::Vertical)
        .h_full()
        .child(text("LOOK & ENVIRONMENT"));
    for style in Style::ALL {
        controls = controls.child(Button::new(format!("{style:?}")).build().on_press_with(
            move |s: &mut Session<S>, tx| {
                change(s, tx, |k| {
                    k.style = style;
                    k.material = None;
                    k.shape = None;
                })
            },
        ));
    }
    controls = controls.child(text("Material / shape (mix parts)"));
    for style in Style::ALL {
        controls =
            controls.child(
                row()
                    .gap(4.)
                    .child(text(format!("M {style:?}")).on_press_with(
                        move |s: &mut Session<S>, tx| change(s, tx, |k| k.material = Some(style)),
                    ))
                    .child(text(format!("S {style:?}")).on_press_with(
                        move |s: &mut Session<S>, tx| change(s, tx, |k| k.shape = Some(style)),
                    )),
            );
    }
    controls = controls
        .child(
            Button::new("Light / dark")
                .build()
                .on_press_with(|s: &mut Session<S>, tx| change(s, tx, |k| k.dark = !k.dark)),
        )
        .child(
            Button::new(format!("Scale {}x", s.knobs.scale))
                .build()
                .on_press_with(|s: &mut Session<S>, tx| {
                    change(s, tx, |k| {
                        k.scale = if k.scale >= 2. { 1. } else { k.scale + 0.25 }
                    })
                }),
        )
        .child(
            Button::new(format!("Reduced motion: {}", s.knobs.reduced))
                .build()
                .on_press_with(|s: &mut Session<S>, tx| change(s, tx, |k| k.reduced = !k.reduced)),
        )
        .child(text("STATES / subject"));
    for (label, state) in [
        ("Rest", PreviewState::Rest),
        ("Hover", PreviewState::Hover),
        ("Pressed", PreviewState::Pressed),
        ("Focus", PreviewState::Focus),
    ] {
        controls = controls.child(
            Button::new(label)
                .build()
                .on_press_with(move |_: &mut Session<S>, tx| tx.preview_state("subject", state)),
        );
    }
    controls = controls
        .child(
            Button::new(format!("Disabled: {}", s.knobs.disabled))
                .on_press(|s: &mut Session<S>| s.knobs.disabled = !s.knobs.disabled)
                .build(),
        )
        .child(
            Button::new(format!("Invalid: {}", s.knobs.invalid))
                .on_press(|s: &mut Session<S>| s.knobs.invalid = !s.knobs.invalid)
                .build(),
        )
        .child(text("F12: inspect under pointer"));
    row()
        .h_full()
        .fill(token::CARD)
        .child(nav)
        .child(
            col()
                .grow(1.)
                .pad(24.)
                .gap(12.)
                .child(text(s.stories[s.selected].name))
                .child((s.stories[s.selected].view)(s)),
        )
        .child(controls)
}
fn validate<S>(stories: &[Story<S>]) -> Result<()> {
    ensure!(!stories.is_empty(), "provide at least one story");
    for (i, s) in stories.iter().enumerate() {
        ensure!(
            !s.name.is_empty() && !stories[..i].iter().any(|x| slug(x.name) == slug(s.name)),
            "story names need unique nonempty slugs: {}",
            s.name
        );
    }
    Ok(())
}
fn slug(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}
/// Opens the story browser, or writes all stories when `--goldens DIR` is supplied.
pub fn playground<S: Default + 'static>(stories: Vec<Story<S>>) -> Result<()> {
    validate(&stories)?;
    let args: Vec<_> = std::env::args().collect();
    if let Some(i) = args
        .iter()
        .position(|a| a == "--goldens" || a == "--check-goldens")
    {
        let dir = args
            .get(i + 1)
            .ok_or_else(|| anyhow::anyhow!("--goldens needs an output directory"))?;
        render_matrix(&stories, Path::new(dir), args[i] == "--check-goldens")?;
        return Ok(());
    }
    let knobs = Knobs::default();
    let theme = knobs.theme();
    el::run(App {
        title: "stillkit playground",
        size: (1180., 780.),
        kind: Kind::App { min: (800, 500) },
        theme,
        state: Session {
            model: S::default(),
            knobs,
            selected: 0,
            stories: stories.into(),
        },
        view: browser,
    })
}
/// Renders every story in five looks and two schemes with fresh state and a fixed clock.
/// Existing PNGs are replaced explicitly; this does not bless the repository's goldens.
pub fn render_all<S: Default + 'static>(
    stories: &[Story<S>],
    dir: &Path,
) -> Result<Vec<std::path::PathBuf>> {
    render_matrix(stories, dir, false)
}
/// Compares every rendered PNG with an existing baseline without modifying it.
pub fn check_goldens<S: Default + 'static>(
    stories: &[Story<S>],
    dir: &Path,
) -> Result<Vec<std::path::PathBuf>> {
    render_matrix(stories, dir, true)
}
fn render_matrix<S: Default + 'static>(
    stories: &[Story<S>],
    dir: &Path,
    check: bool,
) -> Result<Vec<std::path::PathBuf>> {
    validate(stories)?;
    if !check {
        std::fs::create_dir_all(dir)?;
    }
    let mut paths = Vec::new();
    for story in stories {
        for style in Style::ALL {
            for dark in [false, true] {
                let knobs = Knobs {
                    style,
                    dark,
                    reduced: true,
                    ..Knobs::default()
                };
                let theme = knobs.theme();
                let view = story.view;
                let mut rt = Runtime::new(
                    Session {
                        model: S::default(),
                        knobs,
                        selected: 0,
                        stories: Arc::from([]),
                    },
                    move |s| col().pad(24.).fill(token::CARD).h_full().child(view(s)),
                    theme,
                    (640, 480),
                    1.,
                )?;
                rt.set_reduced_motion(true);
                rt.frame(Duration::ZERO);
                if let Some(prepare) = story.before_snapshot {
                    prepare(&mut rt);
                }
                let image = rt.snap()?;
                let path = dir.join(format!(
                    "{}-{}-{}.png",
                    slug(story.name),
                    preset(style),
                    if dark { "dark" } else { "light" }
                ));
                let bytes = encode_png(&image)?;
                if check {
                    ensure!(
                        std::fs::read(&path)? == bytes,
                        "story golden differs: {}",
                        path.display()
                    );
                } else {
                    std::fs::write(&path, bytes)?;
                }
                paths.push(path);
            }
        }
    }
    Ok(paths)
}
fn encode_png(image: &stillkit::Image) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let pixels: Vec<_> = image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            let a = u32::from(p[3]);
            let straight =
                |c: u8| ((u32::from(c) * 255 + a / 2).checked_div(a).unwrap_or(0)).min(255) as u8;
            [straight(p[2]), straight(p[1]), straight(p[0]), p[3]]
        })
        .collect();
    encoder.write_header()?.write_image_data(&pixels)?;
    Ok(bytes)
}
/// Built-in stories for the kit's component families.
pub mod kit;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_kit_story_renders_png_in_each_look_and_scheme() -> Result<()> {
        let root = std::env::var_os("CARGO_TARGET_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("target"));
        let dir = root.join(format!("story-test-{}", std::process::id()));
        let stories = kit::stories();
        let paths = render_all(&stories, &dir)?;
        assert_eq!(paths.len(), stories.len() * 10);
        check_goldens(&stories[..1], &dir)?;
        std::fs::write(&paths[0], b"changed")?;
        assert!(check_goldens(&stories[..1], &dir).is_err());
        // Restore the explicitly generated baseline before decoding all files.
        render_all(&stories[..1], &dir)?;
        for path in paths {
            let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
            let mut reader = decoder.read_info()?;
            assert_eq!((reader.info().width, reader.info().height), (640, 480));
            let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
            reader.next_frame(&mut pixels)?;
            assert!(pixels.iter().any(|v| *v != 0));
        }
        Ok(())
    }
    #[test]
    fn look_changes_keep_model_and_mix_parts() {
        let knobs = Knobs {
            style: Style::Mono,
            material: Some(Style::Liquid),
            shape: Some(Style::OneUi),
            ..Knobs::default()
        };
        let theme = knobs.theme();
        assert!(theme.glass.is_some());
        assert_eq!(theme.radius.surface, 28.);
        assert_eq!(theme.radius.control, None);
        assert_eq!(
            theme.metrics.pad,
            Theme::mono(Scheme::Dark, stillkit::theme::SYSTEM_BLUE)
                .metrics
                .pad
        );
        let mut rt = Runtime::new(7, |n| text(n.to_string()), theme, (200, 100), 1.).unwrap();
        rt.set_scale(2.);
        rt.set_reduced_motion(true);
        rt.set_theme(Knobs::default().theme());
        rt.frame(Duration::ZERO);
        assert_eq!(*rt.state(), 7);
    }
}
