# Native UI: `crates/ui` and `mypaper-shell`

> Written inside mypaper, where this crate was `crates/ui`. The parts on `mypaper-shell` (bars,
> flyouts, the desktop menu, shell measurements) describe how mypaper uses the library; they move
> to mypaper when the docs are split by layer.

The native rebuild (`plans/council/native-plan.md`) draws every shell surface (menus, the top
bar, dock, flyouts and widgets) with one small UI layer, `crates/ui` (`mypaper-ui`), hosted by
one process, `mypaper-shell`. This page is how both work, and what they measured. The flyouts
themselves are in [bars.md](bars.md#flyouts-n3).

## Rules it follows

- **No hardware GPU device in the shell** (plan Rule 1). A process with one pays about 240 MB
  private for the vendor driver stack here. The UI draws with **software Direct2D** into a **WIC
  bitmap** (`D2D1_RENDER_TARGET_TYPE_SOFTWARE`), and text with **DirectWrite**. WARP's DLLs
  (`d3d11`, `dxgi`, `d3d10warp`) are allowed; vendor drivers (`nv*umx`, `nvgpucomp64`, `amdxc64`,
  `atidx*`) never load. A test checks it (`crates/ui/tests/no_gpu_driver.rs`), and the shell logs an
  error if one ever does.
- **If nothing changes, nothing is drawn** (plan Rule 2). A surface redraws only the bounds of
  the paint operations that changed, and an identical frame costs a list comparison. The frame
  ticker runs only while something moves. The shell's loop sleeps in
  `MsgWaitForMultipleObjectsEx` until a message, a scheduled widget tick, or its supervisor's exit wakes it.

## A frame

```text
view(&model, &tokens) ──► DisplayList { ops, hits, focus }
                                   │
Surface::present(ui, origin, size, list)
  1. damage = bounds of the ops that differ from the list on screen   (none: stop)
  2. software Direct2D redraws that rectangle only (clip, clear, ops that touch it)
  3. the rectangle is copied from the WIC bitmap into the window's DIB section
  4. UpdateLayeredWindowIndirect with prcDirty = that rectangle
  5. the accessibility tree follows the hits (only once a client has asked)
```

- **Ops** (`Op`): `Fill`, `Stroke` (rounded or not), `Line`, `Text` (a string or an icon glyph,
  placed by its measured size), `Icon` (a vector icon), `Shadow` (stepped rings), `Arc`, `Image`,
  `TextField`, `Clip`/`Unclip` and `Clipped` (a retained clipped group: scroll views).
- **Hits** (`Hit`): rectangle, id, AccessKit role, label, enabled, checked. Input hit-tests them,
  keyboard focus steps over them (`focus::step`), and assistive technology sees them.
- **Damage** is computed op by op in list order: a changed clock redraws the clock; a moved fill
  redraws where it was and where it is.
- **Text** goes through a cache of `IDWriteTextFormat` and `IDWriteTextLayout`: a label is shaped
  once and redrawn from its layout. Text is grayscale-antialiased with fixed rendering parameters
  (layered windows have alpha, so no ClearType; and golden images must not depend on the machine's
  ClearType tuning). See [The kit › Text](#text) for the parameters and the variable font.
- **Icons**: views name an `icons::Icon`; the theme draws it as a vector or a font glyph. See
  [The kit › Icons](#icons). `icons::centered(text, family, icon, size, rect, color) -> Op` (a
  font glyph) is the older API, kept while surfaces move to `Frame::icon`.
- **Tokens**: [`Theme`](#theme) is the source of truth. `Tokens` (menus) comes from
  `theme.tokens()`; `Colors`, `Ink`, `Step` and `Typeface` below are the older API, kept until
  their users are ported.
- **Type and contrast** shared by the bar, flyouts and widgets: `Typeface` (`system(text)` picks
  Segoe UI Variable Text for small sizes and Display above 20 px; `single(family)` for a theme's
  font) with `style(Step, scale)` or `sized(size, weight, scale)`, where `Step` is the ramp
  (`Micro` 11/500, `Caption` 12/400, `Body` 13/400, `Label` 13/600, `Headline` 15/600, `Title`
  20/600, `Large` 28/500). `Ink::of(text)` gives the contrast steps: `primary`, `secondary`
  (74%), `tertiary` (56%) and `disabled` (34%), the old `--t0..--t3` with the lower two raised.

## The kit

Every surface (bars, flyouts, menus, widgets, Studio) is being rebuilt on one kit:
`mypaper_ui::{theme, kit, layout, scroll, icons}`. A view makes a [`Theme`](#theme), builds its
list with a [`Frame`](#frame-and-components), places things with [layout](#layout), and scrolls
lists with a [`Scroll`](#scroll-view). The gallery renders every component in every style:

```text
cargo run --release -p mypaper-ui --example gallery     # writes docs/media/native/kit-*.png
```

`kit-<style>-<scheme>.png` (liquid, oneui, solid, fluent, mono × dark, light) shows Control
Center, a scrolled notification list, buttons, a segmented control, a field, a slider, list rows,
rings, badges, a tooltip and a menu over a wallpaper with a stand-in for the engine's glass;
`kit-text.png` is the text lab and `kit-icons.png` the icon comparison. All are drawn at 1x and
saved at 2x without smoothing, to show real pixels.

![liquid, dark](media/native/kit-liquid-dark.png)

### Theme

`Theme::new(style, scheme, accent)`, with `Style::{Liquid (default), OneUi, Solid, Fluent, Mono}`,
`Scheme::{Dark, Light}`, and the Windows accent (`theme::SYSTEM_BLUE`, iOS 26's `#0088FF`, when
there is none). Values are design px; `Frame` scales them.

| Field | What |
|---|---|
| `color: Palette` | `label`, `secondary`, `tertiary`, `quaternary` (disabled); `fill` (a control's rest: an off switch, a track, a gray button), `card` (cards, tiles), `well` (fields, a segmented track); `separator`; `accent`, `on_accent`, `accent_wash`; `destructive`, `success`, `warning`; `surface` (a glassless body, and the glass tint), `elevated` (tooltips); `hover`, `pressed` (washes); `knob`, `scrollbar`, `shadow` |
| `font: TypeRamp` | `style(TextRole, s) -> TextStyle`, `line(role, s)` |
| `metrics` | `pad` (panel inset), `gap` (between blocks), `item`, `row` (two lines), `row_single`, `control`, `control_small`, `icon`, `icon_small`, `icon_large`, `hit`, `scrollbar`, `scrollbar_inset` |
| `radius` | `surface`; `card` = surface − pad (concentric, `Radii::concentric(outer, inset, min)`); `control: Option<f32>` (`None`: capsules); `small` |
| `motion` | Apple springs (`Spring::apple(duration, bounce)`): `interactive` (0.15, 0.14), `open` (0.3, 0.12), `layout` (`.snappy`), `scroll` (0.22, 0); `scrollbar_idle` 0.9 s, `scrollbar_fade` 0.35 s |
| `icons`, `icon_weight` | `IconSet::Vector` (Fluent: `IconSet::Font`), stroke per 24 units |
| `glass`, `border` | engine glass material (`None` for Solid and Mono); Solid's one hairline |

- **Contrast on glass.** The label steps sit above Apple's (whose vibrancy blend a layered window
  can't do): dark 100/72/50/30 %, light 88/78/60/34 % (Fluent uses Windows 11's text steps). A test
  holds every style and scheme to label 7:1 (the AAA target liquid-rust uses on large light
  glass), secondary 4.5:1 (also on cards), tertiary 3:1 and accent 3:1 over its own surface; the
  accent is lightened or darkened until it reaches 3:1.
- **Adaptive light/dark** (liquid-rust's rule from Apple's kit): `theme.adaptive(min_side,
  backdrop_luminance) -> Scheme`. Glass smaller than about 180 pt (buttons, a bar, a small widget)
  takes the face that reads over what is behind it (flip at WCAG luminance 0.179); panels and
  menus follow the scheme. `theme.with_scheme(scheme)` gives the flipped theme. The engine has to
  report the backdrop's luminance for this to switch live; until then the scheme rules.
- `theme.tokens()` gives menus their `Tokens`; `theme.ink()` the older `Ink`;
  `theme::{luminance, contrast, over}` are the WCAG helpers the tests use.

| `TextRole` | liquid, solid, mono | oneui | fluent |
|---|---|---|---|
| `LargeTitle` | 28 / 700, −0.6 | 30 / 700 | 28 / 600 |
| `Title1` | 22 / 700, −0.4 | 23 / 700 | 20 / 600 |
| `Title2` (panel titles) | 17 / 600, −0.2 | 18 / 700 | 16 / 600 |
| `Title3` | 15 / 600 | 16 / 700 | 14 / 600 |
| `Headline` (row titles) | 14 / 600 | 15 / 600 | 14 / 600 |
| `Body` | 14 / 400 | 15 / 400 | 14 / 400 |
| `Callout` (subtitles) | 13 / 400 | 13 / 400 | 13 / 400 |
| `Subheadline` (section headers) | 12 / 600 | 12 / 600 | 12 / 600 |
| `Footnote` (timestamps) | 12 / 400 | 12 / 400 | 12 / 400 |
| `Caption` (badges) | 11 / 500 | 11 / 600 | 11 / 400 |

Size / weight, tracking in px. Body is 14, not macOS's 13: grayscale text at 96 DPI needs the
extra pixel.

### Text

- **Segoe UI Variable through its axes.** The ramp's family is `VARIABLE` ("Segoe UI Variable"):
  `IDWriteFactory6::CreateTextFormat` with `wght` = the weight (500 is a real medium, not 400 or
  600) and `opsz` = the design size in points, as Windows 11 draws it. `TextStyle` gained
  `tracking` (device px, applied with `SetCharacterSpacing`) and `optical` (points; 0 takes it from
  the size). Other families keep their named faces.
- **Rasterization** (`TextQuality`, `Ui::set_text_quality`): grayscale, gamma 1.8, grayscale
  enhanced contrast 1.0, natural symmetric, vertical grid fit. The old parameters put contrast 0.5
  in the ClearType-only field of a v1 `IDWriteRenderingParams`, so grayscale text got DirectWrite's
  weakest coverage: light text on glass read thin and grey. `kit-text.png` shows before, after,
  contrast 1.5, gamma 2.2 and GDI classic on dark and light glass; 1.0 is Windows' own grayscale
  default and was the clearest without bloating. Colour fonts are on
  (`D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT`): emoji in notifications draw in colour.
- **Pixels.** Components put text on whole pixels and centre it in the role's line height.

### Icons

`Icon` names (media, chevrons, commands, shell, status with levels, weather): `Play`, `Pause`,
`Previous`, `Next`, `Shuffle`, `Repeat`, `ChevronLeft/Right/Up/Down`, `Add`, `Remove`, `Close`,
`Check`, `More`, `Settings`, `Search`, `Edit`, `Delete`, `Copy`, `Refresh`, `Pin`, `Grip`,
`External`, `Download`, `Upload`, `Apps`, `Window`, `Power`, `Restart`, `SignOut`, `Lock`, `Bell`,
`BellOff`, `Calendar`, `Clock`, `Timer`, `Globe`, `Keyboard`, `Desktop`, `Home`, `User`, `Folder`,
`File`, `Clipboard`, `Terminal`, `Message`, `Controls`, `Layers`, `Volume(0..=3)`, `Mute`,
`Speaker`, `Headphones`, `Mic`, `Wifi(0..=3)`, `Ethernet`, `Offline`, `Hotspot`, `Cast`,
`Airplane`, `Location`, `Bluetooth`, `Brightness`, `Moon`, `NightLight`, `Focus`, `Saver`, `Gauge`,
`Thermometer`, `Cpu`, `Battery(0..=10)`, `Charging`, `BatteryWarning`, `Plug`, `Accessibility`,
`Eye`, `Camera`, `Info`, `Warning`, `Star`, `Heart`, `Notes`, `Tasks`, `Photo`, `Feed`, `Sun`,
`Cloud`, `CloudSun`, `CloudMoon`, `Rain`, `Drizzle`, `Snow`, `Snowflake`, `Thunder`, `Fog`,
`Wind`, `Droplet`, `Umbrella`, `Sunrise`, `Sunset`.

- **Vectors** (every style but Fluent): Lucide's outlines (ISC licence,
  `crates/ui/icons/LICENSE-lucide`), compiled in as SVG path data (`src/icon_paths.rs`), parsed
  once into Direct2D geometry per canvas and drawn as `Op::Icon { rect, icon, weight, color }`:
  round caps and joins, 1.75 units per 24 (SF Symbols' calm weight), never under 1.35 px. Media
  glyphs are filled as well, like SF's `.fill`. Chosen over the font after rendering both
  (`kit-icons.png`): Lucide reads like SF Symbols and covers weather; at 13 px the hinted font is
  a little crisper, so the kit draws row and button glyphs at 14 px and up.
- **Font** (Fluent): Segoe Fluent Icons glyphs (`Icon::font_glyph`); icons the font lacks are
  vectors there too.
- **Adding an icon:** add its Lucide name to `crates/ui/icons/gen.mjs`, run
  `bun crates/ui/icons/gen.mjs <lucide-static>/icons`, then add the variant and its `lucide()` arm.

### Frame and components

`Frame::new(&mut ui.text, &theme, s, &interaction)` builds one `DisplayList` (`f.list`). Lengths
in calls are **device px** (`f.px(design)` converts); `f.font(role)`, `f.line(role)`.

| Call | Draws |
|---|---|
| `surface(rect, glass_available) -> Option<Glass>` | the body, first: with glass only an invisible hit fill, and the `Glass` to hand to `Surface::set_glass`; without, the surface fill (and Solid's hairline) |
| `card(rect) -> inner` | the card fill at the concentric radius; returns the rect inset by `pad` |
| `separator(left, right, y)` | a hairline |
| `label(x, y, text, role, color, max)`, `text_in(rect, text, role, color, Align)`, `paragraph(x, y, w, text, role, color, lines) -> h`, `line_count(..)` | text, cut with an ellipsis, wrapped on the role's line height |
| `icon(rect, Icon, size, color)` | the theme's icon, centred |
| `button(id, rect, label, Option<Icon>, ButtonKind, enabled)`, `button_size(label, icon)` | `Filled` (the one primary), `Tinted`, `Gray`, `Plain`, `Destructive` |
| `icon_button(id, rect, Icon, size, label, filled)` | a round button; `filled` keeps the control fill at rest (media) |
| `switch(id, right, center_y, on, label, enabled) -> Rect` | an on/off switch, knob with a soft shadow |
| `slider(id, rect, value, label, Option<Icon>)` | with an icon, Control Center's thick bar; without, a thin track and knob. The hit's label ends in the percent |
| `segmented(base, rect, labels, selected)` | segment `i` is hit `base + i` |
| `row(&Row, rect)`, `row_height(&Row)` | `Row { id, lead: Lead::{None, Icon, Badge(icon, on), Image, Monogram}, title, subtitle, accessory: Accessory::{None, Chevron, Check, Text, Switch(on), Button(id, icon, label)}, selected, enabled }` |
| `section_header(rect, title, Option<(id, action)>)` | a small title and a plain action |
| `tile(&Tile, rect)` | `Tile { id, icon, title, subtitle, on, enabled, more: Option<id> }`: Control Center's toggle tile |
| `progress(rect, value, color)`, `ring(center, radius, width, value, color)`, `badge(x, center_y, text, color) -> w` | meters and counts |
| `tooltip(anchor, text, bounds) -> Rect` | on the elevated fill, below or above its anchor |
| `field_frame(rect, placeholder, empty, focused, icon) -> inner` | a field's well, placeholder and focus ring; the `Op::TextField` goes in `inner` |
| `empty(rect, icon, title, detail) -> h` | a centred message where a list would be |
| `scroll(viewport, &scroll, now, \|f, origin\| -> content_height)` | see [Scroll view](#scroll-view) |
| `wash(id, rect, radius, enabled)`, `focus_ring(id, rect, radius)`, `hit(..)`, `fill(..)`, `push(op)` | the parts, for custom controls |

- **States** come from `interact::Interaction` (`hover`, `pressed`, `focus`, `focus_visible`).
  Each control's wash and focus ring are always in the list, transparent at rest: a hover changes
  one op and never shifts the rest of the list (a test checks it), so damage stays one control.
- **No outlines.** Nothing draws a border except Solid's surface hairline. Focus rings appear
  only after the keyboard moved focus (`focus_visible`), never on a click (tested).
- Hit id `kit::NONE` (0) draws without a hit (decoration).

### Layout

- `Rect::take_top/take_bottom/take_left/take_right(size, gap) -> (piece, rest)`,
  `inset(Insets::all(v) | Insets::xy(x, y))`, `place(w, h, Align, Align)` (whole pixels),
  `intersect`, `center`.
- `Stack::column(rect, gap)` / `Stack::row`: `next(size) -> Rect`, `space(extra)`, `rest()`,
  `used()` (the content's extent).
- `layout::split(rect, Axis, gap, &[Length::Fixed(px) | Length::Fill(weight)], &mut out)`,
  `layout::grid(rect, columns, gap, row_height, index) -> Rect`.
- `SizeClass::of(width, height)`: `Compact` (< 150 design px on the smaller side), `Regular`
  (< 300), `Large`, so widgets switch layouts by size.

### Scroll view

`Scroll` is the state, kept by the panel: `Scroll::new(&theme.motion)`.

- `f.scroll(viewport, &scroll, now, |f, origin| { ...draw from origin...; content_height })`
  draws the content as one `Op::Clipped` group in the viewport, cuts its hits to the viewport and
  drops those outside, and draws the overlay scrollbar inside the viewport's right edge (always
  in the list; transparent when hidden). Headers and footers are drawn outside the viewport and
  never move.
- **Give the viewport a height from the content, never from the offset**: `min(content, room)`.
  That is what keeps a flyout's size while it scrolls.
- `scroll.wheel(delta, s, now) -> changed`: a delta that is not a multiple of 120 (a precision
  touchpad) moves exactly that much at once; a notch glides 48 design px along `motion.scroll`.
  `page(down, now)` (Page Up/Down), `reveal(top, bottom, now)` (keyboard focus), `reset()`.
- `scroll.tick(now) -> (changed, next)`: call it from the panel's tick; it asks for frames only
  while gliding or fading and `None` at rest (Rule 2).

### Glass contract

- A surface on glass draws **no outline** and no tint over the glass: `f.surface` fills it with
  `Rgba::HIT` (clicks) and returns `Glass { body, radius, material }`. Pass exactly that to
  `Surface::set_glass`: the engine's plate or overlay uses the same rect and radius
  (`px(theme.radius.surface)`), and draws the one border there is (its rim).
- Cards and controls on glass are fills (`card`, `fill`, `well`), never strokes.
- Corners are circular today (Direct2D's rounded rectangle). If the engine moves to continuous
  (squircle) corners, the kit's surface and cards must follow, or the HIT fill and glass will
  disagree at the corners.

### Porting a surface

1. **Theme.** Make one from the surface's look. The flyouts have
   `flyouts::notifications::theme(&Look)` (move it to `Look::theme()` when you own `look.rs`):
   the bar's family picks the style (Fluent → Fluent, Material → OneUi, Mono → Mono, macOS →
   Liquid, or Solid with no glass and an opaque fill), the ink picks the scheme, and the accent
   carries over. Widgets and bars do the same from their look.
2. **Input.** Build an `Interaction` from the surface's hover, pressed and focus ids (or keep
   one and feed it `Event`s with `Interaction::handle`).
3. **Body.** `f.surface(rect, glass::available())` first, and `set_glass` with what it returns.
   (The flyout controller still draws the body via `flyouts::kit::Kit::body`, which already
   follows the same contract.)
4. **Lay out** with `take_*`, `Stack`, `grid` and `place`: no `y += magic` and no hand-placed
   rects. Ask components for their intrinsic sizes (`button_size`, `row_height`, `line`).
5. **Draw** with components, `TextRole`s and `Palette` roles only: no `TextStyle::new`, no
   `Rgba` constants, no `Op::Stroke` around anything, no raw glyph code points.
6. **Lists** that can outgrow their room go in a `Scroll` (see above); route `wheel`, Page
   Up/Down and `tick` to it.
7. **Goldens:** re-bless your surface's images (`MYPAPER_BLESS=1`) and look at them.

The notification center (`crates/shell/src/flyouts/notifications.rs`) is the reference port: a
fixed header, the cards in a scroll view, the month below; its test
`only_the_list_scrolls_and_the_flyout_keeps_its_size` holds the scroll contract.

| Old | New |
|---|---|
| `flyouts::kit::Kit::{header, row, tiles, slider, switch, buttons, notice, separator}` | `Frame::{text_in + icon_button, row, tile, slider, switch, button, empty, separator}` |
| `Look::{ink, dim(), faint(), control(), line(), hover()}` | `theme.color.{label, secondary, quaternary, fill, separator, hover}` |
| `kit::glyph::*`, `icons::centered` | `f.icon(rect, Icon::X, size, color)` |
| `Step`, `Typeface`, `kit.font(size, weight)` | `TextRole`, `f.font(role)` |
| `Ink::of(ink)` | `theme.color.{label, secondary, tertiary, quaternary}` |
| `controls::{Palette, Type}` (Studio) | `Theme` (Studio ports last) |
| `Tokens` for menus | `theme.tokens()` |
| a stepped `scroll: usize` | `Scroll` + `Frame::scroll` |

### Measured

- The notification center live (release, 1440p, the user's 5 app groups, warm, 8 opens):
  drawn in **1.6–2.0 ms** (median 1.9) after the port, against **1.9–2.8 ms** (median about
  2.5) before; on screen in 2.6–3.6 ms against 3.3–5.7. Idle: nothing ticks once the scrollbar
  has faded (`Scroll::tick` returns `None`).
- The gallery (release, headless, 1180×780, three panels and a menu): built in 2–5 ms and drawn
  in 4–6 ms per style; the first style also pays WARP's start-up (about 30–40 ms).

## Surfaces

A `Surface` is one per-pixel-alpha layered window (`WS_EX_LAYERED`, updated only through
`UpdateLayeredWindowIndirect`). Kinds:

| `Kind` | Window | For |
|---|---|---|
| `Popup { activate }` | topmost popup; `activate: false` adds `WS_EX_NOACTIVATE` and answers `MA_NOACTIVATE` | menus, flyouts |
| `Bar` | topmost, never activated except by the clicks it asks for (`set_click_activates`); the app-bar reservation is the bar's (N2) | top bar, dock |
| `DesktopChild { parent }` | **top-level** layered tool window in the bottom band immediately above `parent` (Progman); the historic name describes its layer, not its Win32 parentage | widgets and edit chrome |

- **Hit-testing follows alpha.** A layered window lets clicks through where alpha is 0 and keeps
  them everywhere else. So views fill each surface's whole shape at least with `Rgba::HIT`
  (alpha 1/255, invisible), even where the engine draws glass behind it; only the space between
  surfaces is 0. Golden tests check every body pixel of the menu is hit-testable.
- **Slides and fades** change only position and alpha (`move_to`, `set_alpha`): no redraw.
- **Memory follows visibility.** `release` drops a hidden surface's bitmaps; the next `present`
  draws everything again. Bitmaps are cropped to each surface.
- **Partial updates still pass the size.** `UpdateLayeredWindowIndirect` gets `psize` with every
  new bitmap, resized or not, plus `prcDirty`. Without the size Windows accepts the call and keeps
  showing the old pixels: every hover and press redraw was lost that way (bisected to e338153,
  fixed in c2ad184).

### Desktop windows (N4)

`DesktopChild` uses `WS_POPUP | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`.
It has no Win32 parent or owner. `parent` is the Explorer **z-order anchor**. Screen coordinates
and per-monitor physical pixels apply. The shell manifest declares Windows 8/8.1/10 compatibility
and PerMonitorV2. The manifest does not make ULW children visible: N0.2 measured that raised
Progman hides GDI/ULW children, and WGC exclusion lists ignore children. A top-level surface can
be excluded from the desktop glass overlay capture; the engine's order is widget > glass > Progman.
See [engine-notes.md](engine-notes.md#glass-overlays).

`Surface::keep_above_desktop() -> Result<bool>` repairs the bottom band when necessary and
preserves directly underlying `mypaper.wallpaper.glass` overlays. `desktop_order_valid() -> bool`
is a read-only harness probe. `raise_in_desktop_band() -> Result<()>` lifts the edit palette above
other widgets while keeping ordinary apps above it. `WM_WINDOWPOSCHANGING` suppresses automatic
activation raises; explicit repairs use a guarded move. No periodic z-order timer runs. The host
reacts to window events and restores surfaces hidden by Show Desktop; Explorer destruction drops
surfaces and `DesktopEvents::Ready` recreates them against the new anchor. `DpiChanged(u32)` tells
the host to rebuild its monitor geometry and text/image scale. Mixed-DPI hardware must still be
validated when available; physical-pixel fixtures alone cannot establish that behavior.

### Images, rings and reusable calendar (N4)

`Ui::decode_image(bytes: &[u8], max_size: (u32, u32)) -> Result<Image>` uses WIC, scales before
converting to premultiplied BGRA, and bounds decoded dimensions. `Op::Image { rect, image:
Arc<Image>, opacity, source: Option<Rect> }` shares pixel storage between frames. `source` crops
in source pixels; `None` uses the full image. `Op::RoundedImage { rect, image, opacity, source,
radius }` is the same picture clipped to a rounded rectangle (a bitmap brush filling it), for
pictures inside rounded cards; layered widget surfaces pre-round their bitmaps at load instead
(`widgets::round_corners`). `Op::Arc { center, radius, sweep, width, color }`
draws a clockwise ring segment starting at twelve o'clock, with `sweep` in radians. These ops
participate in normal damage detection. Crossfades arm the ticker only for their finite duration.

`calendar::Date { year: i32, month: u32, day: u32 }` supplies checked `new`, `parse`, `key`,
`ordinal`, `from_ordinal`, `weekday` (Sunday = 0), `iso_week`, `add_days`, and `shift_month`.
`MonthGrid::new(month: Date, week_start: u32)` builds the reusable calendar grid. Its `view(&self,
list: &mut DisplayList, rect: Rect, today: Date, text: Rgba, accent: Rgba, base: u32,
font: &TextStyle)` appends cells and hits `base + day`. Consumers own selection and event storage;
the calendar flyout can reuse the same component without depending on widget state.

### Windowless text fields (N4)

`TextField::new(window: HWND, rect: Rect, multiline: bool, size: f32, color: Rgba) -> Result<Self>`
creates windowless RichEdit text services from `Msftedit.dll`. It is UI-thread-local and cloneable.
Call `set_text(&str)`, `text() -> Result<String>`, `set_rect(Rect)`, `focus()`, `blur()`, and
`revision() -> u64`. Attach it with `Surface::set_text_field(Option<TextField>)`, and append
`Op::TextField { rect, field: TextField, revision: u64 }` to the display list. Detach and blur on
close. Text and its undo history stay in the component while it is attached.

`ITextServices2::TxDrawD2D` paints into the same software Direct2D target. The host supplies
selection/capture, timers, caret invalidation, scrolling and the surface's IMM context. Raw Win32
keyboard, pointer and IME messages enter RichEdit synchronously; app callbacks remain queued.
`Event::TextChanged` covers text, selection and caret changes; compare/read text before saving.
`Event::TextSubmit` is Enter in a single-line field. Multiline Enter stays with RichEdit. Native
selection, bidirectional shaping and undo are covered by the component test. The live fixture
also enters and persists Arabic/CJK through Unicode keyboard events; that check does not exercise
a language-specific IME candidate/composition UI.

A layered window never shows the system caret, so the field records where RichEdit puts it:
`TextField::caret() -> Option<Rect>` (surface pixels, while it shows). Views draw it as a fill;
it does not blink, so a settled field draws nothing. A single-line field hands Up, Down, Tab,
Page Up and Page Down to the app as `Event::Key` (they move the selection of what the field
filters, as in the launcher); Escape and Enter work as before.

### Glass under a surface (N3)

`mypaper_ui::glass` puts the engine's Liquid Glass under a surface. The UI layer only says where;
the shell installs the thread's `Backdrop` (`crates/shell/src/overlay_client.rs`), which sends
the engine one request per change (docs/engine-notes.md, "Glass overlays"):

```rust
pub struct GlassMaterial { clarity: f32, blur: f32, refraction: f32, highlight: f32, tint: Rgba }
pub struct Glass { body: Rect /* surface px */, radius: f32, material: GlassMaterial }
pub trait Backdrop {
    fn available(&self) -> bool;
    fn open(&self, client: HWND, shape: Shape, radius: f32, material: &GlassMaterial) -> bool;
    fn place(&self, client: HWND, shape: Shape, radius: f32);
    fn material(&self, client: HWND, material: &GlassMaterial);
    fn close(&self, client: HWND);
}
glass::install(Rc<dyn Backdrop>); glass::available() -> bool;
glass::set_style(Option<GlassMaterial>); glass::style() -> Option<GlassMaterial>;
Surface::set_glass(Option<Glass>); Surface::has_glass() -> bool;
```

- A surface with glass opens it right after `show` (the content is on screen first; the request
  may wait on the engine's loop), sends `place` **before** each move (`present` with a new
  origin, `move_to`), reshapes it on `set_glass`, and closes it on `hide` and drop. The engine
  moves its window inside the request, so both land in one composition frame.
- The surface still fills its body at least at `Rgba::HIT`: clicks land on it, never on the
  glass. Without a backdrop, or with the engine gone, `available()` is false and views draw the
  style's own fill.
- `set_style` holds the active style's popup material (`None` for solid and mono). A `Menu`
  uses `Tokens::glass` when set, else that style, and only while `available()`; with glass its
  body fill is `Rgba::HIT` under the same hairline and content. `Tokens::glass` defaults to
  `None`, so golden images keep the drawn tint.

`Text::fit(text, style, width) -> Arc<str>` cuts a label with an ellipsis at the last character
that fits (one hit test on its cached layout). `Text::lines(text, style, width, max)` wraps at
word boundaries into at most `max` lines (the last one cut when more would follow), keeping line
breaks. Both are used by the flyouts for SSIDs, app names and notification bodies.

## Input

Window procedures never run app callbacks. They turn messages into `Event`s (`Move`, `Press`,
`Release`, `Key`, `Deactivated`, `Invoke`, `Focus`, `DpiChanged`, `VisibilityChanged`,
`Cancelled`, `TextChanged`, `TextSubmit`) on the thread's queue, and the app's loop takes
them with `next_event()` after dispatching. So app state is never borrowed twice, even when Windows
sends a message from inside our own call (showing, focusing, a COM call to Explorer whose wait loop
dispatches). `Surface::discard_events` drops what a hidden or reused surface said.

### Hover, press and focus

`interact::Interaction { hover, pressed, focus, focus_visible }` is one surface's pointer and key
state. `handle(&shown_list, &event) -> Response` returns `Unchanged`, `Redraw` (a state the view
draws changed) or `Activate(id)`:

- a press shows at once (the redraw happens in the same loop pass as the `WM_LBUTTONDOWN`);
- a control fires on release over the item that was pressed (moving off cancels), on Enter or
  Space for the focused item, or on a UI Automation invoke;
- focus rings show only after the keyboard moved focus (`focus_visible`), never on a click.

`state(id, enabled) -> interact::State` (`Rest`, `Hover`, `Pressed`, `Disabled`) and
`State::wash(ink)` (10% hover, 20% pressed) are what views draw; `press_scale()` sinks pressed
content by 3%. The widget palette runs on it; the bars and widgets keep their own event handling
(dock drags, text fields) and draw from it; the flyout kit draws the same washes from its ids.

**Foreground.** A bar never takes the foreground on a press (the app in front keeps it, as with
the macOS menu bar). Items that open a flyout, start something or are tray icons need it,
because Windows refuses `SetForegroundWindow` (even after a synthetic input or the Alt unlock)
while a shell window holds the foreground lock: Search, Start, a toast. So
`Surface::set_click_activates(rects)` lets those clicks activate the bar through
`WM_MOUSEACTIVATE`, which Windows never refuses; everything else answers `MA_NOACTIVATE`.
Measured with SendInput: the launcher over Windows Search opened with the keyboard 0/4 times
before, 4/4 after.

## Motion

`Spring` is motion.ts's spring (`spring`, `spring-soft`, `spring-bouncy`; the closed-form solution
of m·x'' + c·x' + k·(x − 1) = 0, with the same settle rule). `Tween` moves a value along one.
`Ticker` is a high-resolution waitable timer (`CREATE_WAITABLE_TIMER_HIGH_RESOLUTION`) armed one
display frame ahead (DWM's refresh rate) only while something moves: wait on `handle()` with the
messages, and call `schedule(moving)` after each pass.

## Accessibility

Each surface has an `accesskit_windows` adapter from the start, holding no tree. The tree (a root
with the surface's role and name, and one node per hit with its role, label, bounds, disabled and
toggled state) is built from the hits on the first `WM_GETOBJECT`. Until a client asks, updates do
nothing. Click and focus requests come back as `Event::Invoke` and `Event::Focus`.

## The menu component

`Menu<C>` is a popup menu with cascading submenus, generic over the command type `C`:

- `Entry<C>`: `Separator`, `Action { label, command, checked: Option<bool>, enabled }`,
  `Submenu { label, items }`;
- `menu::layout` measures and stacks rows; `menu::view` is the panel's display list (shadow,
  tint, border, separators, highlight, check marks, labels, chevrons, one hit per row);
- `menu::place` opens at the cursor and flips away from the work area's edges;
  `menu::place_submenu` puts a submenu **against** its parent's edge, with no gap and no overlap;
- `Menu::open`, `close`, `contains`, `owns`, and `handle(ui, surface, event) -> Option<Outcome<C>>`
  (`Chosen(command)` or `Closed`). Keys drive the innermost panel: arrows, Enter, Escape, Left and
  Right. The pointer drives whichever panel's body is under it, so over a submenu's shadow it
  still reaches the parent underneath.
- The root window and one submenu window are created hidden in `Menu::new` and reused: creating a
  layered window costs about 20 ms, more than the rest of opening a menu. A closed menu's windows
  keep no bitmaps.

## App windows and controls (N7)

Studio ([studio-ux.md](studio-ux.md)) is the one ordinary app window. What it added, all additive:

- **`Kind::App { min }`** (`app_window.rs`): a resizable, activated, opaque window with a taskbar
  button. It is not layered: `present` draws the damaged rectangle in the WIC bitmap and copies it
  straight to the window with `SetDIBitsToDevice` (no DIB section, no third copy). The app draws
  its own title area and says where the caption and caption buttons are with
  `Surface::set_chrome(&[(Rect, Chrome)])`; `WM_NCHITTEST` answers from them (plus the top resize
  band), so drag, double-click, Aero Snap and the snap-layouts flyout over Maximize all work.
  Window helpers: `place`, `show_active`, `client_size`, `scale`, `is_maximized`, `minimize`,
  `toggle_maximize`, `set_background` (the colour behind a resize), `set_cursor(Cursor)`,
  `set_frame(dark, border)` (DWM dark mode and border colour), `set_icon_png`. Events:
  `Event::Resized(w, h)`, `Event::Close` (the app decides), `Event::SettingChanged` (light/dark,
  accent or contrast changed). Windows' size/move loop stops the app's own; `set_modal_hook`
  lets the app render from inside it. Studio calls `set_frame` again whenever its palette
  changes (before each frame it draws), so the border and caption follow light/dark live.
- **Clips:** `Op::Clip { rect }` and `Op::Unclip` bracket ops drawn inside a scroll view
  (`PushAxisAlignedClip`); damage treats the clip's rectangle as the bounds.
- **`Text::set_capacity(layouts)`:** a window with a few hundred labels on screen keeps them all
  laid out instead of reshaping them every frame.
- **Controls** (`controls.rs`): immediate-mode controls for app windows. `Palette` is the gray
  token set (dark, light, contrast), `Type` the type ramp (it follows Windows' text size). A frame
  is built by `Builder<A>`, where `A` is the app's action type: each control appends its ops, one
  hit (role, label, state) and a `Target { key, action, behavior, group, rect, visible, scroll }`.
  Hit id − 1 is the target's index. `Behavior` says how input treats it (`Click`, `Toggle`,
  `Slider`, `Field`, `Drag`, `Thumb`); `group` ties segments, tiles and rows together for arrow
  keys; `scroll` lets keyboard focus reach a target scrolled out of view. The app keeps the input
  state and hands it back as `Input { hover, pressed, focus, focus_visible, pointer }`; `key(..)`
  makes stable identities, so focus survives rebuilding the list every frame. The controls:
  `button`, `icon_button`, `switch`, `segmented`, `slider`, `select`, `field`, `badge`, `keycap`,
  `tile_frame`, `plate`, `check_mark`, `section_header`, `scroll`, and the panel helpers
  (`elevated`, `scrim`) that palettes, sheets, popovers and toasts are drawn with. Lists, tabs,
  colour swatches and dialogs are Studio's compositions of these (`crates/studio/src/ui.rs`).
- **Selection is a fill, never a ring.** A picture tile (wallpapers, styles, starters) sits on
  a `plate` drawn before it: the selected wash (`Palette::selected`) behind the picture and its
  label, the hover wash on hover, nothing at rest. A selected card takes the selected wash and a
  `check_mark` (a solid circle with a check). The only strokes left are the keyboard focus ring
  and a focused field's ring.
- **Section headings** are the 14/600 heading in ink-1 (`section_header`). The kit's 12/600
  secondary header suits glass panels; on an opaque window it was too faint to scan by.
- **Pictures** in tiles and cards are `Op::RoundedImage` (Studio's `stage::image_round`): the
  bitmap fills a rounded rectangle through a bitmap brush, so the corners antialias in one
  primitive. The old way, a ring in the page colour over a square picture, left specks where
  two antialiased edges met. Where drawn content reaches a picture's corners (the stage's
  bars, a starter's taskbar), the ring stays but starts a pixel inside the edge.
- **Selects** open the kit's `Menu` under the control (a check on the current value, keys,
  screen readers, edge flipping), on the card fill with Studio's selected wash. The app sets
  `App::picking`; the window's loop opens the menu, routes its surfaces' events to
  `Menu::handle` and applies `Outcome::Chosen` with `App::chose`. The menu's two windows are
  made on first use, so they cost nothing at startup.

## `mypaper-shell`

One UI thread, plus the desktop menu's mouse-hook thread (a low-level hook must answer in
microseconds, whatever the UI is doing). Core starts it as a supervised child (restart policy as
for the other hosts).

- **The loop** waits on its parent process and the message queue; it dispatches, then drains the
  inbox and the UI events in order.
- **The inbox** is a message-only window (class `mypaper.shell.inbox`) that only queues: the hook,
  the `config.json` and `palette.json` watchers post to it from their threads. `WM_CLOSE` to it
  quits the shell.
- **Lifetime:** one shell at a time (a named mutex; a second one exits at once); it exits when the
  process that started it does, so a crashed core never leaves a shell (and its hook) behind.
- **It hosts** the native desktop widgets and the desktop right-click menu ([desktop-extras.md](desktop-extras.md)): it
  follows `desktop_menu` in `config.json` and the theme's look live; with the menu off, nothing is
  installed. Core loads no Direct2D and installs no hook.

## Tests

- `cargo test -p mypaper-ui`: damage, clipping and premultiplied output on the software target,
  springs and the ticker, focus stepping, menu layout and placement (0 gap), the accessibility
  tree, and the GPU-driver check (an integration test: a process of its own).
- `cargo test -p mypaper-shell`: the desktop menu's items, the theme look, and **golden PNGs** of
  the default menu and its View submenu (`crates/shell/tests/golden/`), rendered headless through
  the same software target the screen gets. `MYPAPER_BLESS=1` rewrites them; a mismatch saves the
  actual image to `%TEMP%\mypaper-golden`.

## Measured

Release builds, 2026-11-04, Ryzen laptop (16 logical cores), two 1440p monitors at 96 DPI, theme
rain-city. CPU is a percentage of one core; RAM is private MB.

### N0.1: a 3840×36 bar surface

`cargo run --release -p mypaper-ui --example mypaper-bar-spike -- --x -2560 --y 9` (the bar spans
both monitors), self-measured, with `measure.ps1` alongside for 60 s.

| | Measured | Gate |
|---|---:|---:|
| Private memory, idle after 400 redraws | **8.5 MB** (`measure.ps1`: 9 MB) | ≤10 MB |
| CPU over 95 s idle | **0 ns, 1 wakeup** (`measure.ps1`: 0%) | 0% |
| Redraw, whole bar (draw + copy + `UpdateLayeredWindowIndirect`) | median **0.33 ms**, p95 0.52, max 0.88 | ≤1 ms |
| Redraw, the clock only | median **0.21 ms**, p95 0.42, max 1.23 | ≤1 ms |
| An identical frame | not drawn, 0.001 ms | |
| First paint after process creation | **73 ms** (23 ms of it drawing and showing) | recorded |
| Slide-in, position and alpha only | 118 frames in 537 ms, then the ticker disarmed | |
| GPU vendor drivers loaded | **none** | none |

Private memory step by step (live): 1.4 MB at start, 1.6 after the Direct2D, DirectWrite and WIC
factories, 2.1 after the window, 6.8 after the first paint, 8.7 after 400 redraws, 8.5 idle.

**WARP's cost.** Measured headless (no window): creating the first software target and drawing a
fill (which brings WARP up) adds **1.5 MB** private; the first text adds 0.8 MB (font loading).
Each cached `IDWriteTextLayout` costs about **19 KB**, which is why the layout cache holds 64 and
drops the least recently used half when full (an unbounded first version reached 11.2 MB in this
spike). Making and dropping targets plateaus (no leak). A software target keeps its pixels about
three times (the WIC bitmap, WARP's surface, the window's DIB). WARP is well within the budget, so
the plan's fallback (tiny-skia + `IDWriteBitmapRenderTarget1`) is not needed.

**DLLs** loaded into the spike (42): advapi32 bcryptprimitives cfgmgr32 clbcatq combase **d2d1
d3d10warp d3d11** dcomp directxdatabasehelper dwmapi **dwrite** dxcore **dxgi** gdi32 gdi32full
imm32 kernel.appcore kernel32 kernelbase microsoft.internal.warppal msctf msvcp_win msvcrt ntdll
ole32 oleaut32 powrprof psapi rpcrt4 sechost sxs textshaping ucrtbase uiautomationcore umpdc user32
uxtheme vcruntime140 win32u windowscodecs. No `nv*umx`, `nvgpucomp64`, `amdxc64` or `atidx*`; the
shell's own list (with the menu open) has none either.

### N1: `mypaper-shell` with the desktop menu

This branch's core, wallpaper and shell, in place of the user's mypaper (no bars or widgets);
`crates/shell/examples/menu_live.rs` injecting right-clicks at the left monitor's centre.

| | Measured | Gate |
|---|---:|---:|
| Menu on screen, quick click (press and release together) | median **13.1 ms**, best 11.3, worst 14.5 | ≤25 ms |
| Menu on screen, a person's click (70 ms hold), after the release | median **1.8 ms**, worst 21.4 | |
| Escape closes it (it took the focus) | 30 of 30 | |
| Core loads Direct2D, DirectWrite, D3D or DXGI | **no** | no |
| Shell idle, before any menu | **3.0 MB**, 0% CPU | |
| Shell idle after 30 menus (`measure.ps1`, 60 s) | **11 MB**, 0% CPU; core 2 MB, 0.03% | |
| A menu open, over the warm closed state | **+0.9 MB**; with the View submenu +1.35 MB | ≤+6 MB, freed on close |
| Shell start to its loop running | 83 ms (`READY ready_ms`) | |

- The **first** menu costs the shell about 7 MB once (WARP, DirectWrite's fonts, UI Automation and
  the COM link to Explorer); in N2 the bars pay that at start, so a menu adds only its bitmaps.
- **Freed on close:** a closed menu's bitmaps and render targets are released, but the shell's
  private bytes stay at the heap's high-water (about 1 MB, reused by the next menu: no growth over
  30 menus). `HeapSetInformation(HeapOptimizeResources)` gave back only 0.08 MB, so it is not used.
- One quick click of 15 in the second run did not open our menu (it may have opened Windows' own);
  60 clicks over two runs had no other miss. The hook's decision logic is unchanged from core's.
- Golden PNGs of the default menu and its View submenu match the live menu's content; the only
  visible change is the submenu, which now touches the root instead of overlapping it by 4 px.

## N2 additions: bar icons and pointer events

- `Op::Image { rect, image: Arc<Image> }` draws premultiplied BGRA on the same software target.
  `Image::load(&Ui, &Path)` uses WIC to decode native PNG/ICO/JPEG/etc. A target caches up to 128
  shared bitmaps; releasing the surface releases them. The shell handles SVG icon-pack rasterization
  once with CPU-only resvg and shares decoded images across monitors.
- `Event::Leave` uses `TrackMouseEvent`; `Event::Wheel(Pointer, i16)` converts Windows' screen
  coordinates to the surface's local coordinates and retains the screen anchor. Positive delta is
  upward. `Event::DoubleClick` preserves native tray double-click callbacks. `Event::CaptureLost` lets dock drags cancel cleanly, and `Event::DpiChanged` asks the owner
  to recalculate monitor geometry. Existing menu/event consumers continue to work.
- N2 uses `Kind::Bar` for top bars, cropped docks and one-pixel auto-hide edge surfaces. Invisible
  glass bodies are explicitly filled at `Rgba::HIT`; only the gaps are alpha zero. Reservations live
  in the bars service library, separate from visible window geometry. Slides use `move_to` only.
- The shell loop also waits on the bars' disarmed-at-rest `Ticker` and virtual-desktop event. Its
  timeout is the earliest active provider/config/auto-hide deadline; no periodic message pump was
  introduced. Bar data/actions are Rust calls into `mypaper-bars` rather than a web bridge.

See [bars.md](bars.md) for configuration, recovery ownership and the N3 flyout/plate seams.

N2 bar measurements (2026-11-05, two visible 1440p screens, still photo, paused media): clean idle
median 14 MB and 0.05% of one core; observed peak 21 MB. Three maximized samples have a 0.08% median,
above the 0.05% gate. The final bar paint was at process age 164.4 ms (143.9 ms after core's post-spawn
marker), above the 73 ms reference. Shell kill/repaint took 370-572 ms, above 300 ms. Alpha hit tests,
the native clock click, panic restoration and six style PNGs pass. See [bars.md](bars.md#live-n2-measurements-2026-11-05)
for raw samples, the interrupted Explorer sample and the missing engine/glass/N4 fixture checks.

### N4: top-level desktop widget fixture (2026-11-05)

Six production widgets and a warmed closed menu, both 1440p monitors visible at 96 DPI. Three
60-second official samples: private memory 21/22/22 MB (median 22 MB), CPU 0.18/0.21/0.18% of one core
(median 0.18%, **above the 0.1% gate**). Actual-wall-time harness CPU median was 0.15623%. No vendor
GPU drivers or widget WebViews loaded. Consecutive minute samples added exactly 30 visible stats
presents and one clock present. The locked interaction fixture passed 12 app activations, a real
widget click, Show Desktop/restore, six new HWNDs after Explorer restart, grid drag/resize, and saved
Arabic/CJK Unicode keyboard input. The glass publisher is a stub and these are not merged-shell or
whole-product numbers. Mixed DPI and actual language-IME composition remain unmeasured. Full evidence
and remaining gates: [N4 report](../artifacts/n4/report.md).

## Layers

Components, themes, layout, interaction, geometry and `DisplayList` are portable core.
`damage::regions` shares the pixel-damage policy (including resize and sparse redraw);
`DisplayList::damage` and `damage_regions` retain the existing operation comparison.
Text styles and BGRA bitmap data are portable values, separate from native resources.

`platform` defines static contracts: `Backend` selects associated services, `Renderer`
draws damaged display-list bounds into a CPU bitmap, `TextSystem` owns native formats
and cached layouts, `Window` presents native bitmaps and supplies events, and `TextInput`
owns native editing and IME focus. `platform::current` selects Windows or a compile-only
stub with `cfg`; unsupported native operations fail at runtime. There are no backend
trait objects. Native handle APIs remain Windows extensions.

Windows implementations live under `platform::windows`; root compatibility exports
preserve application paths. Accessibility snapshots/tree construction, glass materials,
menu models/layout/view and icon vector data stay core. Windows owns the accessibility
adapter, HWND backdrop hook, screen queries, icon font placement and native text/raster
resources. Golden PNG comparison/IO already uses portable `Image` data; its raster target
is supplied by the selected renderer. Spring/tween math stays core, while the waitable
frame timer and DWM refresh query live in the Windows backend.

## Render modes

A surface draws in one of two ways. **CPU** (the default) is everything above: software
Direct2D on a WIC bitmap in a per-pixel-alpha layered window, no GPU device in the process, and
glass (if any) from an installed [`Backdrop`](#glass-under-a-surface-n3), else on Windows the
[system backdrop](#system-backdrop)'s frost, drawn by DWM. **GPU** draws the same
display list with the same code on a hardware device, and draws Liquid Glass in-process. Both
kinds mix freely on one UI thread.

```rust,ignore
use stillkit::platform::{Render, set_render};
set_render(Render::Auto);                    // this thread's default, before making surfaces
let bar = Surface::new(Kind::Bar, "bar", Role::Toolbar)?;              // the default
let panel = Surface::with_render(kind, "panel", Role::Dialog, Render::Gpu)?; // its own choice
assert_eq!(panel.render(), Render::Gpu);    // Cpu or Gpu, Auto already resolved
panel.set_glass(Some(glass));                // in-process glass under the panel's content
panel.set_glass_group(&[a, b], 48.0);        // one liquid container: within 48 px they fuse
stillkit::platform::windows::gpu::set_power(gpu::Power::High); // default Low: the iGPU
```

- **Where the choice lives.** A thread default (`set_render`, else `STILLKIT_RENDER=cpu|gpu|auto`,
  else `Cpu`) plus a per-surface override (`Surface::with_render`). Thread-level, like the glass
  backdrop, because `Surface::new` takes no `Ui` and `el::run` makes its surface from an `App`
  with no render field: an app picks GPU with one call before `run`, and mypaper's ~1,000 call
  sites stay as they are. Per surface, so CPU bars and widgets can sit beside one GPU panel.
- **`Auto`** resolves when the surface is made: `Cpu` unless glass is wanted
  (`glass::set_style(Some(..))`, see `glass::wanted`) and no backdrop draws it in full on the
  CPU (`glass::cpu_backdrop`: none is installed, and the system backdrop is unavailable or the
  material has a lens); then `Gpu`. The measured numbers below and in
  [System backdrop](#system-backdrop) are why it isn't simply "GPU when glass".
- **The `gpu` feature.** Without it no GPU code is compiled (no wgpu, no `stillkit-glass`), and
  every mode is `Cpu`. With it, nothing GPU-related loads until the first GPU surface: the
  device is made then, once per thread (`gpu::device`), and a CPU surface never touches it
  (`tests/gpu_driver.rs`; `tests/no_gpu_driver.rs` still passes). Linux uses Vello Hybrid (published as `vello_gpu` 0.3) on Vulkan; the
  stub backend has the same calls and draws on the CPU. Linux initializes its device on the
  first GPU presentation or headless GPU render; simply constructing a surface is lazy.
- **Headless renders** (`render::render`, `el`'s `--snap`) follow the thread's mode, so a GPU
  app's snapshots are what its screen shows. `gpu::render` always draws on the GPU.

### How a GPU surface draws

```text
Surface::present(ui, origin, size, list)       (GPU surface)
  1. damage = the same regions as the CPU path                    (none: no draw, no commit)
  2. per region: IDCompositionSurface::BeginDraw(region) → the D2D 1.1 device context targets
     that piece of the atlas, translated to its offset → Painter::region (clip, clear, ops)
  3. a move or resize: SetWindowPos (the glass is in the window, so it moves with it)
  4. one IDCompositionDevice::Commit, only if something changed
```

- One drawing code: `render::Painter` holds the Direct2D target, brush and geometry caches that
  `Canvas` used to; the software canvas and the GPU layer each own one. The device is hardware
  Direct3D 11 (BGRA) on the adapter `gpu::Power` prefers (`EnumAdapterByGpuPreference`), with a
  Direct2D 1.1 device and a DirectComposition device on it.
- Windows: the same kinds (popups, the bar, desktop-band windows, app windows), made with
  `WS_EX_NOREDIRECTIONBITMAP` instead of `WS_EX_LAYERED`: per-pixel alpha comes from the
  premultiplied composition surface. `set_alpha` is the root visual's opacity.
- **Hit-testing differs.** Without a layered bitmap Windows can't hit-test by alpha, so
  `WM_NCHITTEST` answers from the last frame's top-level op bounds: a rounded corner's outside
  hits as its bounding box, and a miss passes to windows **of this thread** only
  (`HTTRANSPARENT`), not to other apps. Views already fill their body with at least `Rgba::HIT`,
  so clicks on the body behave as on the CPU.
- Not ported: `layered_app` (the alpha canvas for app windows) stays CPU-only.

**Tolerance.** GPU pictures match the CPU ones except for antialiasing on edges: on the kit
gallery pages (`STILLKIT_RENDER=gpu cargo run --release -p stillkit --features gpu --example
gallery -- <dir>`, then `glass_compare`) at most **0.22 %** of channels differ by more than 4
levels (**1.4 %** on the icon sheet, which is all edges); single edge pixels (an arc's square end,
round stroke caps) differ by up to 191; flat areas and text are exact (`kit-text.png` is
byte-identical). `tests/gpu_render.rs` holds a mixed page to ≤ 0.5 % of channels over 4 levels,
each within 128, interiors exact. CPU mode is byte-identical to `docs/media/native/kit-*.png`
with and without the feature.

### Glass in GPU mode

`platform::windows::gpu_glass` draws `stillkit-glass` (liquid-rust) on a wgpu DX12 device on the
same adapter, into a premultiplied composition swapchain on a visual **under** the surface's
content visual: the panel's labels draw over its glass. The two devices share nothing; they are
two visuals of one tree. It draws when the glass changes and while its springs move (a
`glass-motion` thread posts frames until they rest), never when idle.

| Surface kind | What its glass refracts |
| --- | --- |
| Desktop-band window | The wallpaper under it: what is really there. |
| Bar, popup | The wallpaper under it: a stand-in for the windows that may be behind. |
| App window | The wallpaper too, for now (see below). |

The wallpaper is decoded once per thread (WIC, scaled to cover the primary screen like "Fill")
into a GPU texture; each glass copies its window's piece of it, again on a move. Later work:
other apps' windows through Windows Graphics Capture; and the app's own content beneath an
in-window panel, by drawing the ops before the panel's `HIT` fill into a D3D11 texture shared with
D3D12 (an NT handle and a fence) as the glass's backdrop, with the panel's own ops above it.

`set_glass_group(&[Glass], spacing)` puts every body in one liquid container: shapes within
`spacing` fuse through a smooth union, and a body given a new frame springs to it, merging and
splitting on the way (`examples/glass_merge.rs`). The material maps to liquid-rust's regular glass:
`refraction`, `blur` as frost (px to points), `highlight` as the specular rim, `tint` as the stain,
`1 - clarity` as the transparency slider; dark glass unless a light tint says otherwise.
`GlassStyle`'s brightness, saturation, smoothing, depth and inner shadow carry over.

### Measured

`examples/glass_measure.rs`, 2026-11-07, this PC (Ryzen 7 7800X3D with its Radeon iGPU, RTX 4070
SUPER, 2560×1440 at 100 %), release builds. One 360×220 popup panel (a title, a slider; glass under
a 328×188 body), 10 s settle, 30 s idle, then 10 s of the app moving its own slider every display
frame. Sampled from outside: private bytes, CPU time, every thread's context switches (drivers'
included), and the `GPU Process Memory` counters. "Glass process" is a second process with its own
window under the panel, a wgpu device and the same glass code; its numbers are added in.

| Configuration | Adapter | Private MB (app + host) | GPU dedicated / shared MB | Wakes / 30 s idle | Idle CPU | Drag CPU | Drag frame median / p95 ms |
| --- | --- | --- | --- | --- | --- | --- | --- |
| CPU only (fill) | none (WARP) | **6.4** | 0 / 0 | **22** | 0.000 % | 4.5 % | 0.282 / 0.662 |
| CPU + glass process | Radeon iGPU | **228.4** (6.5 + 221.9) | 97.6 / 20.7 | **2034** (4 + 2030) | 0.052 % | 7.2 % | 0.222 / 0.301 |
| CPU + glass process | RTX 4070 SUPER | **168.5** (6.4 + 162.1) | 74.6 / 35.9 | **2848** (11 + 2837) | 0.104 % | 5.3 % | 0.252 / 0.661 |
| GPU, no glass | Radeon iGPU | **29.8** | 8.8 / 3.2 | **61** | 0.000 % | 5.2 % | 0.248 / 0.351 |
| GPU, no glass | RTX 4070 SUPER | **58.1** | 13.9 / 1.7 | **2424** | 0.104 % | 7.8 % | 0.201 / 0.359 |
| GPU + in-process glass | Radeon iGPU | **251.8** | 106.4 / 26.0 | **2050** | 0.000 % | 5.3 % | 0.237 / 0.318 |
| GPU + in-process glass | RTX 4070 SUPER | **178.3** | 88.5 / 35.0 | **5621** | 0.312 % | 6.7 % | 0.162 / 0.214 |

- **Glass costs the same in or out of process.** The wgpu DX12 device is the cost: about
  160–220 MB private, 75–106 MB dedicated GPU memory, and 70–190 driver wakes a second while idle,
  wherever it lives. In-process GPU + glass is within 10 % of CPU + a glass process (iGPU 252 vs
  228 MB; NVIDIA 178 vs 169 MB). By the rule "if a separate glass process costs about as much as
  going GPU, go GPU", an app that must bring its own glass goes GPU.
- **GPU without glass never pays.** Hardware Direct2D costs 30 MB (iGPU) to 58 MB (NVIDIA, with
  ~80 driver wakes a second) against 6.4 MB on the CPU, for the same drag CPU and frame time.
- **The iGPU is the better default** for wakes and CPU (61 vs 2424 idle wakes without glass); the
  discrete GPU's D3D12 stack is smaller in private memory (178 vs 252 MB with glass).
- Drag CPU and frame time are this process's own (the draw and the present call); the GPU's and
  DWM's work is not in them. CPU times are at the 15.6 ms granularity of process times.

**A CPU surface beside a GPU device** (`glass_measure mixed`: a 200×40 widget, 300 redraws, in a
process that already holds the GPU panel and its glass): the first CPU surface adds 1.8–2.5 MB
(WARP and the software target), later ones 0.1 MB; a GPU surface adds 0.15–0.22 MB. Redraw median
0.124–0.129 ms on the CPU against 0.139–0.160 ms on the GPU, the same CPU time. So small surfaces
stay on the CPU even in a GPU process: about free, a little faster, and their redraws never wait on
the GPU (one NVIDIA run had GPU p95 spikes of 12.7 ms; the CPU's stayed under 0.3 ms).

**The CPU path is unchanged** (`stillkit-bar-spike --seconds 60`, `main` against this branch
without the `gpu` feature, alternating runs on a busy PC): whole-bar redraw median 0.28–0.37 ms
both, clock-only 0.14–0.19 ms both, 1 idle wakeup in 60 s both, private after redraws a median of
8.5 MB before and 8.6 MB after (each run within 7.97–8.96 MB), first paint 6.6–6.9 MB both, no GPU
driver. The gallery is byte-identical with and without the feature.

**`Auto`, from these numbers:** CPU when no glass is wanted; when glass is wanted, the external
backdrop if one is installed (mypaper's engine: its device is already paid for, and one serves
every app); then the system backdrop on Windows when the material is frost without a lens (it
costs about nothing, [System backdrop](#system-backdrop)); otherwise GPU, with small glass-less
surfaces made `Render::Cpu` beside it. A glass process would win only if it could exit while no
glass shows, or serve several apps.

## System backdrop

One UI and Fluent are defined by frost: a blur of whatever is behind the panel. A CPU surface
can't see behind itself, so without an engine they used to fall back to a flat tinted fill. On
Windows 11 a CPU surface now gets the frost from DWM itself
(`platform::windows::system_backdrop`), with no GPU device in the process: DWM draws the blur out
of process, and `tests/no_gpu_driver.rs` opens it and finds no vendor driver and no Direct3D 12.

```rust,ignore
// No backdrop installed: the built-in one serves the kit's glass, as an engine would.
let glass = f.surface(body, surface.glass_available());   // One UI: Some(Glass), a HIT fill
surface.set_glass(glass);                                  // frost under the body, from DWM
stillkit::platform::windows::glass::system().active();     // its compositor is up
```

**How.** Each CPU surface with glass gets a *frost window* directly under it, the size of its
body: layered with a fully transparent bitmap (so it is click-through and moves with a
position-only `UpdateLayeredWindow`), never activated, topmost when its surface is. A
Windows.UI.Composition `DesktopWindowTarget` on it holds one visual clipped to the body's rounded
rect (`RectangleClip` with corner radii): a `HostBackdropBrush` sprite (DWM's acrylic blur of
what is behind the window, `DWMWA_USE_HOSTBACKDROPBRUSH`), and over it a colour sprite with the
material's tint as given (its alpha is the stain). `blur` under half a pixel is clear glass: the
tint alone. It speaks the same `Backdrop` contract as the engine, so views and components don't
change: the surface opens it after `show`, places it **before** every move and on reshapes,
closes it on `hide` and drop; the surface's `WM_WINDOWPOSCHANGED` puts the frost back under it
when its z-order changes. Nothing is made until a surface opens glass; the windows, visuals and
compositor all go when the last one closes (the thread's `DispatcherQueue` stays: a few KB).

**Which backdrop.** `glass::install`ed (mypaper's engine) **always** wins, available or not, so
mypaper, which installs its overlay client in the shell and Files before any surface, is
unchanged (`tests/backdrop_system.rs`). Without one, Windows uses the system backdrop; other
platforms have none. `glass::available()` is true while it is (Windows 11, and Settings' "Transparency effects" on: read
per call, one registry value), so menus and `Frame::surface` take glass; otherwise views draw
their fill as before.

| Surface kind | System frost |
| --- | --- |
| `Popup` | Yes: a topmost frost window under it. |
| `Bar` | Yes, the same. |
| `DesktopChild` | Yes: a non-topmost frost window under it in the desktop band; it blurs the wallpaper. |
| `App` | No: a CPU app window is opaque (`SetDIBitsToDevice`), so frost under it can't show. `glass_available()` is false unless an installed backdrop serves it. |

**The Auto rule.** `Render::Auto` makes a surface `Gpu` only for glass no backdrop draws in full
on the CPU (`glass::cpu_backdrop`): with an installed backdrop, `Cpu`; else with the system
backdrop available and a material with no lens (One UI, Fluent, Liquid with refraction and
highlight at 0), `Cpu`; else (a lensed Liquid, Windows 10, transparency off, Linux) `Gpu` when
the `gpu` feature is on. A CPU surface given a lensed material still gets the system frost,
without its lens and rim.

**Measured** (`examples/backdrop_measure.rs`, 2026-11-08, this PC, release): a 360×420 One UI
panel, 10 s settle, 30 s idle, then 10 s of the app moving its own window along a circle every
display frame. Sampled from outside; drag CPU and frame time are this process's own (DWM's
work is not in them).

| Configuration | Private MB | Idle wakes / 30 s | Idle CPU | Drag CPU | Drag move median / p95 ms | GPU drivers loaded |
| --- | --- | --- | --- | --- | --- | --- |
| CPU, fill (before) | **8.3** | **4** | 0.000 % | 3.0 % | 0.170 / 0.226 | none |
| CPU, system backdrop | **8.4** | **5** | 0.000 % | 8.3 % | 0.397 / 0.475 | none |
| GPU, in-process glass (frost only) | **258.3** | **1958** | 0.000 % | 16.7 % | 0.880 / 2.655 | `amdxc64`, `amdxx64`, `atidxx64`, `nvgpucomp64`, `nvwgf2umx` (+ `d3d12`) |

Both CPU rows load the same graphics modules (`d2d1`, `d3d10warp`, `d3d11`, `dxgi`, `dxcore`,
`dcomp`, `coremessaging`).

- **Free when idle and in memory.** The frost adds no private memory to speak of and no idle
  wakes: DWM does the blur, and nothing in this process runs while nothing moves. The GPU glass
  that would otherwise draw this frost costs a wgpu device (~260 MB private, ~2,000 driver wakes
  in 30 s, both vendors' drivers).
- **A move costs one more window move.** The frost window moves with a position-only
  `UpdateLayeredWindow` (0.2 ms; `SetWindowPos` was 0.5 ms and doubled the drag again,
  `backdrop_spike movebench`), so a dragged frosted surface spends 0.4 ms a move against 0.17,
  still half the GPU path's CPU.
- **The modules.** `dcomp.dll` and `coremessaging.dll` were already loaded by a CPU surface; the
  compositor adds no vendor driver, no `d3d12.dll` and no `windows.ui.dll`.

**What else was tried** (`examples/backdrop_spike.rs`, each over this process's own stripes):

- *The host backdrop in the surface's own window*, a `DesktopWindowTarget` with `isTopmost =
  false`: DWM composes a layered window's visuals over its pixels anyway, and the blur is opaque,
  so the content vanished. Hence the separate frost window.
- *`DWMWA_SYSTEMBACKDROP_TYPE` (transient acrylic)* on a never-activated popup: only the grey
  fallback. System backdrops show on active windows only, and are whole-window shapes; they fit
  an app window's own background, not popups, bars or widgets.
- *`SetWindowCompositionAttribute` acrylic accent* (undocumented): it does blur under an inactive
  popup, but only the whole window rect, unrounded. Not used.

**Limits.** No lens, rim or liquid merging (GPU or an engine); separate shapes don't fuse
(`set_glass_group` shows the first body). The blur radius is the system's, not `blur`. A fade
(`set_alpha`) fades the surface, not its frost (a host backdrop under partial opacity drops its
blur). Moves are two window updates in one turn of the loop, placed before the surface moves, as
the engine's windows are; nothing makes them one composition frame. The frost blurs what is behind the frost
window, which is what is behind the surface.

## Linux backend

Linux selects `platform::linux`: Vello CPU 0.3 with the bitmap glyph atlas disabled,
cosmic-text shaping, fontconfig's sans fallback, and bundled OFL Inter variable fonts.
Weight 600 is a real variable-font instance, including the coordinates passed to Vello.
All display-list primitives, images, nested rounded clips and headless `render()` work.
The gallery's twelve Linux pages are in `docs/media/native/linux/`; that directory also
records visual differences and measurement evidence.

Native windows use raw Wayland + xdg-shell and two released-buffer-tracked ARGB8888
`wl_shm` buffers. Damage is physical pixels; callers draw at `Surface::scale()`.
Unchanged lists do not draw or commit. `next_event()` drains pending input without blocking;
`host::wait()` sleeps on the Wayland display fd with no idle timeout;
`animation_frames()` requests compositor callbacks only for its guard's lifetime/duration.
Use `Surface::popup(parent, name, at)` for parent-relative popups; parentless `Kind::Popup`
falls back to a toplevel. `set_chrome`, `start_move` and `start_resize` use real input
serials for client-side chrome. Basic XKB keyboard entry and windowless field rendering
work; CPU surfaces create no graphics device.

On m6u (Ubuntu 26.04 / GNOME 50.1, Ryzen 5 7640HS), the release 400×220 hello window
used **6,016 KiB private** at steady idle. Over 30 seconds: **0 user/system CPU ticks,
0 voluntary/involuntary context-switch deltas**, and **0 GPU libraries** in `/proc/PID/maps`.
This excludes startup protocol traffic; compositor pings and reconfiguration may wake it.
The measurement temporarily used GNOME's strict new-window focus policy, restored it,
and sent no activation request or synthetic input. The protocol's configure states
contained no activation. The saved hello PNG is the client's rendered frame; GNOME denied
a desktop screenshot, so it is not a capture of compositor output.

The element host now runs on Linux too. `Surface::place` sets the physical client size;
the compositor chooses its global position. `scale` returns the host's floating-point
scale contract; configure and preferred-buffer-scale events update physical dimensions.
Opaque app backings advertise an opaque region. Caption drags use `xdg_toplevel.move`
with the real press serial; chrome buttons minimize, toggle maximize or queue close.
`host::shift_down` reads the compositor-provided XKB state. `--snap`, `--tree` and
`--script` run without a Wayland connection, with hello and sound exports at 1×/2×
in `docs/media/native/linux/el-*.png`. No element-runtime or core changes were needed.

Release `el_hello`, 320×180 on the same GNOME session, after a **10 s settle** and over
**30 s**: private memory (**Private_Clean + Private_Dirty**, smaps_rollup)
**5,992 → 5,688 KiB**, RSS **9,056 → 8,752 KiB**, PSS **6,299 → 5,995 KiB**.
User/system CPU deltas: **0 / 0 ticks** (100 Hz); voluntary/involuntary context-switch
deltas: **0 / 0**; GPU-library mappings: **0**. Private memory did not grow; resident
pages fell during the interval. The earlier host implementation run was 5,964 KiB
at both endpoints; these are separate launches, not a paired memory optimization claim.
The final trace has one content attachment/damage, no frame requests and no activated
configure state. Evidence: `el-measurement.json`, `el-maps.txt`, `el-protocol.log` in the
Linux media directory; `measure.py` reproduces this run and restores GNOME's focus policy.
All Linux golden/serde tests (76 passing, one ignored doc example), all-target Clippy,
and Windows MSVC all-target checking pass. The eight existing non-Windows accessibility
dead-code warnings remain.

Not yet supported: Wayland text-input-v3/IBus IME, field selection/caret interaction,
AT-SPI accessibility, X11, fractional scaling, complete mixed-monitor placement, cursor
shapes, and compositor blur. Fontconfig discovery currently loads the primary sans
fallback; complete script-specific fallback discovery is pending. Integer preferred-buffer-scale changes are forwarded as
DPI events; physical mixed-DPI hardware remains untested. Global position and work-area
coordinates are unavailable on Wayland; `screen_at` reports an output extent, without
panel reservations. Bars/background surfaces use layer-shell when advertised and return
an error on GNOME, which has no layer-shell. Layer-shell and parented popup paths compile
but have not been exercised on another compositor. DirectWrite text-quality controls are
Windows-only; Linux uses Vello's grayscale outline rasterization for all presets.

### Linux GPU mode

With `gpu`, `Surface::with_render(.., Render::Gpu)` uses **Vello Hybrid**, renamed
`vello_gpu` in its published 0.3 release (the dependency remains named `vello_hybrid`).
It shares `vello_common` 0.3, glyph outlines and the single `linux/scene.rs` display-list
builder with `vello_cpu`; image upload is the only resource-specific callback. Only Vulkan
is enabled in wgpu. The first GPU draw requests a low-power hardware adapter; software
Vulkan adapters are rejected with an error. Adapter/swapchain failures are reported, rather
than silently switching a GPU surface to CPU; device-loss recovery is not implemented. A thread shares one device between its GPU
surfaces. CPU surfaces never touch it, including with `gpu` compiled in. `Auto` follows
the same glass-wanted / external-backdrop rule as Windows.

Changed physical bounds are rendered into a damage-sized GPU target and copied into a
retained texture. The Wayland swapchain receives the retained image (a full texture copy,
not a full scene rerasterization). No damage means no render, acquire, submit or commit.
Resize redraws the whole surface. With glass, optics and composition redraw the full
surface when content or optics change; refractive dependencies extend beyond content's
damage rectangle. Glass springs use compositor frame callbacks, which stop when the
scene rests. CPU still uses its original two `wl_shm` buffers. Window opacity is applied
to premultiplied pixels, and hide/show retains the client frame.

GPU surfaces support `set_glass`, `set_glass_style` and liquid-merging `set_glass_group`.
Glass is rendered by `stillkit-glass` on the **same Vulkan device**, underneath the Vello
foreground. A Wayland client cannot read the desktop: the default refractable backdrop
is an explicit generated checker/gradient test pattern. The Linux extension
`surface.set_glass_backdrop(image)?` accepts client-owned premultiplied BGRA pixels;
it repeats/crops that image at physical pixel size. Call it again when the app backdrop
changes. This is not compositor blur or a desktop wallpaper capture. Tinted appearances
and continuous corners follow `GlassStyle`; there is no external Linux desktop-glass provider.

Linux-only inspection/event-loop extensions: `Surface::image()` reads the retained client
frame (including glass, before whole-window opacity); GPU readback blocks and is never
used by normal presentation. `linux::surface::connection_fd()` lends the Wayland fd for
poll loops that also watch other descriptors; dispatch with `next_event()` when readable.
No existing public paths changed. `Render::resolve` now recognizes feature-enabled Linux.

`tests/linux_gpu_render.rs` compares CPU and GPU shapes, text, icons, images and nested
rounded clips. Tolerance: no channel differs by more than **128**, and at most **0.5%**
of channels differ by more than **4**; flat blended interiors allow one level of rounding.
Measured on m6u: maximum **56**, **0.094%** above four. A retained partial update matches
a complete GPU render byte-for-byte. `linux_no_gpu_driver` checks the CPU boundary both
with and without the feature; `linux_gpu_driver` checks lazy loading in the opposite
direction and a return to CPU drawing after Vulkan initialization.

The gallery and Linux mix references were regenerated, and the gallery, motion strips
and native GPU/glass client captures were opened and inspected. Linux motion tests now
keep separate font/raster references under `tests/golden/linux`, preserving Windows images.

Release on m6u (amdgpu `1002:15bf`, kernel `7.0.0-31-generic`, GNOME Wayland):
400×220 physical pixels, **10 s settle, 30 s idle, 10 s scripted slider drag at 60 Hz**.
CPU is a feature-off executable; the others compile `gpu`. Mixed is one CPU surface
beside one GPU surface and updates both during drag. Frame times include list construction,
scene drawing and submission/presentation calls; they are CPU wall time, not GPU timestamps.
Drag CPU is process CPU time divided by elapsed time (100% = one core).

| Mode | Private MiB: settle → idle end → drag end | GPU allocated MiB: VRAM / GTT | Idle user/system ticks | Idle switches: voluntary/involuntary, all threads | Drag CPU | Frame mean / p95 ms |
|---|---|---|---|---|---|---|
| cpu | 5.60 → 5.60 → 5.82 | 0.0 / 0.0 | 0 / 0 | 0 / 0 | 3.1% | 0.440 / 0.496 |
| gpu | 61.83 → 61.83 → 62.30 | 195.2 / 72.7 | 0 / 0 | 0 / 0 | 5.4% | 0.863 / 0.970 |
| glass | 63.04 → 63.04 → 63.31 | 195.2 / 74.4 | 0 / 0 | 0 / 0 | 6.6% | 1.071 / 1.206 |
| mixed | 63.07 → 63.07 → 63.71 | 195.2 / 72.7 | 0 / 0 | 0 / 0 | 7.4% | 1.187 / 1.525 |

Private is `Private_Clean + Private_Dirty` from `smaps_rollup`. GPU allocations come from
amdgpu fdinfo, deduplicated by DRM client id; VRAM/GTT are driver accounting on this UMA
machine, not dedicated board residency or additional process-private RAM. Every mode had
zero idle CPU ticks **and zero context switches across all threads**. Tick accounting is
100 Hz. The small memory increase after dragging is included above.

CPU mapped **none** of the forbidden libraries. GPU modes mapped `libvulkan.so`,
`libvulkan_radeon.so`, `libvulkan_lvp.so`, `libdrm.so` and `libdrm_amdgpu.so`; the loader
probes LVP, while fdinfo confirms the amdgpu hardware device. No `libEGL`, `libGL`,
`libgbm` or `radeonsi` loaded. These numbers favour keeping small, non-glass surfaces on
CPU. GNOME's strict new-window focus policy was restored after each run; no activation
request or synthetic mouse/keyboard input was sent, and every child was terminated.
`gpu/*.png` are retained client-frame readbacks, not screenshots of the compositor.
The harness takes an initial readback before settling and another after idle; neither
readback is inside the idle or drag timing intervals.


Validation: workspace golden/serde tests with and without `stillkit/gpu`, all-target
Clippy with GPU, and Windows MSVC all-target checking pass. Linux emits the eight existing
accessibility dead-code warnings, with no new warnings. Evidence and the reproduction
script live in `docs/media/native/linux/gpu/` and `measure_gpu.py`.

## Looks

A look bundles independent values in `stillkit::theme`: `Material`, `Shape`,
`TypeRamp`, `Spacing` (also called `Metrics`), `Palette`, `Motion`, `Icons`,
border ink and `FocusRing`. The five presets expose the same constructors on
each part: `liquid`, `one_ui`, `solid`, `fluent`, `mono`. Palette and whole-theme
constructors take `(Scheme, Rgba)`; other part constructors take no arguments.
The old root `stillkit::Material` remains the menu tint type for compatibility.

```rust,ignore
use stillkit::theme::{Material, Radius, Shape, Theme, TypeRamp};
let look = Theme::one_ui(scheme, accent)
    .material(Material::liquid().refraction(0.3).blur(20.0))
    .shape(Shape::one_ui())
    .type_ramp(TypeRamp::one_ui())
    .radius(Radius::Control, 10.0);
assert!(look.check().is_empty());
```

Builder setters resolve the material, concentric card radius (`surface - pad`,
floored at the shape's `card_min`), label steps and accent again. Palette setters
retain the requested values separately so later changes solve from the original
ink rather than repeatedly degrading it. Use `.palettes(light, dark)` to replace
both scheme palettes. Adaptive scheme changes retain mixed shapes, optics and type.
The legacy public token fields remain writable; use builders for changes that
need derived values recomputed. `Radius::Card` changes the concentric minimum.

`Material::Glass` carries its own `lensed` flag. `Material::overrides(&MaterialOverrides)`
applies sliders without consulting a look name. Non-lensed materials keep lens
controls at zero. Fill materials ignore optics. `tint` sets an explicit RGBA;
otherwise tint follows the palette. A clarity override also sets tint alpha to
`1 - clarity`. `Theme::check()` reports unsatisfied contrast constraints: label
7:1, secondary 4.5:1, tertiary and accent 3:1. Checks include cards, and very clear
glass (tint alpha below 0.5) is checked against both backdrop extremes. Presets
retain their existing scheme-relative contrast contract and exact token values.
Hosts remain responsible for adaptive schemes over arbitrary wallpapers.

`FocusRing` exposes width, gap and opacity for hosts. The existing kit focus-ring
renderer still uses its historical fixed values; this change's kit ownership is
limited to replacing the Mono slider check with a theme value.

Enable Cargo feature `serde` for `Theme::from_toml` and `Theme::from_json`.
Both return `anyhow::Result<Theme>` and use the same strict schema (unknown keys
are errors). See `crates/stillkit/looks/sunset.toml`:

```toml
base = "one_ui"
scheme = "dark"
[material]
base = "liquid"
refraction = 0.3
[shape]
control = 10.0
```

The document's optional `base` defaults to `liquid`; `scheme` is `light` or `dark`
(default `dark`); `accent` is `[r,g,b,a]`, channels in 0..=1. Each optional part
object has an optional `base` preset (`liquid`, `one_ui`, `solid`, `fluent`, `mono`)
and property overrides:

| Part | Properties |
| --- | --- |
| `material` | `clarity`, `blur`, `refraction`, `highlight`, `tint` (RGBA), `lensed` |
| `shape` | `surface`, `card_min`, `control`, `capsule` (true selects capsules), `small` |
| `type_ramp` | `family`, `roles` (ten entries in `TextRole::ALL` order, each with `size`, `weight`, `tracking`, `line`) |
| `spacing` | All `Metrics` field names, design pixels |
| `palette` | All `Palette` color fields (RGBA), plus `monochrome` |
| `motion` | `interactive`, `open`, `layout`, `scroll` (each `mass`, `stiffness`, `damping`); `scrollbar_idle`, `scrollbar_fade` |
| `icons` | `family`, `vector` (true selects vectors), `weight` |
| `focus_ring` | `width`, `gap`, `opacity` (no base) |

Top-level `border` is an RGBA hairline, `false` to remove it, or `true` to use the separator. Omitted values inherit the base. Files set
the selected scheme palette; use the Rust `.palettes` builder for paired custom
palettes. Files may describe an unsatisfiable mix; call `check()` to inspect it.
`Style`, `Theme::new`, `material`, `Knobs`, `GlassMaterial::tuned`, `lensed`, and
the process-global active style remain compatibility APIs. In particular the old
`tuned(style, knobs)` still suppresses lens controls outside Liquid.

`gallery` writes `kit-mix-light.png` and `kit-mix-dark.png`: each row contains two
source presets followed by their mix. Material previews use a deterministic CPU
backdrop stand-in. `golden` enables their comparisons through `golden::check`.
