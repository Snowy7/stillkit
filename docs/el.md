# The element tree (`stillkit::el`)

Phases 3 and 4 of the extraction plan: a declarative view, laid out with flexbox and animated
by the runtime, that still ends as today's `DisplayList`, so `Surface::present` keeps
redrawing only the controls that changed.

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
            .child(el().size(20.0).radius(10.0).fill(token::KNOB).animate()  // springs across
                .absolute(if s.on { 18.0 } else { 2.0 }, 2.0)))
}

el::run(App { title: "Hello", size: (320.0, 180.0), kind: Kind::App { min: (200, 120) },
              theme, state: Hello { on: false }, view })
```

Examples: `examples/el_hello.rs` (elements only), `examples/el_sound.rs` (the plan's Sound
panel, its slider and switch drawn by today's kit through `paint`) and `examples/el_motion.rs`
(rows entering and leaving, a card changing size, a theme switch).

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
- **Motion:** `motion(spring)` (a `Spring` or `token::MOTION_LAYOUT` / `MOTION_INTERACTIVE` /
  `MOTION_OPEN`), `animate()` (the theme's layout spring), `transition(Transition::fade() |
  scale(s) | slide(x, y))`, `instant()`. See [Animation](#animation).
- **Handlers:** `on_press(|s: &mut S| ..)` (click, Enter, Space, assistive invoke),
  `on_press_with(|s, tx: &mut Tx| ..)` (`tx.instant()`, `tx.set_theme(..)`) and
  `on_drag(|s, x, y| ..)` (the pointer's place across the box, `0..=1`, while pressed; its
  changes land at once).

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
3. **Animation** retargets each element's springs to the new layout's targets, then advances
   them on the animation clock; frames while something moves do only this and paint.
4. **Paint** walks the retained tree into a `DisplayList`, drawing what each element shows now.
   Pressable boxes always carry their wash and focus ring (transparent at rest), so **hover,
   press and focus repaint the retained tree without running the view**, and change one op.
5. **Hits** come from interactive elements (a handler or `focusable`) and from elements with a
   `role` or `label`; their role, label and checked state are what assistive technology
   reads. Hit ids are stable hashes of the identity. Only interactive ones are
   `Hit::actionable`, so hover and focus skip a labelled heading, which is still announced
   (not as disabled).

The keyed diff is recorded in `Runtime::changes()`: every keyed element that came, went, moved
or changed its fill or opacity, with its previous and target layout snapshot.

## Animation

The view only says where things end up; the runtime springs what it shows toward that. The
view never runs for animation, and `Runtime::animating()` says whether anything still moves.

- **`.motion(spring)`** (or `.animate()`) animates the element and its subtree: each **edge**
  of its box on its own (a stationary edge never moves), fill, text and icon colour, opacity,
  radius and shadow. Edges are relative to the parent, so children ride along their parent.
  A descendant's own `.motion` overrides; `.instant()` turns it off for a subtree. Springs
  are solved exactly, so retargeting mid-flight keeps the position and velocity.
- **Colour** moves in premultiplied OKLab, without overshoot; text and icon ink move twice as
  fast as fills, so labels keep their contrast through a theme switch (`tx.set_theme`,
  `Runtime::set_theme`).
- **Presence:** `.transition(Transition::fade() | scale(0.9) | slide(-24.0, 0.0))`. An
  element added after the first frame (under a parent that was already there) enters from
  that pose; a removed one stays drawn, out of hit testing, until its exit settles, while its
  siblings close the gap. Coming back mid-exit reverses from where it is. Presence moves along
  the element's motion, or the theme's open spring.
- **No motion:** the first frame; `tx.instant()` and `Runtime::update_instant` (the whole
  change); drags; resizes and scale changes; `.instant()` subtrees.
- **Reduced motion** (`el::reduced_motion()`, Windows' *Animation effects* off; the host
  follows it, `--reduced` forces it): travel and radius snap, while opacity, colour and
  presence take a 0.15 s fade.
- **Time scale:** `Runtime::set_time_scale(0.25)` runs every animation at a quarter speed.
- On screen, positions are whole device pixels and colours 8-bit steps, so the slow tail of
  a spring repaints identical lists, which the surface does not draw. Damage during an
  animation covers only the ops that moved (`tests/el_motion.rs` checks the knob and the card).

`tests/golden/el-motion-*.png` are frame strips (0.1, 0.15, 0.2, 0.3, 0.45 and 1.5 s after a
click at 0.1 s), made through the same path as
`el_motion --script s.json --at 0.3 --snap out.png`.

## Hosting and idle

`el::run` opens a `Surface`, presents the first frame and then sleeps in
`MsgWaitForMultipleObjectsEx` until a message arrives. While `Runtime::animating()`, it holds
`Surface::animation_frames` (a display-rate timer thread posting frames); once everything
settles it drops them, and no timer or ticker remains. A frame that
changes nothing paints nothing; a repainted but identical list costs the surface one
comparison. `el_hello` (release) measured after start-up settled, over 30 s: 0 context
switches on the UI thread, 0 ms CPU, 5.0 MB private; no GPU driver loaded (only the system
`d3d11.dll`/`dxgi.dll` the crate links for WARP sprites). `el_motion` (release, 2026-11-07, 10 s
settle, then 30 s): 6 context switches in the whole process, 0 ms CPU, 6.9 MB private; after
an animation (Add invoked through UI Automation, so no input or focus) settled: 5 switches,
0 ms CPU, 8.0 MB private (the UI Automation adapter now has a tree), and no frame thread left.

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
app --reduced                                 # reduced motion, whatever the system says
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

- Components (phase 5): today's kit controls come in through `paint`.
- The app window's modal resize loop: content catches up when the drag ends.
- Scrolling, text fields and per-element cursors.
- Shared-element motion (`matched`), finite sequences, and theme elevation (the shadow colour
  changes at once on a theme switch; its strength springs).
- Damage is positional: when an exit ends, the ops after it shift, so that one frame redraws
  from the leaving element down.
