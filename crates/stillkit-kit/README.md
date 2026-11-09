# stillkit-kit

Declarative components for `stillkit::el`. The default crate depends only on stillkit;
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
CalendarMonth (Gregorian, Monday first), TextField (native editing, value/placeholder/focus).

Selection and pointer/keyboard activation use runtime checked, disabled, press,
hover and focus state. Disabled controls ignore input and dim the whole subtree.
Radio and segmented options have one Tab stop and wrapping arrow selection.
Sliders expose ranges with arrow/Page/Home/End and assistive stepping. Components without a callback
are display-only, as specified by today's el semantics. Calendar clamps invalid
month/year inputs; values clamp to 0..=1 (NaN becomes zero). Slider steps must be
positive finite fractions of the range. Calendar exposes `on_month_change` with
a (year, month) pair; rebuild from that value. `today(day)` is supplied by the host only for the current month.

Ring uses the public arc primitive. Overlay wrappers are a separate component family.
TextFieldFrame remains a deprecated compatibility frame.

## Needed from el

- **Interaction wash motion:** connect el's retained hover/press painting to the
  existing interaction animator and theme interactive token (including reduced
  motion and idle disarming). `Frame::wash` supports it, but el does not supply it;
  `.motion` cannot observe hover/press state from a component view.
- **Absolute percentage sizing:** nested `.absolute(0., 0.).w_full()` currently
  resolves against the headless root width instead of its padded parent. The
  segmented pill uses a zero-height flex rail to avoid this; no el workaround was
  added outside the kit.
- **Text semantics:** public invalid-state and active-descendant setters are missing.
  Text errors use accessible descriptions; highlighted results use a visual fill.
- **Focus control and focus motion:** public focus/blur-by-key and focus ring motion
  and focus-within styling are unavailable. Search Escape delegates blur to the
  runtime; a visible keyboard focus blurs, but pointer-only focus may remain.
  The ring uses the theme as el draws it.
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


## Overlays

Implemented in separate modules: `Menu`, `MenuItem`, `ContextMenu`, `Select`,
`Popover`, `Dialog`, `AlertDialog`, `Toast`, `Toaster`, and `Tooltip`.
These supersede the older deferred-overlay notes above. All surfaces resolve theme
values; no component inspects a look name. Existing public contrast helpers are
reused without changes; overlay-specific surface/ink helpers live in a new module.

`Menu::new().item(MenuItem::new("Copy").on_press(..)).on_dismiss(..).overlay()`
returns an el overlay to attach to a trigger. Configure `open`, `side`, `align`,
`anchor` and `offset` on that overlay. `Menu::build()` returns content for custom
composition; its dismissal callback then runs on activation only. Rows support
`icon`, `shortcut`, `checked` or `radio`, and `disabled`; menus support `section`
and `separator`. Runtime's vertical focus group skips disabled rows, wraps with
arrows and handles Home/End; Enter/Space activate. With `on_dismiss`, an action
runs first and then closes. Use stable item keys when changing the menu contents.
`ContextMenu::attach(target, point, menu, on_open)` wires window-space points from
secondary clicks or the Menu key. The model stores `Option<(f32, f32)>`.

`Select::new(label, options, selected_index, open, on_open, on_change).key(..)`
combines a value-bearing ComboBox trigger with a match-width radio menu.
Opening focuses the checked option, and selection calls `on_change` then closes
through `on_open(false)`. Enter/Space toggle it; Down opens it (including Alt+Down
when delivered as Down). Character typeahead uses `on_char` (case-insensitive initial-letter cycling),
ignoring Ctrl/Alt/Meta combinations; it also works while the menu is open.
An invalid selected index displays an empty value. Supply unique select keys when
composing multiple selects.

