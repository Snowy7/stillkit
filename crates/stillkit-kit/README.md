# stillkit-kit

Declarative components for `stillkit::el`. The crate depends only on stillkit;
there are no look-name branches. Charts use public chart drawing inside el paint nodes;
all other components use el primitives.

```rust
use stillkit_kit::{Button, ButtonKind, Icon};
struct Model { saved: bool }
let button = Button::new("Save").key("save")
    .kind(ButtonKind::Filled).icon(Icon::Check)
    .on_press(|s: &mut Model| s.saved = true).build();
```

Model values are controlled: rebuild switches, sliders and selection groups from
the current state in the view. Callbacks run between frames through Runtime.
Named parts use caller keys (`save/label`, `volume/track`, `mode/option-1`).
Unkeyed components keep normal positional el identity. All builders return `El<S>`;
layout and additional semantics can be applied after `build()`.

## Components

Implemented: Button (all five kinds), IconButton, Switch, Checkbox, RadioGroup,
Slider (thin and icon bar), Segmented, Progress, Badge, ListRow (arbitrary lead and
accessory elements), SectionHeader, Tile, Card, Separator, EmptyState,
CalendarMonth (Gregorian, Monday first), TextFieldFrame (value/placeholder/focus).

Selection and pointer/keyboard activation use runtime checked, disabled, press,
hover and focus state. Disabled controls ignore input and dim the whole subtree.
Radio and segmented options have one Tab stop and wrapping arrow selection.
Sliders expose ranges with arrow/Page/Home/End and assistive stepping. Components without a callback
are display-only, as specified by today's el semantics. Calendar clamps invalid
month/year inputs; values clamp to 0..=1 (NaN becomes zero). Slider steps must be
positive finite fractions of the range. Calendar exposes `on_month_change` with
a (year, month) pair; rebuild from that value. `today(day)` is supplied by the host only for the current month.

Ring uses the public arc primitive. Overlay wrappers are a separate component family.
TextFieldFrame is intentionally a frame, not an editor.

## Needed from el

- **Interaction wash motion:** connect el's retained hover/press painting to the
  existing interaction animator and theme interactive token (including reduced
  motion and idle disarming). `Frame::wash` supports it, but el does not supply it;
  `.motion` cannot observe hover/press state from a component view.
- **Absolute percentage sizing:** nested `.absolute(0., 0.).w_full()` currently
  resolves against the headless root width instead of its padded parent. The
  segmented pill uses a zero-height flex rail to avoid this; no el workaround was
  added outside the kit.
- **Text input:** native editing/IME, selection, caret, value/change semantics and
  clipboard. Needed to turn TextFieldFrame into TextField.
- **Semantic descendants and pointer hits:** non-actionable cell hits currently
  intercept their actionable row. Table announces the combined row values and omits
  separate cell hits until hit testing walks through non-actionable descendants.
- **Visibility-aware continuous motion:** el has no viewport-visibility callback,
  rotation transform or continuous animation channel. Spinner therefore requires a
  controlled `phase`, `on_tick` and `visible(false)` when hidden (including clipped
  or inactive host content). It uses four arc start positions, stops its one-shot
  timeout when hidden/removed, and stays static under reduced motion. A host/runtime
  override can be passed with `reduced_motion(bool)`; the default follows the OS.
- **Tree focus targeting:** el has focus groups but no public focus-by-key request.
  Tree supports Up/Down/Home/End and Left/Right collapse/expand on branches; moving
  Left from a child to its parent, or Right from an open branch into its first child,
  needs that request. Nodes expose expanded/selected semantics today.
- **Rotation:** disclosure chevrons swap direction and spring their occupied width;
  a true animated rotation needs a transform channel. Height and presence animate.

## Motion

Interactive theme springs drive switch tracks/thumbs, checkbox checks, radio dots,
slider fill/thumb, the single sliding segmented pill, tile discs, progress and
calendar selection. Badge text enters/exits with a short slide/fade while the pill
resizes. Stable part keys retain velocity when a value changes mid-flight. Runtime
owns the clock: first frames are static, reduced motion follows the host setting,
and drag updates land instantly while external slider updates spring. Spinner uses
an el-owned timeout only while explicitly visible; no native windows or background
workers are created by the kit.

Hover/press washes still snap: see Needed from el. Opting a button into `.motion`
only animates its element styles, not the runtime-owned interaction wash.

## Gallery and checks

`cargo run -p stillkit-kit --example kit_gallery` writes Runtime::snap PNGs to
`docs/media/kit/`. Six columns cover rest, hover, press, focus-visible, disabled,
and checked/full values. Static components repeat unchanged across state columns.
Five presets, Studio monochrome (Solid geometry + Mono palette), and two requested
mixes each appear in light and dark (16 sheets). Each cell uses its own headless
Runtime so simultaneous hover/press/focus states are real runtime inputs.

