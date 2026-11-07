# The element tree (`stillkit::el`)

Phase 3 of the extraction plan: a declarative view, laid out with flexbox, that still ends as
today's `DisplayList`, so `Surface::present` keeps redrawing only the controls that changed.

```rust
use stillkit::el::{self, Align, App, El, col, el, text, token};
use stillkit::{Kind, Role, TextRole};

fn view(s: &Hello) -> El<Hello> {
    col().align(Align::Center).gap(token::GAP)
        .child(text(if s.on { "On" } else { "Off" }).font(TextRole::Title2).key("label"))
        .child(el().key("switch").role(Role::Switch).label("Light").checked(s.on)
            .w(40.0).h(24.0).radius(12.0)
            .fill(if s.on { token::ACCENT } else { token::FILL })
            .on_press(|s: &mut Hello| s.on = !s.on)
            .child(el().size(20.0).radius(10.0).fill(token::KNOB)
                .absolute(if s.on { 18.0 } else { 2.0 }, 2.0)))
}

el::run(App { title: "Hello", size: (320.0, 180.0), kind: Kind::App { min: (200, 120) },
              theme, state: Hello { on: false }, view })
```

Examples: `examples/el_hello.rs` (elements only) and `examples/el_sound.rs` (the plan's Sound
panel, its slider and switch drawn by today's kit through `paint`).

## Elements

| Builder | What |
|---|---|
| `el()`, `col()`, `row()`, `grid(n)` | containers: a column, a row, `n` equal columns |
| `text(s)` | one line in a type role (`.font(TextRole)`), cut with an ellipsis; `.wrap(lines)` wraps |
| `icon(Icon)` | a vector icon filling its square box (`token::ICON` unless sized) |
| `image(Arc<Image>)` | a bitmap at its own size unless sized; rounded with `.radius` |
| `paint(\|f, rect, id\| ..)` | draws with the kit (`Frame`) into its laid-out rect; `id` is the element's hit id, so a kit control drawn here hovers, presses and focuses like any element |

- **Layout:** `w`, `h`, `size`, `w_full`, `h_full`, `grow`, `min_*`, `max_*`, `gap`, `pad`,
  `pad_x`, `pad_y`, `align` (cross axis), `justify` (main axis), `absolute(x, y)`.
- **Style:** `fill`, `color` (text and icon ink), `radius` (clamped to a capsule), `shadow`
  (the theme's elevation), `opacity` (the element and everything in it).
- **Semantics:** `key`, `role`, `label`, `checked`, `disabled`, `focusable`.
- **Handlers:** `on_press(|s: &mut S| ..)` (click, Enter, Space, assistive invoke) and
  `on_drag(|s, x, y| ..)` (the pointer's place across the box, `0..=1`, while pressed).

Lengths are design pixels (`f32`) or theme tokens (`token::PAD`, `token::CONTROL`,
`token::RADIUS_CARD`...); colours are `Rgba` or tokens (`token::LABEL`, `token::CARD`...). Make
your own with `Len::Theme(|t| ..)` and `Paint::Theme(|t| ..)`. Tokens resolve when the tree is
laid out, so a new theme (`Runtime::set_theme`) restyles without running the view.

## How a frame is made

1. **Handlers** queued by input run on `&mut S` at the start of the frame. If any ran (or the
   app called `Runtime::update`), the **view** runs once.
2. **Layout** (Taffy, flexbox and grid) keeps one node per element identity: its key, or its
   place among unkeyed siblings, under its parent's identity. Only nodes whose style, text or
   children changed are restyled, so Taffy re-lays out only what they touch
   (`Runtime::touched`). Lengths are rounded like `Frame::px`; rects land on whole device
   pixels. Text measures through the thread's `Text` cache.
3. **Paint** walks the retained tree into a `DisplayList`. Pressable boxes always carry their
   wash and focus ring (transparent at rest), so **hover, press and focus repaint the retained
   tree without running the view**, and change one op.
4. **Hits** come from interactive elements (a handler or `focusable`) and from elements with a
   `role` or `label`; their role, label and checked state are what assistive technology
   reads. Hit ids are stable hashes of the identity. Elements with semantics but no handler are
   announced as disabled, so focus and hover skip them.

The keyed diff is recorded in `Runtime::changes()`: every keyed element that came, went, moved
or changed its fill or opacity, with its previous and target snapshot. Phase 4's animation
tracks retarget there (the `retarget` step in `src/el/runtime.rs`) and paint from the tracks instead of the targets.

## Hosting and idle

`el::run` opens a `Surface`, presents the first frame and then sleeps in
`MsgWaitForMultipleObjectsEx` until a message arrives: no timer, no ticker. A frame that
changes nothing paints nothing; a repainted but identical list costs the surface one
comparison. `el_hello` (release) measured after start-up settled, over 30 s: 0 context
switches on the UI thread, 0 ms CPU, 5.0 MB private; no GPU driver loaded (only the system
`d3d11.dll`/`dxgi.dll` the crate links for WARP sprites).

Apps with their own loop host a `Runtime` directly: feed `handle(&event)`, call `frame(dt)`,
and `present(&mut surface, origin)` when it returns `true`.

## Agent tooling

Every `el::run` binary is also headless tooling, on a virtual clock, drawn through the same
software target as a window:

```sh
app --snap out.png [--scale 2] [--at 1.5]    # the frame at 1.5 s
app --tree out.json                           # key, path, role, label, rect, fill, text, state
app --script steps.json --snap out.png        # replay input first
app --inactive                                # a window that never takes the focus
```

Scripts and dumps use design pixels. A script is a JSON array of timed steps:

```json
[{"at": 0.2, "move": [160, 106]}, {"at": 0.3, "down": [160, 106]}, {"at": 0.35, "up": [160, 106]},
 {"at": 0.5, "click": "switch"}, {"at": 0.6, "key": "Tab"}, {"at": 0.7, "key": "Enter"}]
```

Keys: `Tab`, `Shift+Tab`, `Enter`, `Space`, `Escape`, `Left`, `Right`, `Up`, `Down`, or a
virtual-key number. Tests use the same runtime: `frame(dt)`, `pointer_down/up/move(x, y)`,
`click(key)`, `key(vk)`, `tab(back)`, `find(key)`, `presented(key)`, `tree()`, `snap()`,
`play(&script, until)`, plus `view_runs()`, `paints()` and `damage()`.

## Not yet

- Animation (phase 4): `changes()` is the hook.
- Components (phase 5): today's kit controls come in through `paint`.
- The app window's modal resize loop: content catches up when the drag ends.
- Scrolling, text fields and per-element cursors.