`Popover::new(body_el).title(..).actions(action_row).build()` returns elevated
content to wrap in `el::popover(..)`. Title and actions are optional.
`Dialog::new(title, body, on_dismiss).confirm(label, action).build()` returns a
modal el overlay; apply `.open(model.open)` and attach it to the opening button.
Cancel is selected for safe initial focus. Both buttons use the existing Button;
confirmation runs its action and then dismisses, while Escape, Cancel and backdrop
presses dismiss without confirmation. `destructive(true)` changes confirmation
emphasis. `AlertDialog` uses AlertDialog semantics and destructive emphasis with
the same safe focus. Runtime traps focus and restores the trigger on closure.

`Toast::new(unique_key, message).icon(..).action(label, callback)` builds a Status
node (the host maps Status to a polite live region). `.timeout(duration, dismiss)`
returns the element with a one-shot `on_timeout`; the callback must remove the
keyed toast from model state. A key represents one notification lifetime: do not
reuse it for a replacement notification. `Toaster::new().child(toast_el).build()`
returns the bottom-right stack overlay. No kit clock, thread or recurring timer
is created. `Tooltip::attach(trigger, text)` delegates to `.tooltip(text)`;
el supplies elevated fill, Footnote type, a small radius, shadow, delayed hover
and focus opening, accessible description, and no tooltip hit target.

Overlay panels use `MOTION_OPEN` and scale/fade presence; toasts slide/fade from
the bottom. el owns reduced motion and idle disarming. **Still needed from el:**
transition origins tied to the final anchor side after flips (the current scale
primitive only scales about the centre), and tooltip style/motion overrides if
custom type or presence beyond el's themed default is needed. Tooltips currently
fade using el's open spring automatically. Popup surfaces and boundary flips are
entirely runtime-owned; kit creates no native windows.

`cargo run -p stillkit-kit --example kit_overlays` writes twelve
`docs/media/kit/overlays-<look>-<scheme>.png` sheets: all five presets in light and
dark plus One UI with Liquid material. Each sheet uses independent Runtime cells
to show simultaneously open menus, select, popover, both dialog kinds, toast and
an actual delayed tooltip. Golden comparisons and behaviour tests live in
`tests/overlays.rs`. All twelve PNGs were opened and visually inspected.

Compared with `docs/media/native/kit-liquid-dark.png`, menus retain the old
32px rows, 6px padding, 30px leading gutter, 224px minimum width, 9px separator
slots, Body typography, vector icons, check/radio marks and concentric row radius.
The elevated theme fill replaces wallpaper-dependent native glass in headless
sheets; elevation uses the theme shadow, and accessible ink is contrast-adjusted.
Section headings and shortcut hints extend the old menu. Cascading submenus are
outside this component's scope. One UI + Liquid material has the same opaque
fallback in these snapshots; actual glass is supplied by the host.

Overlay validation: `cargo test -p stillkit-kit` passes all 22 tests (ten overlay,
three motion, nine existing runtime tests); `cargo clippy -p stillkit-kit
--all-targets` is warning-free. `cargo check -p stillkit-kit --target
x86_64-unknown-linux-gnu` passes with the existing core a11y dead-code warnings.
All commands used `CARGO_TARGET_DIR=C:\mypaper-targets\stillkit\K\target`;
tests and gallery used the documented LLVM override. The twelve-sheet final
visual audit is also recorded in `docs/media/kit/overlays-overview.png`.

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

## Text

`TextField::new("Name").key("name").value(model.name.as_str())
.on_input(|s: &mut Model, value| s.name = value.into()).build()` uses el's editor:
selection, clipboard, IME, undo, caret and scrolling stay in Runtime.
Supply distinct stable keys. Labels may be above (default), inside with
`label_inside(true)`, or omitted with an empty label (the placeholder names the input).
`placeholder`, `icon`, `accessory`, `clearable`, `helper`, `error`, `invalid`,
`disabled`, `read_only`, `on_submit` and `select_all_on_focus` compose on the builder.
Errors use theme destructive ink and an accessible `Invalid: ?` description.
El does not expose an invalid-state setter yet.