`cargo test -p stillkit-kit` compares those sheets on Windows and exercises
activation, toggles, drag quantization/clamping, selection, disabled input and focus
order. Non-Windows currently has a compile-only renderer, so raster tests are
Windows-only. Set `CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm` on Windows if Cranelift
terminates at a panic. Golden updates use the existing `STILLKIT_BLESS=1` workflow.

Contrast resolves against each component's own composited surface. Neutral label
ink is adjusted across the neutral surfaces; button emphasis has its own surface
and ink pair. A mid-luminance accent may need a darker filled-button surface to
make 7:1 possible. Disabled content is intentionally dimmed and excluded from the
contrast floor. Glass checks use the theme's opaque surface fallback;
real hosts remain responsible for adaptive schemes over arbitrary wallpapers.

## Comparison with the immediate-mode kit

Verified visually against `docs/media/native/kit-liquid-dark.png` and
`docs/media/native/kit-oneui-light.png`. Type, vector
icons, 40x24 switch, 18px slider thumb, 28px icon bar, capsule buttons and concentric
cards read as the same family. Deliberate differences: neutral fallback backgrounds
replace the native gallery's wallpaper/glass stand-in; all presets use an accent
icon bar (the old kit chose white for dark schemes); AAA text shifts tinted/plain
and destructive label ink and can darken primary fills. Rows stretch to the parent;
badges now keep an intrinsic compact pill.
The gallery intentionally stretches buttons so state columns remain comparable.
Segment labels use the kit's 7:1/4.5:1 ink floors; adjacent-month calendar dates
are shown in secondary ink, and the calendar uses Monday-first Gregorian dates.
The runtime currently washes/focuses the slider's whole target rather than only its thumb.
Rings now use the themed arc stroke and percentage labels. Studio is a
composition of existing parts, not a duplicate controls implementation.

The One UI + Liquid material sheets currently match One UI in this glassless
snapshot path: optics are supplied by a host backdrop, not component code. The
Fluent + Mono palette mix changes ink while keeping Fluent geometry and type.
`docs/media/kit/overview.png` records the visual audit of all sixteen sheets.

Validation on Windows: twenty-four tests pass (including sixteen original and
 twelve layout golden sheets),
clippy is clean for all targets, and `cargo check --target
x86_64-unknown-linux-gnu` passes. The Linux core reports existing dead-code warnings
in its compile-only platform path. All commands used the assigned target directory;
tests used the documented LLVM override.

Motion goldens: `motion-{switch,segmented,checkbox}.png`, at 0.1, 0.15, 0.2,
0.3, 0.45 and 1.5 seconds after a scripted click at 0.1. Tests call `Runtime::play`
with an explicit `until`, the same path used by `--script` + `--at`. They also
check first-frame/idle behavior, pill bounds, intermediate travel, reduced motion,
instant updates and slider drag snapping. Static sheets settle interaction input
before capture. Keyboard stepping and rings are covered by layout tests;
no files under `crates/stillkit` were changed.

## Layout and data

Implemented: Tabs with panels and a sliding indicator; Disclosure (also exported as
Accordion); Stepper; Ring; controlled Spinner; static Skeleton; Avatar (monogram or
image, size and status); ScrollView; Table; Tree; Sidebar; Toolbar; Chart and QuotaBar.
Every interactive component uses runtime keyboard activation and semantic values.
Static charts, skeletons and avatars stay out of the focus order.

Values, expanded state, selection and sort order live in your model. Tab indices
clamp to available panels. Supply stable keys on each built component. Table rows
and tree nodes carry their own keys; `on_sort` asks the host to reorder its data.
The table header is a fixed sibling above the scrolling body, so it remains visible
without a sticky-position primitive. Columns share width. Sidebar groups compose
Disclosure; Toolbar overflow can be composed with the overlay family rather than
adding a second menu implementation. Accordion sections are independently controlled.

```rust
use stillkit::el::text;
use stillkit_kit::Tabs;
struct Model { page: usize }
let tabs = Tabs::new("Workspace", 0)
    .tab("Overview", text("Overview panel"))
    .tab("Activity", text("Activity panel"))
    .on_change(|s: &mut Model, page| s.page = page)
    .build().key("workspace-tabs");
```

`cargo run -p stillkit-kit --example kit_layout` writes twelve
`docs/media/kit/layout-<look>-<scheme>.png` sheets: all five presets and a Fluent +
Mono mix in both schemes. `tests/layout.rs` covers keyboard tab switching,
disclosure expansion/height motion, rapid range stepping and assistive actions,
radio/segmented arrows, table sort/selection/fixed headers, tree keys, sidebar
selection, spinner shutdown, meter values, and these golden sheets.

The area chart retains the old kit's smooth interpolation, overlaid translucent
areas and labelled grid. The quota retains its hatch and pace marker, with el text,
a theme-aware avatar and reset badge replacing the old hardcoded dark chip ink.
Controls follow each preset's typography, geometry and palette; neutral headless
surfaces replace native backdrop glass. Rings use token::RING rather than fixed
stroke weights. Disclosure uses a width spring rather than a rotation, and spinner
rotation is stepped pending the el capabilities above.
