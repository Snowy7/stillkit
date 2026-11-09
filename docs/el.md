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
(rows entering and leaving, a card changing size, a theme switch), `examples/el_scroll.rs`
(a long list scrolling under a header that stays put), `examples/el_overlays.rs` (a
popover, a context menu, a tooltip, a dialog, a select-style menu and a toast stack) and
`examples/el_text.rs` (a form: a name, a password, a search field with a clear button and a
notes area).

## Elements

| Builder | What |
|---|---|
| `el()`, `col()`, `row()`, `grid(n)` | containers: a column, a row, `n` equal columns |
| `text(s)` | one line in a type role (`.font(TextRole)`), cut with an ellipsis; `.wrap(lines)` wraps |
| `icon(Icon)` | a vector icon filling its square box (`token::ICON` unless sized); `Icon::Path(&IconPath)` draws an app's own SVG path data on the 24-unit grid |
| `image(Arc<Image>)` | a bitmap at its own size unless sized; rounded with `.radius` |
| `arc(fraction)` | a ring or meter: a circle fitting its box, `fraction` of its span filled in `color` (`token::ACCENT`) over a track (`.track(paint)`, `token::FILL`; `.no_track()`), stroked `token::RING` wide (`.stroke(len)`) with round caps; `.arc_start(ArcStart::Top \| Right \| Bottom \| Left)` or any start, `.arc_angle(radians)` clockwise from twelve (`1.25 * PI` is half past seven), and `.arc_span(0..=1)` (`Left` with `0.5` is a semicircle meter; 7:30 with `0.75` a gauge); with motion the fraction springs |
| `paint(\|f, rect, id\| ..)` | draws with the kit (`Frame`) into its laid-out rect; `id` is the element's hit id, so a kit control drawn here hovers, presses and focuses like any element |
| `on_drop_files(|s, paths| ..)` + `dropping(|el| ..)` | files dropped from other apps: the innermost element under the drag with a handler takes the paths and, while files hover, its `dropping` look (no view run). Hosts route `Event::FilesHover`/`FilesLeave`/`FilesDropped` through `handle` (`Runtime::files_over`, `drop_files`) |
| `virtual_list(key, count, row, |range| rows)` | a vertical scroll view of `count` rows, each `row` design px tall, that builds only the rows near sight (a viewport either side) between spacers that keep the scroll extent exact; scrolling past what was built runs the view again in the same frame. For lists of 100,000 rows |
| `native(name)` | a hole (`Op::Hole`) onto the host's native GPU layer `name`: `Runtime::natives()` reports its visible box (cut by scroll views); the host places the layer with `Surface::native_layer` and `place_native_layer` and draws into it with its own device (a `CAMetalLayer` on macOS; a child `HWND` under a GPU surface on Windows; not yet on Wayland). Children and overlays draw over it. See `examples/native_layer.rs` |
| `text_input(key)`, `text_area(key)` | a text field: one line, or wrapped lines that scroll; see [Text fields](#text-fields) |

- **Layout:** `w`, `h`, `size`, `w_full`, `h_full`, `grow`, `min_*`, `max_*`, `gap`, `pad`,
  `pad_x`, `pad_y`, `align` (cross axis), `justify` (main axis), `absolute(x, y)`. Every box
  is a containing block: an absolute child is placed from its parent's corner and its
  `w_full`/`h_full` are the parent's box, padding included (as CSS's), at any depth.