`SecureField::new` starts secure; pass `revealed(model.revealed)` and
`on_reveal` for the model-owned reveal toggle. El redacts secure editor values.
`SearchField::new` supplies a search icon, a clear action and select-all on focus.
Escape clears and delegates blur to el (keyboard-visible focus only at present).
`TextArea::new` supplies multiline editing; configure `rows(n)`, `max_len(n)`
and `character_count(true)`. Counts and limits use Unicode scalar values, as el does.
`TextFieldFrame` remains a deprecated source-compatible legacy frame, used only
by the original gallery to retain its historical golden baseline.

`Combobox::new(label, options, &model.query, on_change)` takes a `QueryState`:
query, open, highlighted filtered index, and optional selected source index.
Typing opens and filters case-insensitively; Up/Down wraps, Enter picks and
Escape closes. Pointer rows pick too. The combobox exposes the selected label as
its accessible value. Its nonmodal popover reuses Menu and keeps focus in the
editor. Active-descendant semantics need an el setter. Use stable keys.

`CommandPalette::new(&model.query, on_change).command(Command::new(key, label,
action).group(group).icon(icon).shortcut(hint)).build()` returns a dialog overlay.
Attach it to its opening control. Commands filter by label; consecutive groups
get headers, with icons and shortcut hints in Menu rows. Keep each group together.
Arrows highlight, Enter runs and closes, Escape dismisses, and Runtime traps and
returns focus. Empty results show a message. Shortcut hints are descriptive;
the host owns global shortcut dispatch. Reset the query on opening if desired.

Field styles and clear-button presence use `MOTION_INTERACTIVE`; Menu and dialog
surfaces use `MOTION_OPEN`. Runtime owns reduced motion. Focus rings use the theme,
but their animation and focus-within rings around accessory wells need el support.

`cargo run -p stillkit-kit --example kit_text` writes twelve
`docs/media/kit/text-<look>-<scheme>.png` sheets: all presets and the One UI +
Liquid material mix, light and dark. Cells show rest, keyboard focus, filled search,
error, disabled/read-only, secure/multiline counter, open combobox and open palette.
The new sheets are visually checked against the native Liquid search field.
`tests/text.rs` exercises editing, clear/Escape, reveal, error descriptions,
filter/keyboard/pick, command execution and focus return, typeahead and max count.

Text validation: 45 headless tests pass across the kit, including eleven text tests;
all-target clippy is clean. The Linux target check passes with the existing core
a11y dead-code warnings. All twelve text PNGs were opened and inspected, with
native Liquid's search well as a reference. Commands used `D:\stillkit-targets\N`
and the LLVM dev override. No files under `crates/stillkit` were edited.


## Polish audit (2026-11-08)

Stepper now uses theme-height Remove/Add icon actions around a centred value on
one well. Endpoint actions disable at min/max; keyboard and assistive stepping
retain their existing range semantics. Disclosure reserves one stable themed icon
column plus ITEM spacing in both states; its body starts at the title. Sidebar
items and group triggers share the same leading inset, icon column and ITEM gap,
with selection fills inset by the sidebar's themed item padding. Group content
starts on the next grid column; caller-composed rows retain their own padding.
The chevron no longer changes occupied width while opening.

TextArea uses RADIUS_CARD on both the well and editor, including focus geometry;
single-line editors retain RADIUS_CONTROL. Search icons occupy the well's leading
padding, followed by ITEM spacing so the editor ring cannot touch the icon.
Button/list/toolbar/table/dialog/select gaps and toast padding/glyphs now read
existing semantic metrics. Menu check size and shortcut spacing read MenuMetrics.
Badge retains the native kit's 18px height, 6px horizontal inset and Caption type.
Menu retains its 32px rows, 6px panel inset and 30px leading column. Toast uses
Body type, card corners and the theme's Toast shadow.

### Remaining token gaps

These values remain local until the theme owns the relevant component geometry;
no new kit-only theme tokens or look-name branches were introduced:

- Button horizontal inset (14), list/field leading inset (10), inline field label
  inset (8), compact editor actions (28), disabled opacity (.45).
