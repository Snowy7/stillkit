# stillkit-kit

Declarative components for `stillkit::el`. The crate depends only on stillkit;
there are no look-name branches or immediate-mode painting callbacks.

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
Radio and segmented options support Tab + Enter/Space; arrow selection and slider
keyboard stepping are pending the capability below. Components without a callback
are display-only, as specified by today's el semantics. Calendar clamps invalid
month/year inputs; values clamp to 0..=1 (NaN becomes zero). Slider steps must be
positive finite fractions of the range. Calendar has no built-in month navigation;
rebuild with a new month from the host's navigation actions.

Ring and Tooltip are deferred rather than implemented through the old Frame API.
TextFieldFrame is intentionally a frame, not an editor.

## Needed from el

- **Key actions and groups:** public typed key handler or range/group builders.
  Slider needs Left/Down decrement and Right/Up increment, Home/End, an explicit
  step, and accessible min/max/current value. Radio/segmented need roving focus,
  arrows within a group, wrapping, and selected-value semantics. Disabled members
  must be skipped. Current runtime exposes only press/drag builders, so components
  cannot implement this themselves. Percentage values currently end the labels.
- **FocusRing rendering:** el's painter calls the old `Frame::focus_ring`, which
  hardcodes width/gap/opacity. It must read `theme.focus_ring`; no component-level
  API currently permits changing this. The kit leaves runtime ownership intact.
- **Arc primitive:** an element arc/ring with themed stroke width, background track
  and fraction. Required for Ring without importing display-list/Frame painting.
- **Anchored overlays:** anchor key, placement/edge flipping, lifecycle, focus return,
  dismissal and accessible relationships. Needed for Tooltip (hover/focus lifecycle)
  and later Select, Menu, Popover, Dialog, Toast. No overlay substitutes are shipped.
- **Scroll viewport:** clipped content/hits, wheel/keyboard scrolling, focus reveal,
  offset and scrollbar; required before ScrollView and long menus.
- **Text input:** native editing/IME, selection, caret, value/change semantics and
  clipboard. Needed to turn TextFieldFrame into TextField.
- **Non-action semantics:** today's el announces role-only nodes as disabled. Expose
  semantic value/description and non-actionable, enabled roles for progress, labels,
  separators and groups without adding them to keyboard focus order.

## Motion later

Add `.motion(..)` at button/icon fills, switch thumbs, checkbox/radio marks,
slider fill/thumb, segmented selection plate, tile disc, progress fraction and
calendar selection. Retain the part keys above for retargeting. No motion is opted
into here, and no timers, native windows or background work are created by the kit.

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
and destructive label ink and can darken primary fills. Rows and badges stretch to
the parent unless the caller chooses cross-axis alignment or a width. The runtime
currently washes/focuses the slider's whole target rather than only its thumb.
Tooltip and rings are absent pending the public primitives above. Studio is a
composition of existing parts, not a duplicate controls implementation.

The One UI + Liquid material sheets currently match One UI in this glassless
snapshot path: optics are supplied by a host backdrop, not component code. The
Fluent + Mono palette mix changes ink while keeping Fluent geometry and type.
`docs/media/kit/overview.png` records the visual audit of all sixteen sheets.

Validation on Windows: eight tests pass (including all sixteen golden sheets),
clippy is clean for all targets, and `cargo check --target
x86_64-unknown-linux-gnu` passes. The Linux core reports existing dead-code warnings
in its compile-only platform path. All commands used the assigned target directory;
tests used the documented LLVM override after the Cranelift panic exit.