- **Style:** `fill`, `color` (text and icon ink), `radius` (clamped to a capsule), `shadow`
  (the theme's soft shadow at `ShadowLevel::Raised`), `shadow_level(ShadowLevel::Menu)` (a
  menu's, a dialog's...), `box_shadow(SoftShadow)` (a custom one), `opacity` (the element and
  everything in it), `focus_within(|el| el.fill(..))` (a look while focus is on or in it). `cascade_ink()` makes its ink (own or a state look's) flow to descendants that set none, as CSS's `color` inherits, so a button's label follows its hover colour. `hovered(|el| ..)` and `pressed(|el| ..)` are the same for the pointer over it (or an interactive element inside it) and a press on it; pressed lies over focus, focus over hover, and none runs the view. A
  popup's window grows by its shadows' reach.
- **Text:** `font(TextRole)`, and `family(..)`, `weight(..)`, `text_size(..)` (its line scales with it) and `text_align(TextAlign::Start | Center | End)` over the role; fields take them too.
- **Pointer:** `cursor(Cursor)`: the shape over it (innermost wins; fields show an I-beam), kept through a press that began on it. `Runtime::cursor()` reads it; `el::run` applies it. Shapes: arrow, text, hand, move, resize (both ways), grab, grabbing, crosshair, not allowed, copy.
- **Transform:** `rotate(radians)` and `spin(period)`. See [Rotation and
  spinners](#rotation-and-spinners).
- **Semantics:** `key`, `role`, `label`, `checked`, `disabled`, `focusable`, `description`,
  `value` (the value as text, "40%"), `selected`, `expanded`, `range(min, max, step, value)`,
  `invalid(bool)`, `active_descendant(key)`. See [Accessibility values](#accessibility-values).
- **Keyboard:** `focus_group(Orientation)`, `select_follows_focus()`; `tx.focus(key)`,
  `tx.blur()`. See [Keys and focus](#keys-and-focus).
- **Scrolling:** `scroll(Axis::Vertical | Horizontal)`. See [Scroll views](#scroll-views).
- **Overlays:** `overlay(popover(..) | menu(..) | dialog(..) | toasts(..))`, `tooltip(text)`.
  See [Overlays](#overlays).
- **Motion:** `motion(spring)` (a `Spring` or `token::MOTION_LAYOUT` / `MOTION_INTERACTIVE` /
  `MOTION_OPEN`), `animate()` (the theme's layout spring), `transition(Transition::fade() |
  scale(s) | slide(x, y))`, `instant()`. See [Animation](#animation).
- **Raw pointer:** `on_pointer(|s, e: &PointerEvent, tx| -> bool)`: `Hover`, then `Down`, `Move`… `Up` or `Cancel` for a press of any button that began on it (it holds the pointer until released, wherever it goes), with design-px `at`, `local`, `delta` (moves within a frame merge), `origin`, `size`, `mods` and `clicks`; return whether the state changed (a hover that changed nothing runs no view). `on_wheel(|s, w: &WheelEvent, tx| -> bool)` takes the wheel over it before scroll views. Hosts pass held modifiers with `Runtime::set_modifiers` (`el::run` does).
- **Handlers:** `on_press(|s: &mut S| ..)` (click, Enter, Space, assistive invoke),
  `on_press_with(|s, tx: &mut Tx| ..)` (`tx.instant()`, `tx.set_theme(..)`) and
  `on_drag(|s, x, y| ..)` (the pointer's place across the box, `0..=1`, while pressed; its
  changes land at once), `on_key(|s, key: KeyCode, tx| -> Handled)` and
  `on_step(|s, delta: f64| ..)` (a `range` stepped by keys or assistive technology),
  `on_char(|s, c: char, tx| -> Handled)` (a typed character, after a focused field),
  `on_input(|s, text: &str| ..)` and `on_submit(|s, text: &str| ..)` (text fields),
  `on_context_menu(|s, x, y| ..)` (secondary click or the Menu key, design px in the window)
  and `on_timeout(after, |s| ..)` (once, `after` the element appears).
  `tx.scroll_into_view(key)` shows an element once the view has run; `tx.focus(key)` and
  `tx.blur()` move keyboard focus then.

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
   (not as disabled). Announced content inside an enabled control (a table's cells in their
   row) keeps its own hit for assistive technology, but the pointer goes through it: hover,
   presses and tooltips land on the control. A control inside a control still takes its own.

Focus rings come from the theme's `FocusRing` (width and gap in design px, the accent at its
opacity); the default ring, at rest, draws exactly what `Frame::focus_ring` does.

**Interaction feedback eases** (`el/feedback.rs`): hover and press washes ease in and out
along the theme's `motion.interactive` spring (critically damped, so a wash never
overshoots), and a focus ring fades up while growing from the box's edge out to its gap
(along the same spring). Feedback starts one display frame in, so the frame after a hover,
press or Tab already shows it moving. It runs on the runtime clock (time scale included) and
never runs the view; an entry exists only while a control is hot or easing back, so a settled
window asks for no frames. Reduced motion lands every change at once. Kit controls drawn
through `paint` draw their own washes, which still snap. `tests/el_focus.rs` checks the
easing, its end (no frames, no timer), reduced motion, focus by key, focus-within, the new
semantics and cells passing presses to their row.

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

## Rotation and spinners

```rust
icon(Icon::ChevronRight).rotate(if s.open { FRAC_PI_2 } else { 0.0 }).animate()  // a disclosure
arc(0.25).size(20.0).no_track().spin(Duration::from_secs(1))                   // a spinner
```

- **`.rotate(radians)`** turns the element and its subtree clockwise about its centre at
  draw time: layout keeps the upright box, while its ops, its hits (their bounding box) and so
  its damage are turned. With motion the angle springs (it snaps under reduced motion). The
  renderers have no transform: `el/rotate.rs` maps each op onto a turned op of its own kind.
  Arcs (their start), lines and circles turn exactly at any angle, every box at quarter
  turns, and icons are drawn as their stroked outline (lines with round joins and caps) while
  turned. Text, images, filled icons and boxes at other angles stay upright about their
  turned centre, and a clip at another angle grows to the turned box.
- **`.spin(period)`** adds a turn every `period` on the runtime clock. A spinner moves only
  while it can be seen: laid out and live (not leaving), inside the window and every scroll
  view around it, not under an open dialog, its window shown (`Runtime::set_visible`; the
  host follows `Event::VisibilityChanged` on Windows and macOS), and motion not reduced
  (there it rests at its own angle). Otherwise it holds where it was, and frames run only
  while something visible spins, so a spinner scrolled away, under a dialog or in a hidden
  window leaves no ticker. Its angle moves in thousandths of a radian. The view never runs.

`tests/el_spin.rs` checks the spinner's clock, every stop and the ticker's end, a turned box's
drawing, hits, pixels and damage, a chevron springing a quarter turn as its outline, and an arc
from half past seven, by its pixels.

`tests/golden/el-motion-*.png` are frame strips (0.1, 0.15, 0.2, 0.3, 0.45 and 1.5 s after a
click at 0.1 s), made through the same path as
`el_motion --script s.json --at 0.3 --snap out.png`.

## Text fields

```rust
text_input("name").placeholder("Your name").max_len(32)
    .value(s.name.as_str()).on_input(|s, t| s.name = t.into()).on_submit(|s, t| s.go(t))
text_input("password").secure().value(s.password.as_str()).on_input(|s, t| s.password = t.into())
text_area("notes").rows(5).value(s.notes.as_str()).on_input(|s, t| s.notes = t.into())
```

- **The view owns the string.** `.value(..)` is what the field shows; each edit (typing,
  deleting, a paste, undo, an IME commit, assistive technology's Set Value) goes to
  `on_input` with the new text, the view runs, and whatever `.value` it gives back is what the
  field shows. Give the old text back to refuse an edit (the caret and undo history go back
  too), or another to transform it. Without `.value` the field keeps its own text.
- **Builders:** `placeholder`, `secure` (bullets; no copy or cut; no text for assistive
  technology, in `--tree` or in the list), `max_len(chars)`, `read_only` (focus, select and
  copy only), `select_all_on_focus`, `rows(n)` (a text area's height when unsized, 3),
  `font(TextRole)`, `color`. Padding, fill and radius are the element's own. Caret and
  selection come from `token::CARET` and `token::SELECTION` (the theme's accent), the
  placeholder from `token::TERTIARY`. A field shows its focus ring whenever it has focus.
- **Editing:** a click places the caret, a drag selects, a double click selects a word and a
  triple click the line (dragging after them extends by words or lines). Arrows, Ctrl+arrows
  by word (Windows' stops: the start of the next word), Home and End (Ctrl: the whole
  text), Up and Down and Page Up and Down in a text area (keeping their column), Shift to
  extend, Backspace and Delete (Ctrl: by word), Ctrl+A, Ctrl+Z, Ctrl+Y and Ctrl+Shift+Z, Ctrl+C,
  Ctrl+X and Ctrl+V (and Ctrl+Insert, Shift+Delete, Shift+Insert). Typing undoes a word at a
  time. Moves are by grapheme. Enter submits a `text_input` (Ctrl+Enter a `text_area`, where
  Enter is a new line); without `on_submit` it goes on to the ancestors.
- **Layout:** a field is one line of its role tall (a text area `rows` lines) plus its
  padding, and only as wide as it is given. A long line scrolls sideways to keep the caret in
  sight; a text area wraps at word boundaries and scrolls in its own scroll view (the wheel,
  the overlay scrollbar, the caret kept in sight), which never runs the view either.
- **What runs:** caret moves, selection, scrolling and the blink only repaint. The caret
  blinks (530 ms on, 530 off) on the runtime clock while its window has the keyboard, and
  holds still, shown, after 5 s without input, as Windows' caret does; `wake_at` holds the
  next blink, and nothing once it holds still or no field has focus.
- **Clipboard:** `Runtime::set_clipboard(impl Clipboard)`. Headless runtimes and tests keep
  a `MemoryClipboard` (a script never touches the user's clipboard); `el::run` gives Linux
  runtimes the seat's selection. On Windows the focused field's RichEdit uses the system
  clipboard itself.
- **IME:** `Runtime::preedit(text, cursor)` draws a composition inline, underlined, at the
  caret, and `type_text` commits; `caret_rect()` is where candidate windows go.
- **Per platform** (`Runtime::sync_text_input`, called by `el::run` before each frame):
  - *Windows:* while a field has focus in a window, the existing windowless RichEdit
    (`TextField`) types into it: the system IME (TSF, its composition inline, candidates at
    the caret), caret placement, mouse selection, undo and the system clipboard. The runtime
    keeps focus, drawing order (`Op::TextField`, with the caret drawn from
    `TextField::caret`) and the key policy: the surface asks `el::edit::command` which keys
    the field takes; the rest reach the app as `Event::Key` (Tab and Escape always). One
    RichEdit exists at a time, made on focus, dropped on blur; unfocused fields draw through
    the runtime's editor. Apps' own `TextField`s behave as before.
  - *Linux (Wayland):* the runtime's editor, drawn by stillkit. Characters come from
    xkbcommon with the locale's compose table (dead keys, Compose sequences) as `Event::Char`
    after their key; the clipboard is `wl_data_device` (text/plain;charset=utf-8,
    UTF8_STRING, text/plain); the IME is text-input-v3 (enabled with the caret's rectangle
    while a field has focus, its preedit drawn inline). **Compile-checked only so far:**
    see [Not yet](#not-yet).
- **Accessibility:** a field is a text input (multi-line, or a password input), named by its
  `label` or placeholder. Its text is its value (Set Value types it), and
  `Semantics::Field` gives one AccessKit text run per visual line, with each character's
  place and width, word starts, and the caret and selection. A secure field gives neither.
- **Headless:** `type_text(s)`, `key_with(vk, Modifiers::CTRL)`, `preedit`, and the
  script's `{"type": "text"}` and `{"key": "Ctrl+Shift+Left"}`. `Shown` (and `--tree`)
  carry a field's text and `selection` (anchor, caret). `tests/el_text.rs` checks typing,
  word moves, undo, paste, multi-click, a refused value, Enter and Tab, a secure field, the
  runs, and the blink; the RichEdit path has its own test (a hidden window, never focused).

## Keys and focus

A key goes to the focused element, then up through its ancestors (the root when nothing has
focus). A focused text field gets it first: it takes what it edits with (and every key that
types a character, which arrives apart as `type_text`), and refuses the rest, which go on
as below; Escape and Tab always go on. Characters go to the focused field, else to the
focused element's and its ancestors' `on_char`. Handlers read the modifiers held through
`tx.modifiers()`; the host sends every key with them (`Runtime::key_with`). Each element in turn gets its `on_key`; a key it uses (`Handled::Yes`) re-runs the
view and goes no further. Unused, the element's built-in keys come next:

- **A range** (`range` + `on_step`, while it has focus): Right and Up add a step, Left and
  Down take one, Page Up and Page Down a tenth of the range in whole steps, Home and End go to
  the ends. `on_step` gets the change, already within `min..=max`; keys faster than frames add
  up from the latest value. Assistive technology's Increment, Decrement and Set Value come the
  same way (`Runtime::accessibility_action`; the window host routes them).
- **A focus group** (`focus_group(o)`, while a member has focus): the arrows along `o` move
  between its enabled members, wrapping; Home and End go to the ends. With
  `select_follows_focus()` they also press where they land (radio groups, segmented controls).
  Members are its focusable descendants outside nested groups.
- **A scroll view**: Page Up and Page Down while focus is anywhere inside; arrows along its
  axis, Space, Home and End while the view itself has focus.

Then the defaults: Enter and Space press, Escape dismisses an open overlay (else drops a
visible focus), the Menu key opens a context menu, and arrows move focus like Tab (inside an
open menu or dialog, Tab and arrows stay in it). Tab (and the arrows) walk the retained tree's **focus stops** in paint order,
wrapping: a focus group is one stop, entered on its selected or checked member (else its
first, its last going back); a scroll view with focusable content is no stop itself. Stops
hidden in a scroll view are reached, and focus arriving there scrolls them into view. Key
handlers run as the key arrives (after any queued presses), not at the next frame, so they can
say whether they used it.

**Focus by key.** `tx.focus(key)` (from any handler) and `Runtime::focus(key)` (from the
host) move focus to the first focusable element keyed `key` once the view has run, and scroll
it into view; `tx.blur()` and `Runtime::blur()` take it away. A key handler's move shows the
ring; others keep whether it showed. A key no enabled, focusable element has leaves focus
where it is. A tree's Left to its parent and Right into its first child, a search field's
Escape, a combobox's Down into its list are each one call.

**Focus-within.** `.focus_within(|el| el.fill(token::FILL))` gives an element another look
(fill, colour, radius, shadow, opacity below 1) while focus is on it or anywhere inside it.
Focus moving in and out retargets its look without running the view, springing along its
motion.

## Accessibility values

`Runtime::semantics()` holds what each hit item says beyond its name, role and checked state,
and `Runtime::present` hands it to the surface (`Surface::set_accessibility_semantics`), so
`Hit` stays as it was. A `range` is `Semantics::Range` (UI Automation's RangeValue, with
increment, decrement and set when it steps); `description`, `value`, `selected` and
`expanded` are one `Semantics::Detail`, with `invalid` (`El::invalid`: AccessKit's
invalid, for a field with an error; pair it with a description saying why) and
`active_descendant` (`El::active_descendant(key)`: the hit of the first element keyed `key`,
such as a combobox's highlighted row in its list overlay, which assistive technology reads as
if it had focus while the field keeps it; left out while that element has no hit in the
list). `--tree` dumps them too: `range` (`[min, max, value]`), `value`, `description`,
`selected`, `expanded`, `scroll` (a scroll view's offset, design px), `invalid` and
`active_descendant` (the key).

## Scroll views

`.scroll(Axis::Vertical)` (or `Horizontal`) makes a box a viewport: its children lay out at
their own length along the axis, in a content node inside the box that carries the box's
padding and gap. Give the box a size (`h`, `grow`, `max_h`) for it to be smaller than them.
The offset never reaches layout: paint moves the children back by it, inside one
`Op::Clipped` the size of the box, so **scrolling neither runs the view nor lays anything
out**, and damage stays inside the viewport. Hits are cut to it and dropped outside it (the
focused one stays, so Enter still finds it).

- The wheel scrolls the innermost view under the pointer that can move: a notch glides
  `scroll::NOTCH` design px along the theme's `motion.scroll`, a touchpad's small deltas move
  at once; keys glide too (see above). Reduced motion lands at once.
- Keyboard focus arriving on a hidden child scrolls every view around it just enough to show
  it (with the content's padding when it is near an end); so do `tx.scroll_into_view(key)`
  and `Runtime::scroll_into_view(key)` once the view has run.
- The overlay scrollbar (the theme's `scrollbar` metrics and colour) shows while it scrolls,
  then fades after `motion.scrollbar_idle` over `scrollbar_fade`. Frames keep coming until it
  has faded (painting nothing while it holds still), then none.
- A shorter content pulls the offset back at once; hover follows content scrolling under a
  still pointer. The clip is rectangular: round what scrolls, not the view's corners.

`Runtime::scroll_offset(key)` says where a view rests; `wheel_at(x, y, delta)` and the
script's `{"wheel": [x, y, delta]}` drive it headless. `tests/el_scroll.rs` checks clipping,
damage, the ticker's end and revealing.

## Overlays

Content above the window's content, declared on its trigger and opened by state:

```rust
button("share").on_press(|s| s.share = !s.share)
    .overlay(popover(card().key("share-panel")..).open(s.share).on_dismiss(|s| s.share = false))
canvas.on_context_menu(|s, x, y| s.menu = Some((x, y)))
    .overlay(s.menu.map(|(x, y)| menu(items).at(x, y).on_dismiss(|s| s.menu = None)))
button("copy").tooltip("Copy the link")
root.overlay(dialog(panel).open(s.confirm).on_dismiss(|s| s.confirm = false))
    .overlay(toasts(col().children(toasts.map(|t| toast(t).on_timeout(TOAST, ..)))))
```

| Kind | Placed | Focus | Goes away on |
|---|---|---|---|
| `popover(el)` | under its trigger, centred | none (`.take_focus()` to move it in) | Escape, a press outside (which still lands), deactivation |
| `menu(el)` | under its trigger's left edge, or `.at(x, y)` | its selected (else first) item; arrows move through items, Tab stays inside; the rest of the window takes no pointer | Escape, a press outside (which only closes it), deactivation |
| `dialog(el)` | centred over a dimmed backdrop (`token::SCRIM`) | its selected (else first) control, trapped | Escape, a press on the backdrop |
| `toasts(stack)` | the window's bottom-right corner | never | its toasts' own `on_timeout` |
| `.tooltip(text)` | above its trigger | never; no hits | leaving, a press, the wheel, Escape |

- **Open is presence:** an overlay is open while it is in the view and `.open(true)` (the
  default). Closing removes it; its `.transition` (popovers and menus default to `scale(0.96)`,
  tooltips and dialogs fade) plays out as an exit. `on_dismiss(|s| ..)` is how the runtime asks
  the state to close it; a press on the trigger is never "outside", so the trigger's own
  toggle closes it without a dismissal reopening it. An anchor (`.anchor(key)`) or trigger
  that goes away dismisses too, in the same frame.
- **Placement:** `.side(Side::Top | Bottom | Left | Right)` with `.align(Start | Center |
  End)` and `.offset(len)`. It flips to the other side when only that fits, flips `Start` and
  `End` at an edge, then shifts to stay 8 design px inside the window. `.match_width()` makes it
  at least as wide as its anchor (a select). Placement moves the overlay's laid-out subtree, so
  Taffy never lays it out twice (`match_width` adds one pass when the anchor's width changes).
  An anchor inside a scroll view is followed as it scrolls (one relayout per scrolled frame
  while such an overlay is open).
- **How it works:** after each view run the runtime lifts every overlay out of the tree and
  appends the open ones to the root as its last children ("entries", nested overlays after the
  one holding them, then toasts, then the tooltip), so layout, presence, hit testing, keys,
  focus groups and `find`/`click`/`tree` treat them like any element. Escape goes to the
  focused element's `on_key`s first, then to the overlay holding focus, else the topmost one.
- **Focus:** an overlay that takes focus remembers where focus was (or its trigger) and gives
  it back when it closes, if focus was inside it or gone. Tooltips and toasts never take it.
- **Tooltips** are runtime state, not view state: a pointer rest (or keyboard focus arriving)
  on a trigger arms `TOOLTIP_DELAY` (0.5 s) on the runtime clock; when it fires the tooltip is
  appended and laid out **without running the view**; moving straight from a shown tooltip to
  another trigger opens the next at once. After a press, the wheel or Escape, the trigger shows
  none until the pointer leaves it.
- **Timers:** `Runtime::wake_at()` is when the runtime next needs a frame though nothing moves
  (a tooltip's delay, an `on_timeout`); `None` once nothing waits. `on_timeout` fires once per
  appearance and is forgotten when its element goes.
- **Accessibility:** a trigger announces what it opens (`Detail::popup`: menu or dialog),
  whether it is open (`expanded`) and, while open, the panel it controls (`Detail::controls`,
  UIA's ControllerFor). Popover and dialog panels are dialogs, menus menus; a dialog's panel is
  `modal`, and while it is open the hits before it (the background) leave the list: nothing
  under it can be pointed at, focused or read. A tooltip's text is its trigger's description.
  An element with `Role::Status` (a toast) is a polite live region.
- **Escaping the window:** the host can hand the runtime the screen around the window
  (`Runtime::set_screen`, device px from the window's top-left; `el::run` gives the work area of
  the window's monitor). A menu, popover or tooltip that the window would squeeze onto its
  anchor (or that is larger than the window) is then placed against the screen instead, and
  `Runtime::popups()` lists it with its own display list. `el::run` shows each in a
  `Kind::Popup { activate: false }` surface at its rect (pooled, hidden and released when it
  closes, kept through its exit), so the keyboard stays with the window, and feeds its pointer
  input back offset into the window's pixels. `list()`, `snap()` and `tree()` still hold it;
  `present()` leaves it out. Components don't know which way their overlay went.

Headless, `{"right": [x, y]}` right-clicks and `{"key": "Menu"}` presses the Menu key;
`right_click(x, y)` and `wake_at()` drive tests. `tests/el_overlays.rs` checks placement and
flips, every dismissal path, focus trap and return, the tooltip's delay, toast timers
disarming, the relations, hover never running the view, and popup escape. Live (release,
2026-11-07, shown inactive, driven by its own timer only): the select menu opened in a 150 x 328
popup below its button, past the window's bottom edge; a 2 s `on_timeout` closed it with no
input; then 0 ms CPU over 5 s. Idle private memory is unchanged against the base commit
(`el_hello` 5.1-5.2 MB, `el_motion` 6.6-6.8 MB, `el_overlays` 6.6 MB).

## Hosting and idle

`el::run` opens a `Surface`, presents the first frame and then sleeps in
`MsgWaitForMultipleObjectsEx` until a message arrives. While `Runtime::animating()`, it holds
`Surface::animation_frames` (a display-rate timer thread posting frames); once everything
settles it drops them, and no timer or ticker remains. A pending runtime timer
(`Runtime::wake_at`: a tooltip's delay, a toast, the caret's blink) gets one wake through the
platform host, `host::wake_at(deadline)`: on Windows the message wait's own timeout
(`MsgWaitForMultipleObjectsEx`, no thread or timer object), on Linux a timerfd polled beside
the display fd and disarmed when nothing waits. With no deadline the wait is infinite. A frame that
changes nothing paints nothing; a repainted but identical list costs the surface one
comparison. `el_hello` (release) measured after start-up settled, over 30 s: 0 context
switches on the UI thread, 0 ms CPU, 5.0 MB private; no GPU driver loaded (only the system
`d3d11.dll`/`dxgi.dll` the crate links for WARP sprites). `el_motion` (release, 2026-11-07, 10 s
settle, then 30 s): 6 context switches in the whole process, 0 ms CPU, 6.9 MB private; after
an animation (Add invoked through UI Automation, so no input or focus) settled: 5 switches,
0 ms CPU, 8.0 MB private (the UI Automation adapter now has a tree), and no frame thread left.

Apps with their own loop host a `Runtime` directly: feed `handle(&event)`, call `frame(dt)`,
and `present(&mut surface, origin)` when it returns `true`. Workers wake such a loop with
`stillkit::waker()` (made on the UI thread; `Waker::wake` from any thread ends the current or
next `host::wait`, coalesced into one native message: an `NSEvent` on macOS, a thread message on
Windows, an eventfd beside the display on Linux). Drain your channels after every wait.

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

Keys: `Tab`, `Enter`, `Space`, `Escape`, `Left`, `Right`, `Up`, `Down`, `PageUp`,
`PageDown`, `Home`, `End`, `Menu`, `Backspace`, `Delete`, `Insert`, a letter or digit, or a
virtual-key number, each with any of `Ctrl+`, `Alt+` and `Shift+` (`Ctrl+Shift+Z`);
`{"type": "text"}` types into the focused field; `{"wheel": [x, y, delta]}` turns
the wheel (120 a notch, positive up); `{"right": [x, y]}` clicks the secondary button. Tests
use the same runtime: `frame(dt)`, `pointer_down/up/move(x, y)`, `right_click(x, y)`,
`click(key)`, `key(vk)`, `tab(back)`, `wheel_at(x, y, delta)`, `find(key)`, `presented(key)`,
`tree()`, `semantics()`, `scroll_offset(key)`, `wake_at()`, `popups()`, `snap()`,
`play(&script, until)`, plus `view_runs()`, `paints()` and `damage()`.

## Not yet

- Components (phase 5): today's kit controls come in through `paint`.
- The app window's modal resize loop: content catches up when the drag ends.
- Per-element cursors (an I-beam over fields comes from RichEdit while one has focus on
  Windows, nowhere else).
- Text fields, what is left:
  - **Linux is compile-checked only.** Untested on a compositor: characters and compose,
    modifiers, the `wl_data_device` clipboard (copy, and paste from another client or our
    own selection), text-input-v3 (enable, cursor rectangle, preedit, commit), the timerfd
    wake, and the stillkit-drawn caret, selection and preedit underline there. Linux has no
    key repeat yet (wl_keyboard's repeat_info is ignored), so holding Backspace deletes one.
  - **Windows:** RichEdit draws the selection in the system's highlight colour, not
    `token::SELECTION`, and masks a password with `*`. A text area's scroll while focused is
    RichEdit's own (no overlay scrollbar until it loses focus, and the offsets don't carry
    over). Undo history lives in the RichEdit, so it ends when the field loses focus.
  - Caret placement is by characters on one shaped line: mixed-direction (bidi) text gets
    visual caret stops but logical selection rects in the runtime's editor (RichEdit, on
    Windows, handles bidi itself). No drag and drop of text, no context menu (cut, copy,
    paste) and no spell checking. Shift+click doesn't extend yet.
- Overlays, what is left:
  - **Assistive technology can't open or close a trigger yet.** A trigger that reports
    `expanded` is an ExpandCollapse control in UI Automation, which hides Invoke, and
    `src/platform/windows/a11y.rs` drops `Action::Expand`/`Collapse`. Map both to the Click
    path there (they press the trigger). Keyboard and pointer work.
  - **Popups:** drawn at the window's scale (a monitor with another DPI gets the window's
    pixels), against the work area of the window's monitor only, and their accessibility
    tree is separate from the window's (their hits and names, without the window's
    semantics). A root that scrolls keeps its overlays in the window.
  - Tooltips are text; rich hover cards are popovers the app opens. No hover grace period
    beyond moving straight across, and toasts don't pause under the pointer.
  - No cascading submenus: an overlay declared inside a menu opens above it (`side(Right)`),
    but hover-to-open and the safe triangle are component work. A select opens below its
    trigger, not with its selected item over it.
  - Overlays aren't capped to the window or screen: give a long one `max_h` and a scroll
    view.
- Rotation has no renderer transform: text, images, filled icons and boxes at angles other
  than quarter turns stay upright (see [Rotation and spinners](#rotation-and-spinners)). A
  turned icon is its outline in lines and discs, so a translucent ink darkens slightly where
  they overlap. A transform op (or a rotation on `Op::Icon`) would make every op exact.
- `spin` is the one continuous channel (a turn); indeterminate bars that slide need a clip
  builder and a `continuous` phase, not there yet. Linux doesn't tell the host when the window
  is hidden, so spinners there stop only for the other reasons.
- Scroll views have no rubber band, snapping, `Both` axis or momentum beyond a notch's glide;
  Shift+wheel does not scroll sideways (the event carries no modifiers).
- Shared-element motion (`matched`), finite sequences, and theme elevation (the shadow colour
  changes at once on a theme switch; its strength springs).
- Damage is positional: when an exit ends, the ops after it shift, so that one frame redraws
  from the leaving element down.

## Selectable documents

`text("Version 1.0").selectable()` enables drag selection, double-click words,
triple-click paragraphs, Shift-click extension, Ctrl+A and Ctrl+C (Command+A/C
also work). Selection uses `token::SELECTION` and exposes character ranges to
assistive technology. It does not run the view or start a caret timer.

Use `col().selectable().children(...)` for a document: its display text shares
one selection and copies in tree reading order, with a newline between elements.
Text fields retain their independent editing state; interactive controls are excluded.
Descendants added before or after `.selectable()` participate. Give dynamic documents stable element keys.
Selectable labels wrap fully by default; `.wrap(n)` still limits visible lines.
The window host uses `el::SystemClipboard`; headless runtimes can inject a
`Clipboard` with `Runtime::set_clipboard`.