- Switch track/thumb sizes and travel; checkbox/radio mark, dot and inset sizes;
  slider track/bar/thumb sizes; segmented plate inset; calendar cell/gutter sizes.
- Badge minimum width/height (18), horizontal inset (6) and capsule radius (9).
- Menu row highlight radius (currently derived from surface radius), header and
  separator insets, and radio-dot geometry. MenuMetrics already supplies row,
  panel inset, gutter, check glyph and shortcut gap.
- Toast horizontal inset (14), maximum width (400) and entrance distance (12);
  table cell insets, tree indentation, tab indicator thickness/radius, avatar
  status geometry and skeleton sizing have no dedicated metrics.
- Field-specific multiline radius could eventually differ from card radius;
  RADIUS_CARD is the existing finite shape token used today.
- Tooltip is entirely el-owned. Compared with the old native tooltip, el uses
  8x4 padding and radius.small rather than 10x6 and radius.small + 2. Matching that
  needs shared tooltip geometry/style tokens in el. No wrapper or duplicate
  tooltip lifecycle was added here; the Needed from el list above is preserved.

### Repeating the visual audit

The existing 52 golden sheets remain the normal 1x gate. `sheet_at_scale` in each
of the four gallery helpers renders at a real integer Runtime device scale,
including device-sized cell compositing (no bitmap enlargement). The explicit
`polish::export_scale_audit` test exports all four families across the same eight
preset/composition themes in light and dark at both 1x and 2x (128 PNGs):

```powershell
$env:CARGO_TARGET_DIR = 'D:\stillkit-targets\S'
$env:CARGO_PROFILE_DEV_CODEGEN_BACKEND = 'llvm'
$env:STILLKIT_AUDIT_DIR = 'D:\stillkit-targets\S\audit-final'
cargo test -p stillkit-kit --test polish export_scale_audit -- --ignored
```

The audit export is explicitly invoked because it writes inspection artifacts;
the two normal polish tests verify range limits, control height, value centring
and stable disclosure alignment across all 16 theme compositions at both scales.


Visual verification: opened the baseline sheets and old native kit references,
then every final sheet through labelled contact sheets (128 exports), with native
pixel crops of Stepper/Disclosure, Sidebar, TextArea, palette search and Toast.
Full-resolution Studio layout and Solid overlay sheets were also opened to check
the overview reductions. Compared before/after: numeric buttons now use aligned
vector marks; group/content text follows its title grid; multiline wells retain
finite card corners; palette search glyphs clear the editor focus ring. Badge,
menu row geometry and ink hierarchy retain the reference treatment. No additional
obvious snapping drift was found at the inspected integer device scales.

Inspection artifacts remain under `D:\stillkit-targets\S\audit-before`,
`audit-step1`, `audit-final` and `audit-contact`. The tracked changes are all 16
`kit-*`, 12 `layout-*`, 12 `text-*`, 12 `overlays-*` goldens, the checkbox motion
strip (themed label gap), and refreshed `overview.png`/`overlays-overview.png`.


Validation used `CARGO_TARGET_DIR=D:\stillkit-targets\S` and the LLVM dev
backend throughout. The final full test run passed 47 tests (plus the explicitly
invoked audit exporter); Linux and aarch64 macOS cargo checks passed. Linux emits
8 pre-existing dead-code warnings in stillkit's compile-only accessibility path.
No changes were made under `crates/stillkit`, and no branch was pushed or merged.

Final `cargo clippy -p stillkit-kit --all-targets` is warning-free; the audit test
reuses the text gallery's overlay module to avoid duplicate module loading.

## App shells

`SplitView::new(key, &state, access, first, second)` composes two controlled panes.
Keep `SplitState` in the model and persist its public `size` in app settings.
`axis(SplitAxis::Vertical)` stacks panes; children may nest splits. `limits(min,max)`,
`reset_size(px)` and `collapsible(true)` control the leading pane. Captured pointer
travel resizes outside the grip; cancellation restores the starting size. Double-click
and Home reset; axis arrows step by 10 px (Shift: 1); Enter collapses/restores.
The splitter exposes an accessible range and resize cursor. Give the trailing pane
room by choosing limits appropriate to the host's minimum window size.

`TabStrip` accepts stable-id `Tab` values and emits `TabEvent` intents. Apply reorder
immediately to the model; keyed neighbours animate through the runtime. Tabs include
icons, titles, dirty dots and close actions, a scrollable rail, all-tabs menu and new
button. Arrows/Home/End select and reveal tabs; middle-click requests close. The app
owns unsaved-document policy. Forward Ctrl+Tab through `TabStripState::shortcut`
before Runtime (which reserves Tab for focus traversal). Forward Ctrl+Shift+Tab too.
A release more than 24 px vertically outside a tab emits `TornOut(id)`; the app moves
the document to another window. The kit does not create windows or transfer state.

`WindowTitlebar` provides leading, tabs and trailing slots. After layout and DPI
changes, call `WindowTitlebar::sync_chrome(&surface, &runtime, key)`. It registers
caption/minimize/maximize/close bounds with the existing platform hit tester;
Windows supplies dragging, double-click maximize and HTMAXBUTTON snap hover.
Keyboard/assistive caption activation emits `WindowAction` for the host to apply.
On macOS set `Caption::Compact` and pass `Surface::system_controls().right / scale`
to `system_inset`. No platform APIs, threads or imports are added by these builders.

`ShortcutRecorder` renders `ShortcutRecorderState` as keycaps. Its accessible name
is the action being bound. Before Runtime, forward Key **and KeyUp** through
`state.handle(event, modifiers)`, including after acceptance until key release;
consume events it uses so repeats cannot activate another control. Escape
cancels, Backspace/Delete clears, and deactivation cancels. Persist `chord`; the app
validates conflicts and registers shortcuts. `Keybind` displays keycap labels alone.

`HoverCard` wraps interactive rich content in a non-modal popover. Give it a
`key("details-panel")` and track that complete panel, including its padding. Feed pointer
and leave/deactivation/Escape events to `HoverCardState::handle`, using logical
anchor/card bounds and the window scale. Its 500 ms entry delay has one host wake:
include `deadline()` alongside `Runtime::wake_at`, then call `advance(now)`.
Pointer transfer into the card keeps it open; dismissal clears the deadline.
Use `enter(now)` for keyboard-focus entry, or a keyboard-activatable trigger to open
it directly. The example demonstrates the host wiring without a polling timer.

`RollingText` slides/fades a replaced complete text run (including shaped Unicode),
using retained presence and the runtime's reduced-motion policy. Unlike Rune's
per-grapheme LCS matching, it replaces the whole run and keeps no global cache.
`SidebarHeader`/`SidebarSection` provide controlled collapse and counts;
`SidebarRow` provides icons, selection and counts. Wrap rows in a vertical focus
group for arrow navigation. `Page` places a heading, toolbar and flexible content.
All components use theme tokens and shared contrast helpers for the five looks.

`cargo run -p stillkit-kit --example app_shell` opens an editor shell. Add
`--snap-dir PATH` to export all ten look/scheme combinations, at rest and with rich content open,
without a window.
`tests/app_shell.rs` covers resize capture/cancel/reset, keyboard limits, tab
reorder/tear-out/close, shortcut key-up, delayed hover transfer, collapse and idle.

`PopupList`/`PopupItem` supply bounded listbox content for completion popovers.
The app filters rows, supplies stable ids/selection and applies picks; disabled rows
stay announced but are skipped by keyboard navigation. Compose it with `el::popover`
and `take_focus()` when keyboard focus should leave the triggering editor.

### Virtual tables

For large data, keep a `TableModel` in app state and build with `VirtualTable::new(key, &state, model)`.
`set_rows` supplies stable keys and type-ahead labels only when the order changes;
the custom cell callback runs only for the built range. Columns retain their widths
with min/max bounds and a header drag handle. The header stays above the scrolling
body. The sort callback applies your comparator and calls `set_rows` with the result.
`selection.multi = true` enables Ctrl toggles and Shift ranges. Up/Down, Home/End,
PageUp/PageDown, type-ahead and Enter operate over the whole model, including rows
that have never been built. `reveal(key)` scrolls to an unbuilt row. Grid, row and
cell semantics include complete counts and indexes. Width persistence across process
launches belongs to the app; the model retains widths across view runs.

```rust
VirtualTable::new("records", &s.table, |s: &mut State| &mut s.table)
    .label("Records")
    .height(420.)
    .cell(|row, col| text(format!("{row}:{col}")))
    .on_open(|s, key| s.opened = key.to_string())
    .on_sort(|s, (col, dir)| { /* reorder s.table for col and dir */ })
    .build()
```

Only the key, borrowed model and mutable model accessor are required. The label defaults
to the key. Without `height`, the control grows to fill available height; constrain its
parent. Paging moves ten rows without an explicit height. The default cells show row
labels in the first column. `on_sort` enables sortable headers; without it, header presses
do nothing. `on_open` enables Enter opening; otherwise Enter is unhandled. Columns use
fixed design-pixel widths with no horizontal scroll; the header does not scroll sideways.

### Virtual trees

`TreeModel::new(nodes)` caches flattened visibility; `VirtualTree::new(key, &state, model).build()` builds only
rows near sight. Left/Right collapse, expand and move to parents or first children;
Up/Down, Home/End, paging and type-ahead use visible order. Set `selection.multi` for
Ctrl and Shift selection. `reveal(key)` opens the complete ancestor path, selects the
node and scrolls to it. `filter` keeps matching labels and ancestors; `clear_filter`
keeps current expansion state. `sort_natural` orders sibling filenames by numeric
runs without integer overflow. Each tree item reports level, sibling count and position.

```rust
VirtualTree::new("files", &s.tree, |s: &mut State| &mut s.tree)
    .label("Project files")
    .height(390.)
    .on_open(|s, key| s.opened = key.to_string())
    .build()
```

The tree uses the same label and height defaults as the table. Leaf opening is off
without `on_open`; Enter/Space still toggle branches.

Run `cargo run -p stillkit-kit --example data_grid` for a 100,000-row table alongside
a file-like tree, with resizing, sorting, reveal and filtering.

## Component stories

The optional [playground](../../docs/guide/playground.md) includes each supported component and all button kinds. Run `cargo run -p stillkit-playground --example stories`; `--goldens DIR` renders every story across looks and schemes.

## Markdown (`markdown` feature)

`Markdown::new(source).on_link(|state, url| ..).build()` renders headings,
paragraphs, nested ordered/unordered lists, quotes, rules and equal-width GFM tables
(including column alignment). Inline bold, italic, strike, code and links use native
rich runs. Fenced code uses a monospace face and a Copy code button. Copy preserves
the fence contents; `.on_copy(..)` lets an app supply its own clipboard handler.
Without it the button writes to the system clipboard on activation.

The whole view is one selectable document in reading order. Ctrl+A/C and drag selection
cross blocks; copy buttons keep their input and their labels are excluded. Layout,
typography, spacing and paint follow the current theme. Give the built element a
stable `.key(..)` and compose it inside a bounded scroll view for long documents.

Enable `stillkit-kit = { ..., features = ["markdown"] }`. This opts into
pulldown-cmark with default features disabled. Parsing happens only when `build`
runs; retained hover/selection does not parse, and there are no timers, workers or
network requests. Image markup shows alt text only. Raw HTML is ignored; applications
can compose their own supplied image elements outside the Markdown view. There is
no syntax highlighting, HTML renderer or streaming parse cache.

Run `cargo run -p stillkit-kit --features markdown --example markdown_viewer`.
Without the feature, this example renders the source as selectable plain text, for
comparing the optional parser/renderer's executable cost.
