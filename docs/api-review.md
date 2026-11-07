# Public API review for 0.1

Reviewed against the mypaper shell, Studio and Files source in its original checkout (read-only), all stillkit crates, examples and guides. The inventory below records every explicit public declaration and re-export in core sources, including platform alternatives and doc-hidden compatibility APIs. `pub(crate)` and `pub(super)` are excluded. Declarations in private implementation modules are included when their types are re-exported, so source paths are not necessarily import paths. Enum variants and trait members inherit their enclosing item's visibility; their documented definitions accompany the listed types/traits in source.

## Decisions

- `Identity { prefix, desktop_classes }` and `set_identity -> Result<(), Identity>` are portable. Root and backend identity import paths share one type. Windows preserves `<prefix>`, `<prefix>.app`, `<prefix>.desktop` and desktop ordering; Linux uses prefix as Wayland app_id. No X11 backend exists. macOS accepts configuration without renaming the bundle.
- `interaction_motion` is the single defining module. `interact::motion` is a compatibility re-export of the same module and types, not a second compiled file. mypaper shell flyouts and their kit adapter use `interact::motion::InteractionMotion`; that exact path remains compatible. There are no direct interaction_motion imports in mypaper.
- `controls` is doc-hidden with an explicit legacy note. It exists for mypaper Studio's old immediate-mode controls. Its public items remain callable. `kit`, `Frame`, chart helpers, calendar and the root drawing primitives remain visible: examples and/or stillkit-kit use them, so they are not exclusively legacy APIs.
- Existing public paths were retained after the consumer search. No existing item was narrowed merely because it looked low-level: backend contracts are documented in docs/ui.md, and platform hooks support native integration. New identity internals are `pub(crate)`; the Linux adapter and its methods are `pub(super)`, with no new public accessibility configuration API.
- `#![warn(missing_docs)]` now checks the crate. Missing comments on exported types, fields, variants, constants and methods were filled across core and native backend alternatives. Changes under `el` are documentation only. No stillkit-kit source was changed.

## 0.2 migration/deprecation plan

`controls` is a deprecation candidate for 0.2 after Studio moves to el + stillkit-kit; it has no `#[deprecated]` attribute in 0.1. Keep the source-compatible path until that consumer migrates. Prefer `interaction_motion` for timing imports; consider deprecating the `interact::motion` alias in 0.2 after downstream migration. Keep immediate-mode `kit` and its root exports while the gallery and declarative paint adapters use them. Do not remove backend contracts without replacing the documented native integration seam. These are migration plans, not current compiler deprecations.

## Linux verification

AT-SPI uses the shared Snapshot tree, retained hits/focus/semantics, lazy AccessKit activation, bounded action queues and a socket integrated into Wayland poll. `STILLKIT_ACCESSIBILITY=1` opts into AccessKit Unix's eager executor and D-Bus discovery; unset means no adapter/thread/connection/wake descriptor. An enabled process may discover clients started later. Automatic discovery in the default disconnected mode is intentionally unavailable. Linux adapter tests are compile-checked here; WSL has no compositor.

On m6u test Orca/AT-SPI discovery; latest tree on initial/late activation; labels, roles, focus, numeric/text updates, reorder actions and live status; multiple windows, close and client reconnect; idle CPU/wakes before enabling, while connected and after Orca exits; CPU mode without GPU drivers. Verify custom app_id in the compositor. See platform-notes.md for startup instructions.

## Declaration inventory

Each entry uses a source line to distinguish same-named fields and inherent methods. Root `pub use` entries define the supported short import paths. Feature/test-gated declarations are retained in the inventory; availability follows their source cfg attributes. Empty backend compatibility modules are included.


Inventory: 2051 explicit public declarations/re-exports across all backend alternatives.


### a11y.rs

```rust
16: pub enum Semantics {
44: pub struct FieldText {
46: pub lines: Vec<TextLine>,
48: pub anchor: (usize, usize),
50: pub caret: (usize, usize),
55: pub struct TextLine {
57: pub text: Arc<str>,
59: pub rect: crate::Rect,
61: pub chars: Vec<(f32, f32)>,
66: pub struct Detail {
68: pub description: Option<Arc<str>>,
70: pub value: Option<Arc<str>>,
72: pub selected: Option<bool>,
74: pub expanded: Option<bool>,
76: pub popup: Option<accesskit::HasPopup>,
78: pub controls: Option<HitId>,
80: pub modal: bool,
85: pub enum AccessibilityAction {
123: pub role: Role,
124: pub name: Arc<str>,
125: pub hits: Vec<Hit>,
126: pub focus: Option<HitId>,
127: pub semantics: Vec<(HitId, Semantics)>,
```


### bitmap.rs

```rust
7: pub struct TextQuality {
9: pub gamma: f32,
11: pub contrast: f32,
13: pub mode: TextMode,
15: pub grid_fit: bool,
20: pub enum TextMode {
45: pub struct Image {
47: pub width: u32,
49: pub height: u32,
51: pub pixels: Vec<u8>,
56: pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
```


### calendar.rs

```rust
6: pub struct Date {
8: pub year: i32,
10: pub month: u32,
12: pub day: u32,
16: pub fn new(year: i32, month: u32, day: u32) -> Option<Self> {
21: pub fn parse(value: &str) -> Option<Self> {
31: pub fn key(self) -> String {
35: pub fn ordinal(self) -> i64 {
44: pub fn from_ordinal(day: i64) -> Self {
61: pub fn weekday(self) -> u32 {
65: pub fn iso_week(self) -> u32 {
76: pub fn add_days(self, days: i64) -> Self {
80: pub fn shift_month(self, delta: i32) -> Self {
90: pub fn days_in_month(year: i32, month: u32) -> u32 {
104: pub const MONTHS: [&str; 12] = [
119: pub const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
123: pub struct MonthGrid {
125: pub month: Date,
127: pub week_start: u32,
129: pub cells: Vec<Option<Date>>,
133: pub fn new(month: Date, week_start: u32) -> Self {
158: pub fn view(
173: pub fn view_with_weekdays(
```


### chart.rs

```rust
10: pub struct Series<'a> {
12: pub color: Rgba,
14: pub values: &'a [f64],
19: pub fn area_chart(
132: pub struct LimitBar<'a> {
134: pub left: f32,
136: pub pace: Option<f32>,
138: pub fill: Rgba,
140: pub badge: Option<&'a str>,
142: pub label: Option<&'a str>,
144: pub reset: Option<&'a str>,
148: pub fn limit_bar(f: &mut Frame<'_>, rect: Rect, bar: &LimitBar<'_>) {
154: pub fn compact_limit_bar(f: &mut Frame<'_>, rect: Rect, bar: &LimitBar<'_>) {
```


### color_row/popover.rs

```rust
10: pub struct Hsv {
12: pub hue: f32,
14: pub saturation: f32,
16: pub value: f32,
21: pub enum Part {
30: pub fn from_rgb(rgb: [u8; 3]) -> Self {
52: pub fn rgb(self) -> [u8; 3] {
69: pub fn at(self, part: Part, rect: Rect, point: Point) -> Self {
101: pub fn draw<A: Clone>(
```


### color_row.rs

```rust
5: pub mod popover;
8: pub struct Choice<A> {
10: pub label: String,
12: pub color: Option<Rgba>,
14: pub selected: bool,
16: pub action: A,
20: pub fn height<A: Clone>(b: &Builder<'_, A>) -> f32 {
29: pub fn draw<A: Clone>(b: &mut Builder<'_, A>, id: Key, rect: Rect, choices: &[Choice<A>]) {
```


### continuous.rs

```rust
6: pub fn corner(radius: f32, smoothing: f32, budget: f32) -> (f32, f32) {
42: pub fn surface(list: &mut DisplayList, body: Rect, smoothing: f32) {
```


### controls.rs

```rust
32: pub type Key = u64;
35: pub fn key(parts: impl Hash) -> Key {
42: pub fn format_value(value: f64, unit: &str) -> String {
59: pub fn segment_fill(dark: bool) -> Rgba {
68: pub struct Palette {
70: pub dark: bool,
72: pub bg: Rgba,
74: pub chrome: Rgba,
76: pub panel: Rgba,
78: pub hover: Rgba,
80: pub selected: Rgba,
82: pub line: Rgba,
84: pub edge: Rgba,
86: pub control: Rgba,
88: pub fill: Rgba,
90: pub well: Rgba,
92: pub ink1: Rgba,
94: pub ink2: Rgba,
96: pub ink3: Rgba,
98: pub ink4: Rgba,
100: pub solid: Rgba,
102: pub on_solid: Rgba,
104: pub solid_hover: Rgba,
106: pub scrim: Rgba,
108: pub shadow: Rgba,
125: pub fn dark() -> Self {
151: pub fn light() -> Self {
178: pub fn contrast(window: Rgba, text: Rgba, highlight: Rgba, gray: Rgba, button: Rgba) -> Self {
204: pub fn hint(&self) -> Rgba {
211: pub fn theme(&self) -> Theme {
251: pub enum Font {
274: pub struct Type {
276: pub text: Arc<str>,
278: pub display: Arc<str>,
280: pub mono: Arc<str>,
282: pub icons: Arc<str>,
284: pub text_scale: f32,
289: pub fn new(text: &Text) -> Self {
308: pub fn style(&self, font: Font, scale: f32) -> TextStyle {
339: pub fn icon(&self, size: f32, scale: f32) -> TextStyle {
347: pub enum Glyph {
368: pub struct Input {
370: pub hover: Option<Key>,
372: pub pressed: Option<Key>,
374: pub focus: Option<Key>,
376: pub focus_visible: bool,
378: pub pointer: Option<Point>,
383: pub enum Behavior {
419: pub struct Target<A> {
421: pub key: Key,
423: pub action: A,
425: pub behavior: Behavior,
427: pub group: Option<Key>,
429: pub rect: Rect,
431: pub visible: Option<Rect>,
433: pub scroll: Option<Key>,
438: pub struct ScrollArea {
440: pub key: Key,
442: pub viewport: Rect,
444: pub content: f32,
449: pub enum ButtonKind {
460: pub enum Align {
470: pub struct Builder<'a, A> {
472: pub list: DisplayList,
474: pub semantics: Vec<(crate::HitId, crate::Semantics)>,
476: pub text: &'a mut Text,
478: pub p: &'a Palette,
480: pub t: &'a Type,
482: pub kit: Theme,
484: pub s: f32,
486: pub input: &'a Input,
488: pub targets: Vec<Target<A>>,
490: pub scrolls: Vec<ScrollArea>,
498: pub fn new(text: &'a mut Text, p: &'a Palette, t: &'a Type, s: f32, input: &'a Input) -> Self {
517: pub fn px(&self, v: f32) -> f32 {
522: pub fn with_style(mut self, style: Style) -> Self {
533: pub fn with_filled_controls(mut self) -> Self {
539: pub fn control_radius(&self, rect: Rect) -> f32 {
547: pub fn label_width(&self, width: f32) -> f32 {
553: pub fn list_row_area(&self, rect: Rect) -> Rect {
563: pub fn font(&self, font: Font) -> TextStyle {
568: pub fn hovered(&self, key: Key) -> bool {
573: pub fn pressed(&self, key: Key) -> bool {
578: pub fn focused(&self, key: Key) -> bool {
583: pub fn group<R>(&mut self, key: Key, f: impl FnOnce(&mut Self) -> R) -> R {
591: pub fn clip_rect(&self) -> Option<Rect> {
596: pub fn semantics(&mut self, key: Key, value: crate::Semantics) {
603: pub fn announce(&mut self, message: &str, revision: u64) {
613: pub fn hit(
627: pub fn hit_checked(
699: pub fn fill(&mut self, rect: Rect, radius: f32, color: Rgba) {
710: pub fn stroke(&mut self, rect: Rect, radius: f32, width: f32, color: Rgba) {
720: pub fn line(&mut self, from: Point, to: Point, width: f32, color: Rgba) {
730: pub fn hline(&mut self, x0: f32, x1: f32, y: f32, color: Rgba) {
736: pub fn vline(&mut self, x: f32, y0: f32, y1: f32, color: Rgba) {
742: pub fn measure(&mut self, text: &str, style: &TextStyle) -> (f32, f32) {
748: pub fn text_in(
777: pub fn text_at(
800: pub fn paragraph(
821: pub fn icon(&mut self, rect: Rect, glyph: impl Into<Glyph>, size: f32, color: Rgba) {
875: pub fn focus_ring(&mut self, key: Key, rect: Rect, radius: f32) {
888: pub fn wash(&mut self, key: Key, rect: Rect, radius: f32) {
898: pub fn elevated(&mut self, rect: Rect, radius: f32) {
911: pub fn scrim(&mut self, key: Key, rect: Rect, dismiss: A) {
920: pub fn button(
984: pub fn button_width(&mut self, label: &str, icon: bool, small: bool) -> f32 {
998: pub fn icon_button(
1031: pub fn switch(&mut self, key: Key, at: Point, on: bool, label: &str, action: A) -> Rect {
1060: pub fn segmented(
1198: pub fn segmented_width(&mut self, items: &[&str]) -> f32 {
1209: pub fn labeled_segments(
1269: pub fn fit_tiles(
1370: pub fn slider(
1469: pub fn select(&mut self, key: Key, rect: Rect, value: &str, label: &str, action: A) {
1511: pub fn select_width(&mut self, value: &str) -> f32 {
1520: pub fn field(
1567: pub fn field_inner(&self, rect: Rect, glyph: bool) -> Rect {
1578: pub fn badge(&mut self, x: f32, y: f32, label: &str, glyph: Option<Glyph>, solid: bool) -> f32 {
1616: pub fn overlay_badge(&mut self, x: f32, y: f32, label: &str, glyph: Option<Glyph>) -> f32 {
1653: pub fn keycap(&mut self, x: f32, y: f32, label: &str) -> f32 {
1664: pub fn keys(&mut self, x: f32, y: f32, keys: &[&str]) -> f32 {
1673: pub fn dot(&mut self, center: Point, on: bool) {
1683: pub fn tile_frame(&mut self, key: Key, rect: Rect, radius: f32, selected: bool, gap: Rgba) {
1711: pub fn row(
1758: pub fn row_height(&self, subtitle: bool) -> f32 {
1766: pub fn section_header(&mut self, rect: Rect, title: &str) {
1774: pub fn plate(&mut self, key: Key, rect: Rect, radius: f32, selected: bool) {
1784: pub fn check_mark(&mut self, card: Rect) {
1797: pub fn card(&mut self, rect: Rect, radius: f32) {
1807: pub fn scroll(
1876: pub fn intersect(a: Rect, b: Rect) -> Option<Rect> {
```


### damage.rs

```rust
4: pub fn regions(
```


### el/animate.rs

```rust
29: pub rel: [f32; 4],
30: pub fill: Option<Rgba>,
32: pub ink: Rgba,
33: pub opacity: f32,
35: pub radius: f32,
37: pub shadow: f32,
39: pub value: f32,
41: pub motion: Option<Spring>,
43: pub open: Spring,
45: pub transition: Option<Transition>,
51: pub s: f32,
52: pub x: f32,
53: pub y: f32,
57: pub const ID: Self = Self { s: 1.0, x: 0.0, y: 0.0 };
82: pub rect: Rect,
84: pub local: Rect,
86: pub xf: Xf,
87: pub fill: Option<Rgba>,
88: pub ink: Rgba,
90: pub opacity: f32,
92: pub radius: f32,
93: pub shadow: f32,
95: pub value: f32,
100: pub fn radius(&self) -> f32 {
290: pub t: f64,
292: pub reduced: bool,
298: pub fn retarget(&mut self, laid: &[Laid], targets: &[Target], prev: &HashSet<u64>, mode: Mode) {
324: pub fn leave(&mut self, id: u64) -> bool {
336: pub fn retain(&mut self, keep: impl Fn(u64) -> bool) {
342: pub fn settle(&mut self) -> Vec<u64> {
357: pub fn moving(&self) -> bool {
363: pub fn poses(&self, laid: &[Laid], targets: &[Target], scrolled: &HashMap<usize, Point>) -> Vec<Pose> {
425: pub fn ghost(&self, id: u64, frozen: &[Pose], parent: &Pose) -> Vec<Pose> {
```


### el/clipboard.rs

```rust
7: pub trait Clipboard {
16: pub struct Memory(pub Option<String>);
```


### el/edit.rs

```rust
112: pub fn text(&self) -> &str {
127: pub text: String,
129: pub anchor: usize,
130: pub caret: usize,
135: pub preedit: Option<(String, usize)>,
137: pub max: Option<usize>,
144: pub fn new(text: &str) -> Self {
154: pub fn selection(&self) -> std::ops::Range<usize> {
158: pub fn selected(&self) -> &str {
163: pub fn place(&mut self, at: usize, extend: bool) {
173: pub fn select(&mut self, range: std::ops::Range<usize>) {
181: pub fn set_text(&mut self, text: &str) {
216: pub fn type_text(&mut self, text: &str, multiline: bool) -> bool {
230: pub fn apply(&mut self, cmd: &Cmd, multiline: bool) -> bool {
291: pub fn undo(&mut self) -> bool {
298: pub fn redo(&mut self) -> bool {
306: pub fn mark(&self) -> Mark {
314: pub fn back(&mut self, mark: Mark) {
322: pub fn word_at(&self, at: usize) -> std::ops::Range<usize> {
334: pub fn line_at(&self, at: usize) -> std::ops::Range<usize> {
341: pub fn shown(&self) -> std::borrow::Cow<'_, str> {
353: pub fn shown_caret(&self) -> usize {
```


### el/element.rs

```rust
15: pub type Key = Arc<str>;
19: pub enum Len {
28: pub fn resolve(self, theme: &Theme) -> f32 {
44: pub enum Paint {
53: pub fn resolve(self, theme: &Theme) -> Rgba {
70: pub enum Motion {
79: pub fn resolve(self, theme: &Theme) -> Spring {
96: pub struct Transition {
107: pub const fn fade() -> Self {
116: pub const fn scale(scale: f32) -> Self {
124: pub const fn slide(x: f32, y: f32) -> Self {
135: pub struct Tx {
144: pub fn instant(&mut self) {
150: pub fn modifiers(&self) -> Modifiers {
155: pub fn set_theme(&mut self, theme: Theme) {
161: pub fn scroll_into_view(&mut self, key: impl Into<Key>) {
169: pub struct KeyCode(pub u16);
173: pub const BACKSPACE: Self = Self(0x08);
175: pub const TAB: Self = Self(0x09);
177: pub const ENTER: Self = Self(0x0D);
179: pub const ESCAPE: Self = Self(0x1B);
181: pub const SPACE: Self = Self(0x20);
183: pub const PAGE_UP: Self = Self(0x21);
185: pub const PAGE_DOWN: Self = Self(0x22);
187: pub const END: Self = Self(0x23);
189: pub const HOME: Self = Self(0x24);
191: pub const LEFT: Self = Self(0x25);
193: pub const UP: Self = Self(0x26);
195: pub const RIGHT: Self = Self(0x27);
197: pub const DOWN: Self = Self(0x28);
199: pub const INSERT: Self = Self(0x2D);
201: pub const DELETE: Self = Self(0x2E);
205: pub const fn of(c: char) -> Self {
212: pub struct Modifiers {
214: pub shift: bool,
216: pub ctrl: bool,
218: pub alt: bool,
220: pub meta: bool,
225: pub const NONE: Self = Self { shift: false, ctrl: false, alt: false, meta: false };
227: pub const SHIFT: Self = Self { shift: true, ..Self::NONE };
229: pub const CTRL: Self = Self { ctrl: true, ..Self::NONE };
231: pub const ALT: Self = Self { alt: true, ..Self::NONE };
251: pub enum Handled {
266: pub enum Axis {
274: pub type Orientation = Axis;
278: pub enum ArcStart {
304: pub enum Align {
317: pub enum Justify {
337: pub type Painter = Box<dyn Fn(&mut Frame, Rect, HitId)>;
368: pub min: f64,
369: pub max: f64,
370: pub step: f64,
371: pub value: f64,
378: pub fraction: f32,
379: pub start: ArcStart,
381: pub span: f32,
382: pub stroke: Len,
384: pub track: Option<Paint>,
390: pub multiline: bool,
392: pub value: Option<Arc<str>>,
393: pub placeholder: Option<Arc<str>>,
394: pub secure: bool,
395: pub max_len: Option<usize>,
396: pub read_only: bool,
397: pub select_all: bool,
398: pub role: TextRole,
400: pub rows: usize,
422: pub w: Option<Dim>,
423: pub h: Option<Dim>,
424: pub min_w: Option<Len>,
425: pub min_h: Option<Len>,
426: pub max_w: Option<Len>,
427: pub max_h: Option<Len>,
428: pub grow: f32,
429: pub gap: Option<Len>,
431: pub pad: [Option<Len>; 4],
432: pub align: Option<Align>,
433: pub justify: Option<Justify>,
434: pub absolute: Option<(Len, Len)>,
439: pub fill: Option<Paint>,
441: pub color: Option<Paint>,
442: pub radius: Option<Len>,
443: pub shadow: Option<Cast>,
444: pub opacity: f32,
456: pub fn layers(self, theme: &Theme) -> [crate::SoftShadow; 2] {
467: pub motion: Option<Motion>,
468: pub instant: bool,
469: pub transition: Option<Transition>,
474: pub role: Option<Role>,
475: pub label: Option<Arc<str>>,
476: pub checked: Option<bool>,
477: pub disabled: bool,
478: pub focusable: bool,
479: pub range: Option<Range>,
480: pub description: Option<Arc<str>>,
482: pub value: Option<Arc<str>>,
483: pub selected: Option<bool>,
484: pub expanded: Option<bool>,
490: pub axis: Axis,
492: pub follow: bool,
497: pub struct El<S> {
559: pub fn el<S>() -> El<S> {
564: pub fn row<S>() -> El<S> {
569: pub fn col<S>() -> El<S> {
574: pub fn grid<S>(columns: u16) -> El<S> {
580: pub fn text<S>(text: impl Into<Arc<str>>) -> El<S> {
611: pub fn text_input<S>(key: impl Into<Key>) -> El<S> {
619: pub fn text_area<S>(key: impl Into<Key>) -> El<S> {
624: pub fn icon<S>(icon: Icon) -> El<S> {
629: pub fn image<S>(image: Arc<Image>) -> El<S> {
635: pub fn paint<S>(painter: impl Fn(&mut Frame, Rect, HitId) + 'static) -> El<S> {
644: pub fn arc<S>(fraction: f32) -> El<S> {
657: pub fn key(mut self, key: impl Into<Key>) -> Self {
663: pub fn child(mut self, child: El<S>) -> Self {
669: pub fn children(mut self, children: impl IntoIterator<Item = El<S>>) -> Self {
677: pub fn w(mut self, w: impl Into<Len>) -> Self {
683: pub fn h(mut self, h: impl Into<Len>) -> Self {
689: pub fn size(self, side: impl Into<Len>) -> Self {
695: pub fn w_full(mut self) -> Self {
701: pub fn h_full(mut self) -> Self {
707: pub fn grow(mut self, share: f32) -> Self {
713: pub fn min_w(mut self, v: impl Into<Len>) -> Self {
719: pub fn min_h(mut self, v: impl Into<Len>) -> Self {
725: pub fn max_w(mut self, v: impl Into<Len>) -> Self {
731: pub fn max_h(mut self, v: impl Into<Len>) -> Self {
737: pub fn gap(mut self, gap: impl Into<Len>) -> Self {
743: pub fn pad(mut self, pad: impl Into<Len>) -> Self {
749: pub fn pad_x(mut self, pad: impl Into<Len>) -> Self {
756: pub fn pad_y(mut self, pad: impl Into<Len>) -> Self {
763: pub fn align(mut self, align: Align) -> Self {
769: pub fn justify(mut self, justify: Justify) -> Self {
775: pub fn absolute(mut self, x: impl Into<Len>, y: impl Into<Len>) -> Self {
783: pub fn fill(mut self, fill: impl Into<Paint>) -> Self {
789: pub fn color(mut self, color: impl Into<Paint>) -> Self {
795: pub fn radius(mut self, radius: impl Into<Len>) -> Self {
802: pub fn shadow(self) -> Self {
807: pub fn shadow_level(mut self, level: crate::ShadowLevel) -> Self {
813: pub fn box_shadow(mut self, shadow: crate::SoftShadow) -> Self {
819: pub fn opacity(mut self, opacity: f32) -> Self {
825: pub fn font(mut self, role: TextRole) -> Self {
845: pub fn placeholder(mut self, text: impl Into<Arc<str>>) -> Self {
853: pub fn secure(mut self) -> Self {
859: pub fn max_len(mut self, chars: usize) -> Self {
865: pub fn read_only(mut self) -> Self {
871: pub fn select_all_on_focus(mut self) -> Self {
877: pub fn rows(mut self, rows: usize) -> Self {
886: pub fn on_input(mut self, handler: impl Fn(&mut S, &str) + 'static) -> Self {
893: pub fn on_submit(mut self, handler: impl Fn(&mut S, &str) + 'static) -> Self {
899: pub fn wrap(mut self, lines: usize) -> Self {
910: pub fn role(mut self, role: Role) -> Self {
916: pub fn label(mut self, label: impl Into<Arc<str>>) -> Self {
922: pub fn checked(mut self, on: bool) -> Self {
928: pub fn disabled(mut self, disabled: bool) -> Self {
934: pub fn focusable(mut self) -> Self {
940: pub fn description(mut self, description: impl Into<Arc<str>>) -> Self {
948: pub fn value(mut self, value: impl Into<Arc<str>>) -> Self {
959: pub fn selected(mut self, selected: bool) -> Self {
965: pub fn expanded(mut self, expanded: bool) -> Self {
973: pub fn range(mut self, min: f64, max: f64, step: f64, value: f64) -> Self {
985: pub fn focus_group(mut self, orientation: Orientation) -> Self {
995: pub fn select_follows_focus(mut self) -> Self {
1010: pub fn scroll(mut self, axis: Axis) -> Self {
1018: pub fn arc_start(mut self, start: ArcStart) -> Self {
1027: pub fn arc_span(mut self, span: f32) -> Self {
1035: pub fn stroke(mut self, width: impl Into<Len>) -> Self {
1043: pub fn track(mut self, track: impl Into<Paint>) -> Self {
1051: pub fn no_track(mut self) -> Self {
1066: pub fn motion(mut self, motion: impl Into<Motion>) -> Self {
1072: pub fn animate(self) -> Self {
1077: pub fn instant(mut self) -> Self {
1085: pub fn transition(mut self, transition: Transition) -> Self {
1094: pub fn on_press(mut self, handler: impl Fn(&mut S) + 'static) -> Self {
1101: pub fn on_press_with(mut self, handler: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
1109: pub fn on_drag(mut self, handler: impl Fn(&mut S, f32, f32) + 'static) -> Self {
1121: pub fn on_key(mut self, handler: impl Fn(&mut S, KeyCode, &mut Tx) -> Handled + 'static) -> Self {
1130: pub fn on_char(mut self, handler: impl Fn(&mut S, char, &mut Tx) -> Handled + 'static) -> Self {
1140: pub fn on_step(mut self, handler: impl Fn(&mut S, f64) + 'static) -> Self {
1150: pub fn on_context_menu(mut self, handler: impl Fn(&mut S, f32, f32) + 'static) -> Self {
1158: pub fn on_timeout(mut self, after: Duration, handler: impl Fn(&mut S) + 'static) -> Self {
1171: pub fn overlay(mut self, overlay: impl Into<Option<super::Overlay<S>>>) -> Self {
1183: pub fn tooltip(mut self, text: impl Into<Arc<str>>) -> Self {
```


### el/field.rs

```rust
27: pub start: usize,
28: pub end: usize,
30: pub stops: Vec<(usize, f32)>,
32: pub drawn: Arc<str>,
41: pub fn x(&self, at: usize) -> f32 {
46: pub fn at(&self, x: f32) -> usize {
57: pub lines: Vec<Line>,
59: pub line: f32,
63: pub fn height(&self) -> f32 {
68: pub fn line_of(&self, at: usize) -> usize {
76: pub style: TextStyle,
77: pub line: f32,
78: pub ink: Rgba,
79: pub multiline: bool,
80: pub secure: bool,
82: pub rect: Rect,
83: pub content: Rect,
89: pub ed: Editor,
92: pub value: Option<Arc<str>>,
93: pub taken: Option<super::edit::Mark>,
94: pub look: Option<Look>,
95: pub shape: Shape,
99: pub scroll_x: f32,
101: pub goal: Option<f32>,
103: pub blink: Duration,
105: pub clicks: Option<(Duration, Point, u8)>,
108: pub native: Option<crate::TextField>,
112: pub fn new(text: &str) -> Self {
120: pub fn reshape(&mut self, text: &mut Text) -> bool {
134: pub fn follow(&mut self) {
148: pub fn caret(&self, s: f32) -> Rect {
170: pub fn hit(&self, p: Point, scroll_y: f32) -> usize {
190: pub fn accessible(&self, scroll_y: f32) -> Option<crate::FieldText> {
218: pub fn touch(&mut self, now: Duration) {
223: pub fn caret_on(&self, now: Duration) -> bool {
229: pub fn next_blink(&self, now: Duration) -> Option<Duration> {
```


### el/host.rs

```rust
36: pub struct App<S, V> {
38: pub title: &'static str,
40: pub size: (f32, f32),
42: pub kind: Kind,
44: pub theme: Theme,
46: pub state: S,
48: pub view: V,
53: pub enum Input {
76: pub struct Script(pub Vec<(Duration, Input)>);
80: pub fn parse(text: &str) -> Result<Self> {
117: pub fn end(&self) -> Duration {
164: pub fn play(&mut self, script: &Script, until: Option<Duration>) {
198: pub fn tree_json(&self) -> String {
292: pub fn run<S: 'static, V: Fn(&S) -> El<S> + 'static>(app: App<S, V>) -> Result<()> {
```


### el/json.rs

```rust
16: pub fn get(&self, key: &str) -> Option<&Json> {
23: pub fn num(&self) -> Option<f64> {
30: pub fn str(&self) -> Option<&str> {
38: pub fn point(&self) -> Option<(f32, f32)> {
```


### el/layout.rs

```rust
44: pub id: u64,
45: pub hit: HitId,
47: pub rect: Rect,
49: pub parent: Option<usize>,
51: pub extent: Option<(f32, f32)>,
60: pub touched: usize,
65: pub theme: &'a Theme,
66: pub s: f32,
98: pub fn new() -> Self {
109: pub fn update<S>(
```


### el/mod.rs

```rust
54: pub mod token;
56: pub use element::{ Align, ArcStart, Axis, El, Handled, Justify, Key, KeyCode, Len, Modifiers, Motion, Orientation, Paint, Painter, Transition, Tx, arc, col, el, grid, icon, image, paint, row, text, text_area, text_input, };
60: pub use crate::platform::current::host::reduced_motion;
61: pub use clipboard::{Clipboard, Memory as MemoryClipboard};
62: pub use host::{App, Input, Script, run};
64: pub use accesskit::HasPopup;
66: pub use crate::theme::{ShadowLevel, SoftShadow};
67: pub use overlay::{Overlay, Side, TOOLTIP_DELAY, dialog, menu, popover, toasts};
68: pub use runtime::{Change, Popup, Runtime, Shown, Snapshot};
```


### el/overlay.rs

```rust
32: pub enum Side {
121: pub struct Overlay<S> {
161: pub fn popover<S>(content: El<S>) -> Overlay<S> {
168: pub fn menu<S>(content: El<S>) -> Overlay<S> {
175: pub fn dialog<S>(content: El<S>) -> Overlay<S> {
182: pub fn toasts<S>(stack: El<S>) -> Overlay<S> {
189: pub fn open(mut self, open: bool) -> Self {
196: pub fn key(mut self, key: impl Into<Key>) -> Self {
202: pub fn anchor(mut self, key: impl Into<Key>) -> Self {
209: pub fn at(mut self, x: f32, y: f32) -> Self {
216: pub fn side(mut self, side: Side) -> Self {
223: pub fn align(mut self, align: Align) -> Self {
229: pub fn offset(mut self, offset: impl Into<Len>) -> Self {
235: pub fn match_width(mut self) -> Self {
241: pub fn take_focus(mut self) -> Self {
249: pub fn on_dismiss(mut self, handler: impl Fn(&mut S) + 'static) -> Self {
278: pub const TOOLTIP_DELAY: Duration = Duration::from_millis(500);
299: pub anchor: Option<Rect>,
300: pub size: (f32, f32),
302: pub bounds: Rect,
303: pub side: Option<Side>,
304: pub align: Align,
305: pub gap: f32,
```


### el/runtime/layer.rs

```rust
24: pub id: u64,
25: pub kind: Kind,
27: pub trigger: Option<u64>,
28: pub anchor: Anchor,
29: pub side: Option<Side>,
30: pub align: Align,
31: pub offset: Len,
32: pub match_width: bool,
33: pub focus: bool,
34: pub on_dismiss: Option<Dismiss<S>>,
36: pub at: usize,
37: pub len: usize,
39: pub escaped: bool,
44: pub fn panel(&self) -> usize {
48: pub fn range(&self) -> Range<usize> {
56: pub popup: Option<HasPopup>,
57: pub expanded: Option<bool>,
58: pub controls: Option<HitId>,
59: pub modal: bool,
65: pub trigger: Option<u64>,
66: pub due: Option<Duration>,
67: pub shown: bool,
73: pub kind: Kind,
74: pub escaped: bool,
80: pub struct Popup {
82: pub id: u64,
84: pub rect: Rect,
86: pub list: DisplayList,
662: pub fn wake_at(&self) -> Option<Duration> {
670: pub fn set_screen(&mut self, screen: Option<Rect>) {
682: pub fn popups(&self) -> &[Popup] {
```


### el/runtime/text.rs

```rust
261: pub fn type_text(&mut self, text: &str) {
307: pub fn preedit(&mut self, text: &str, cursor: usize) {
324: pub fn caret_rect(&self) -> Option<Rect> {
332: pub fn set_clipboard(&mut self, clipboard: impl super::super::Clipboard + 'static) {
453: pub fn sync_text_input(&mut self, surface: &crate::Surface) {
```


### el/runtime.rs

```rust
49: pub use layer::Popup;
81: pub struct Snapshot {
83: pub rect: Rect,
85: pub fill: Option<Rgba>,
87: pub opacity: f32,
94: pub struct Change {
96: pub key: Key,
98: pub from: Option<Snapshot>,
100: pub to: Option<Snapshot>,
105: pub struct Shown {
107: pub key: Option<Key>,
109: pub path: String,
111: pub role: Option<Role>,
113: pub label: Option<Arc<str>>,
115: pub rect: Rect,
117: pub fill: Option<Rgba>,
119: pub text: Option<Arc<str>>,
121: pub checked: Option<bool>,
123: pub state: State,
125: pub focused: bool,
127: pub opacity: f32,
129: pub range: Option<(f64, f64, f64)>,
131: pub value: Option<Arc<str>>,
133: pub selection: Option<(usize, usize)>,
135: pub description: Option<Arc<str>>,
137: pub selected: Option<bool>,
139: pub expanded: Option<bool>,
141: pub scroll: Option<f32>,
180: pub struct Runtime<S> {
270: pub fn new(
345: pub fn frame(&mut self, dt: Duration) -> bool {
756: pub fn handle(&mut self, event: &Event) {
1009: pub fn tab(&mut self, back: bool) {
1051: pub fn accessibility_action(&mut self, hit: HitId, action: &AccessibilityAction) -> bool {
1145: pub fn scroll_into_view(&mut self, key: &str) {
1151: pub fn scroll_offset(&self, key: &str) -> Option<f32> {
1180: pub fn update(&mut self, change: impl FnOnce(&mut S)) {
1186: pub fn update_instant(&mut self, change: impl FnOnce(&mut S)) {
1192: pub fn state(&self) -> &S {
1197: pub fn resize(&mut self, size: (u32, u32)) {
1206: pub fn set_scale(&mut self, scale: f32) {
1216: pub fn set_theme(&mut self, theme: Theme) {
1222: pub fn set_background(&mut self, background: Option<Rgba>) {
1230: pub fn set_time_scale(&mut self, scale: f32) {
1235: pub fn time_scale(&self) -> f32 {
1241: pub fn set_reduced_motion(&mut self, reduced: bool) {
1246: pub fn reduced_motion(&self) -> bool {
1252: pub fn animating(&self) -> bool {
1257: pub fn theme(&self) -> &Theme {
1262: pub fn scale(&self) -> f32 {
1267: pub fn size(&self) -> (u32, u32) {
1272: pub fn ui(&mut self) -> &mut Ui {
1277: pub fn list(&self) -> &DisplayList {
1283: pub fn semantics(&self) -> &[(HitId, Semantics)] {
1288: pub fn damage(&self) -> Option<Rect> {
1293: pub fn now(&self) -> Duration {
1298: pub fn view_runs(&self) -> u64 {
1303: pub fn paints(&self) -> u64 {
1308: pub fn touched(&self) -> usize {
1313: pub fn changes(&self) -> &[Change] {
1320: pub fn pointer_move(&mut self, x: f32, y: f32) {
1325: pub fn pointer_down(&mut self, x: f32, y: f32) {
1330: pub fn pointer_up(&mut self, x: f32, y: f32) {
1335: pub fn right_click(&mut self, x: f32, y: f32) {
1341: pub fn wheel_at(&mut self, x: f32, y: f32, delta: i16) {
1347: pub fn key(&mut self, vk: u16) {
1354: pub fn key_with(&mut self, vk: u16, mods: Modifiers) {
1362: pub fn click(&mut self, key: &str) -> bool {
1373: pub fn find(&self, key: &str) -> Option<Rect> {
1378: pub fn presented(&self, key: &str) -> Option<Shown> {
1383: pub fn tree(&self) -> Vec<Shown> {
1422: pub fn present(&mut self, surface: &mut Surface, origin: (i32, i32)) -> Result<bool> {
1432: pub fn snap(&mut self) -> Result<Image> {
```


### el/scroll.rs

```rust
20: pub axis: Axis,
24: pub max: f32,
26: pub viewport: f32,
30: pub shown: (f32, f32),
34: pub fn new(axis: Axis) -> Self {
47: pub fn laid(&mut self, rect: Rect, extent: (f32, f32), t: f64) {
60: pub fn target(&self) -> f32 {
65: pub fn offset(&self, t: f64) -> f32 {
70: pub fn shift(&self, t: f64) -> Point {
79: pub fn to(&mut self, to: f32, spring: Option<Spring>, t: f64) -> bool {
90: pub fn by(&mut self, by: f32, spring: Option<Spring>, t: f64) -> bool {
96: pub fn reveal(&mut self, start: f32, end: f32, spring: Option<Spring>, t: f64) -> bool {
108: pub fn bar(&self, t: f64, theme: &Theme) -> f32 {
123: pub fn settle(&mut self, t: f64, theme: &Theme) -> bool {
```


### el/token.rs

```rust
10: pub const PAD: Len = Len::Theme(|t| t.metrics.pad);
12: pub const GAP: Len = Len::Theme(|t| t.metrics.gap);
14: pub const ITEM: Len = Len::Theme(|t| t.metrics.item);
16: pub const ROW_SINGLE: Len = Len::Theme(|t| t.metrics.row_single);
18: pub const ROW: Len = Len::Theme(|t| t.metrics.row);
20: pub const CONTROL: Len = Len::Theme(|t| t.metrics.control);
22: pub const CONTROL_SMALL: Len = Len::Theme(|t| t.metrics.control_small);
24: pub const ICON: Len = Len::Theme(|t| t.metrics.icon);
26: pub const HIT: Len = Len::Theme(|t| t.metrics.hit);
29: pub const RING: Len = Len::Theme(|t| t.icon_weight * 2.0);
34: pub const RADIUS_SURFACE: Len = Len::Theme(|t| t.radius.surface);
36: pub const RADIUS_CARD: Len = Len::Theme(|t| t.radius.card);
38: pub const RADIUS_SMALL: Len = Len::Theme(|t| t.radius.small);
40: pub const RADIUS_CONTROL: Len = Len::Theme(|t| t.radius.control.unwrap_or(f32::INFINITY));
45: pub const LABEL: Paint = Paint::Theme(|t| t.color.label);
47: pub const SECONDARY: Paint = Paint::Theme(|t| t.color.secondary);
49: pub const TERTIARY: Paint = Paint::Theme(|t| t.color.tertiary);
51: pub const SURFACE: Paint = Paint::Theme(|t| t.color.surface);
53: pub const ELEVATED: Paint = Paint::Theme(|t| t.color.elevated);
55: pub const CARD: Paint = Paint::Theme(|t| t.color.card);
57: pub const FILL: Paint = Paint::Theme(|t| t.color.fill);
59: pub const WELL: Paint = Paint::Theme(|t| t.color.well);
61: pub const SEPARATOR: Paint = Paint::Theme(|t| t.color.separator);
63: pub const ACCENT: Paint = Paint::Theme(|t| t.color.accent);
65: pub const ON_ACCENT: Paint = Paint::Theme(|t| t.color.on_accent);
67: pub const KNOB: Paint = Paint::Theme(|t| t.color.knob);
69: pub const CARET: Paint = Paint::Theme(|t| t.color.accent);
71: pub const SELECTION: Paint = Paint::Theme(|t| t.color.accent.with_alpha(if t.dark() { 0.45 } else { 0.3 }));
76: pub const MOTION_LAYOUT: Motion = Motion::Theme(|t| t.motion.layout);
78: pub const MOTION_INTERACTIVE: Motion = Motion::Theme(|t| t.motion.interactive);
80: pub const MOTION_OPEN: Motion = Motion::Theme(|t| t.motion.open);
83: pub const SCRIM: Paint = Paint::Theme(|t| crate::Rgba::new(0.0, 0.0, 0.0, if t.dark() { 0.5 } else { 0.3 }));
```


### focus.rs

```rust
7: pub fn step(hits: &[Hit], current: Option<HitId>, delta: i32) -> Option<HitId> {
```


### geom.rs

```rust
5: pub struct Point {
7: pub x: f32,
9: pub y: f32,
14: pub const fn new(x: f32, y: f32) -> Self {
21: pub struct Rect {
23: pub left: f32,
25: pub top: f32,
27: pub right: f32,
29: pub bottom: f32,
34: pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
44: pub fn width(&self) -> f32 {
49: pub fn height(&self) -> f32 {
54: pub fn is_empty(&self) -> bool {
59: pub fn contains(&self, p: Point) -> bool {
64: pub fn intersects(&self, other: &Rect) -> bool {
72: pub fn inflate(self, by: f32) -> Self {
82: pub fn offset(self, dx: f32, dy: f32) -> Self {
92: pub fn union(self, other: Rect) -> Self {
102: pub fn intersect(self, other: Rect) -> Self {
112: pub fn center(&self) -> Point {
120: pub fn inset(self, insets: crate::layout::Insets) -> Self {
130: pub fn take_top(self, height: f32, gap: f32) -> (Self, Self) {
145: pub fn take_bottom(self, height: f32, gap: f32) -> (Self, Self) {
157: pub fn take_left(self, width: f32, gap: f32) -> (Self, Self) {
169: pub fn take_right(self, width: f32, gap: f32) -> (Self, Self) {
181: pub fn place(
194: pub fn pixels(&self, width: u32, height: u32) -> Option<PixelRect> {
210: pub struct PixelRect {
212: pub x: u32,
214: pub y: u32,
216: pub width: u32,
218: pub height: u32,
223: pub fn full(width: u32, height: u32) -> Self {
233: pub fn to_rect(self) -> Rect {
245: pub struct Rgba {
247: pub r: f32,
249: pub g: f32,
251: pub b: f32,
253: pub a: f32,
258: pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);
261: pub const HIT: Self = Self::new(0.0, 0.0, 0.0, 1.0 / 255.0);
264: pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
269: pub fn rgb8([r, g, b]: [u8; 3], a: f32) -> Self {
279: pub const fn with_alpha(self, a: f32) -> Self {
310: pub mod continuous;
```


### glass.rs

```rust
23: pub struct GlassStyle {
25: pub brightness: f32,
27: pub saturation: f32,
29: pub smoothing: f32,
31: pub sheen: Option<f32>,
33: pub depth: Option<f32>,
35: pub inner_shadow: f32,
37: pub fade_ms: f32,
39: pub fade_curve: Option<crate::transition::Easing>,
59: pub struct GlassMaterial {
61: pub clarity: f32,
64: pub blur: f32,
66: pub refraction: f32,
68: pub highlight: f32,
71: pub tint: Rgba,
76: pub struct Glass {
78: pub body: Rect,
80: pub radius: f32,
82: pub material: GlassMaterial,
87: pub struct Shape {
89: pub x: i32,
91: pub y: i32,
93: pub width: i32,
95: pub height: i32,
100: pub fn shape(&self, origin: (i32, i32)) -> Shape {
115: pub fn lensed(&self) -> bool {
120: pub use crate::platform::current::glass::{Backdrop, install};
124: pub fn set_style(material: Option<GlassMaterial>) {
131: pub fn wanted() -> bool {
138: pub fn cpu_backdrop() -> bool {
143: pub fn style() -> Option<GlassMaterial> {
148: pub fn available() -> bool {
```


### golden.rs

```rust
22: pub fn check(image: &Image, path: &Path) -> Result<()> {
76: pub fn write(image: &Image, path: &Path) -> Result<()> {
105: pub fn read(path: &Path) -> Result<Image> {
```


### icons.rs

```rust
20: pub const SYSTEM_FAMILY: &str = "Segoe Fluent Icons";
23: pub const CHECK: char = '\u{E73E}';
25: pub const CHEVRON_RIGHT: char = '\u{E76C}';
27: pub use crate::platform::current::icons::{system_family, centered};
31: pub enum IconSet {
39: pub fn from_system_glyph(glyph: char) -> Option<Icon> {
81: pub enum Icon {
334: pub const fn glyph(self) -> char {
343: pub const fn font_glyph(self) -> Option<char> {
427: pub fn glyph_in(self, family: &str) -> char {
442: pub const fn filled(self) -> bool {
```


### identity.rs

```rust
7: pub struct Identity {
9: pub prefix: &'static str,
13: pub desktop_classes: &'static [&'static str],
32: pub fn set_identity(identity: Identity) -> Result<(), Identity> {
```


### interact.rs

```rust
15: pub use crate::interaction_motion as motion;
24: pub enum State {
38: pub fn wash(self, ink: Rgba) -> Rgba {
47: pub fn press_scale(self) -> f32 {
54: pub enum Response {
65: pub struct Interaction {
67: pub hover: Option<HitId>,
69: pub pressed: Option<HitId>,
71: pub focus: Option<HitId>,
73: pub focus_visible: bool,
78: pub fn state(&self, id: HitId, enabled: bool) -> State {
91: pub fn ring(&self, id: HitId) -> bool {
96: pub fn reset(&mut self) -> Response {
107: pub fn handle(&mut self, list: &DisplayList, event: &Event) -> Response {
164: pub fn step(&mut self, list: &DisplayList, delta: i32) {
```


### interaction_motion.rs

```rust
36: pub struct InteractionMotion {
38: pub hover: Timing,
40: pub press: Timing,
47: pub fn new(hover: Timing, press: Timing) -> Self {
56: pub fn moving(&self) -> bool {
60: pub fn step(&mut self, input: &Interaction, now: Instant) -> bool {
76: pub fn wash(&self, id: HitId, hover: Rgba, press: Rgba, fallback: Rgba) -> Rgba {
```


### kit.rs

```rust
32: pub struct Frame<'a> {
34: pub text: &'a mut Text,
36: pub theme: &'a Theme,
38: pub s: f32,
40: pub list: DisplayList,
42: pub input: &'a Interaction,
48: pub enum ButtonKind {
63: pub enum Lead {
78: pub enum Accessory<'a> {
95: pub struct Row<'a> {
97: pub id: HitId,
99: pub lead: Lead,
101: pub title: &'a str,
103: pub subtitle: Option<&'a str>,
105: pub accessory: Accessory<'a>,
107: pub selected: bool,
109: pub enabled: bool,
115: pub struct Tile<'a> {
117: pub id: HitId,
119: pub icon: Icon,
121: pub title: &'a str,
123: pub subtitle: &'a str,
125: pub on: bool,
127: pub enabled: bool,
129: pub more: Option<HitId>,
133: pub const NONE: HitId = 0;
137: pub fn new(text: &'a mut Text, theme: &'a Theme, s: f32, input: &'a Interaction) -> Self {
149: pub fn with_motion(mut self, motion: &'a crate::interact::motion::InteractionMotion) -> Self {
155: pub fn px(&self, v: f32) -> f32 {
160: pub fn font(&self, role: TextRole) -> TextStyle {
165: pub fn line(&self, role: TextRole) -> f32 {
170: pub fn push(&mut self, op: Op) {
175: pub fn fill(&mut self, rect: Rect, radius: f32, color: Rgba) {
184: pub fn hit(
208: pub fn state(&self, id: HitId, enabled: bool) -> State {
216: pub fn control_radius(&self, height: f32) -> f32 {
224: pub fn wash(&mut self, id: HitId, rect: Rect, radius: f32, enabled: bool) {
241: pub fn focus_ring(&mut self, id: HitId, rect: Rect, radius: f32) {
262: pub fn surface(&mut self, rect: Rect, glass_available: bool) -> Option<Glass> {
269: pub fn surface_shaped(&mut self, rect: Rect, radius: f32, glass_available: bool) -> Option<Glass> {
296: pub fn fill_continuous(&mut self, rect: Rect, radius: f32, smoothing: f32, color: Rgba) {
306: pub fn card(&mut self, rect: Rect) -> Rect {
313: pub fn separator(&mut self, left: f32, right: f32, y: f32) {
323: pub fn separator_v(&mut self, top: f32, bottom: f32, x: f32) {
335: pub fn label(
349: pub fn label_styled(
370: pub fn text_in(
394: pub fn paragraph(
426: pub fn line_count(&mut self, text: &str, role: TextRole, width: f32, lines: usize) -> usize {
433: pub fn icon(&mut self, rect: Rect, icon: Icon, size: f32, color: Rgba) {
457: pub fn button_size(&mut self, label: &str, icon: bool) -> (f32, f32) {
472: pub fn button(
512: pub fn icon_button(
533: pub fn quiet_icon_button(&mut self, id: HitId, rect: Rect, icon: Icon, label: &str, enabled: bool, selected: bool) {
544: pub fn switch(
598: pub fn slider(&mut self, id: HitId, rect: Rect, value: f32, label: &str, icon: Option<Icon>) {
659: pub fn segmented(&mut self, base: HitId, rect: Rect, labels: &[&str], selected: usize) {
704: pub fn row_height(&self, row: &Row) -> f32 {
714: pub fn row(&mut self, row: &Row, rect: Rect) {
849: pub fn section_header(&mut self, rect: Rect, title: &str, action: Option<(HitId, &str)>) {
873: pub fn tile(&mut self, tile: &Tile, rect: Rect) {
918: pub fn progress(&mut self, rect: Rect, value: f32, color: Option<Rgba>) {
934: pub fn ring(&mut self, center: Point, radius: f32, width: f32, value: f32, color: Option<Rgba>) {
953: pub fn badge(&mut self, x: f32, center_y: f32, text: &str, color: Option<Rgba>) -> f32 {
976: pub fn tooltip(&mut self, anchor: Rect, text: &str, bounds: Rect) -> Rect {
1014: pub fn field_frame(
1062: pub fn empty(&mut self, rect: Rect, icon: Option<Icon>, title: &str, detail: Option<&str>) -> f32 {
1106: pub fn scroll(
```


### layout.rs

```rust
18: pub struct Insets {
20: pub top: f32,
22: pub right: f32,
24: pub bottom: f32,
26: pub left: f32,
31: pub const fn all(v: f32) -> Self {
41: pub const fn xy(x: f32, y: f32) -> Self {
53: pub enum Align {
65: pub fn offset(self, start: f32, room: f32, size: f32) -> f32 {
76: pub enum Axis {
85: pub struct Stack {
87: pub rect: Rect,
89: pub axis: Axis,
91: pub gap: f32,
93: pub cursor: f32,
98: pub fn column(rect: Rect, gap: f32) -> Self {
108: pub fn row(rect: Rect, gap: f32) -> Self {
118: pub fn next(&mut self, size: f32) -> Rect {
128: pub fn space(&mut self, space: f32) {
133: pub fn rest(&self) -> Rect {
147: pub fn used(&self) -> f32 {
158: pub enum Length {
167: pub fn split(rect: Rect, axis: Axis, gap: f32, lengths: &[Length], out: &mut [Rect]) {
202: pub fn grid(rect: Rect, columns: usize, gap: f32, height: f32, index: usize) -> Rect {
216: pub enum SizeClass {
227: pub fn of(width: f32, height: f32) -> Self {
```


### lib.rs

```rust
15: pub mod platform;
17: pub use identity::{Identity, set_identity};
18: pub use a11y::{AccessibilityAction, Detail, FieldText, Semantics, TextLine};
19: pub use platform::current::app_window;
20: pub mod calendar;
21: pub mod chart;
22: pub mod color_row;
23: pub use platform::current::compose;
26: pub mod controls;
27: pub mod damage;
28: pub mod el;
29: pub mod focus;
30: pub mod geom;
31: pub mod glass;
33: pub mod golden;
35: pub mod icons;
36: pub mod interact;
37: pub mod interaction_motion;
38: pub mod kit;
39: pub use platform::current::layered_app;
40: pub mod layout;
41: pub mod menu;
42: pub use platform::current::modules;
43: pub mod motion;
44: pub use platform::current::render;
45: pub mod reveal;
50: pub mod scroll;
51: pub mod sortable;
54: pub use platform::current::text_field;
55: pub mod theme;
57: pub mod transition;
60: pub use app_window::{Chrome, Cursor, set_modal_hook};
61: pub use geom::{PixelRect, Point, Rect, Rgba};
62: pub use glass::{Glass, GlassMaterial};
63: pub use interact::Interaction;
64: pub use kit::{Accessory, ButtonKind, Frame, Lead, Row as ListRow, Tile};
65: pub use layout::{Align, Insets, SizeClass, Stack};
66: pub use menu::{Entry, Menu, MenuIcon, Outcome, Screen, screen_at};
67: pub use motion::{Spring, Ticker, Tween, display_frame};
68: pub use render::{Image, TextMode, TextQuality, Ui, render};
69: pub use scene::{DisplayList, Hit, HitId, Op};
70: pub use scroll::Scroll;
71: pub use surface::AnimationFrames;
72: pub use surface::{Button, Event, Kind, Pointer, Surface, SurfaceId, next_event, queue_event};
73: pub use text::{Text, TextStyle, VARIABLE};
74: pub use text_field::TextField;
75: pub use theme::{Palette, Scheme, ShadowLevel, SoftShadow, Style, TextRole, Theme};
76: pub use tokens::{Colors, Ink, Material, MenuMetrics, Step, Tokens, Typeface};
77: pub use watch::FileWatch;
80: pub use accesskit::Role;
```


### menu.rs

```rust
32: pub fn set_interaction_motion(motion: Option<InteractionTimings>) {
38: pub struct MenuMotion {
40: pub timing: crate::transition::Timing,
42: pub style: crate::reveal::Reveal,
46: pub fn set_open_motion(motion: Option<MenuMotion>) {
51: pub fn set_style(tokens: Option<Tokens>) {
57: pub enum Entry<C> {
91: pub enum MenuIcon {
109: pub const ICON_SIZE: f32 = 16.0;
111: pub const RADIO: char = '\u{E915}';
115: pub fn action(label: &str, command: C) -> Self {
125: pub fn check(label: &str, command: C, on: bool) -> Self {
135: pub fn radio(label: &str, command: C, on: bool) -> Self {
140: pub fn with_icon(self, icon: MenuIcon) -> Self {
152: pub fn plain(&self) -> &Self {
160: pub fn icon(&self) -> Option<&MenuIcon> {
168: pub fn selectable(&self) -> bool {
177: pub fn label(&self) -> &str {
186: pub type Edges = (i32, i32, i32, i32);
188: pub use crate::platform::current::screen::{Screen, screen_at};
192: pub struct Row {
194: pub top: f32,
196: pub height: f32,
198: pub label: (f32, f32),
203: pub struct Layout {
205: pub width: f32,
207: pub height: f32,
209: pub rows: Vec<Row>,
211: pub check: (f32, f32),
213: pub chevron: (f32, f32),
215: pub scale: f32,
220: pub fn size(&self) -> (i32, i32) {
233: pub fn layout<C>(items: &[Entry<C>], tokens: &Tokens, scale: f32, text: &mut Text) -> Layout {
270: pub fn place(size: (i32, i32), anchor: (i32, i32), work: Edges) -> (i32, i32) {
291: pub fn place_submenu(size: (i32, i32), parent: Edges, row_top: i32, work: Edges) -> (i32, i32) {
313: pub fn view<C>(
323: pub fn view_animated<C>(
516: pub enum Outcome<C> {
568: pub struct Menu<C> {
585: pub fn new(name: &str) -> Result<Self> {
604: pub fn set_presentation(&mut self, tokens: Tokens, opening: MenuMotion) {
609: pub fn set_interaction_timings(
618: pub fn open(
679: pub fn close(&mut self) {
684: pub fn is_open(&self) -> bool {
689: pub fn contains(&self, x: i32, y: i32) -> bool {
694: pub fn owns(&self, id: SurfaceId) -> bool {
699: pub fn handle(&mut self, ui: &mut Ui, id: SurfaceId, event: &Event) -> Option<Outcome<C>> {
```


### motion.rs

```rust
11: pub struct Spring {
13: pub mass: f32,
15: pub stiffness: f32,
17: pub damping: f32,
22: pub const DEFAULT: Self = Self::new(1.0, 220.0, 22.0);
24: pub const SOFT: Self = Self::new(1.0, 120.0, 17.0);
26: pub const BOUNCY: Self = Self::new(1.0, 260.0, 13.0);
29: pub const fn new(mass: f32, stiffness: f32, damping: f32) -> Self {
42: pub const fn apple(duration: f32, bounce: f32) -> Self {
61: pub const SMOOTH: Self = Self::apple(0.5, 0.0);
63: pub const SNAPPY: Self = Self::apple(0.5, 0.15);
65: pub const INTERACTIVE: Self = Self::apple(0.15, 0.14);
68: pub fn at(&self, t: f32) -> f32 {
95: pub fn released(&self, d0: f32, v0: f32, t: f32) -> (f32, f32) {
130: pub fn calm(self) -> Self {
139: pub fn faster(self, by: f32) -> Self {
149: pub fn settle(&self, epsilon: f32) -> f32 {
161: pub struct Tween {
163: pub from: f32,
165: pub to: f32,
173: pub fn new(from: f32, to: f32, spring: Spring, now: Instant) -> Self {
184: pub fn value(&self, now: Instant) -> f32 {
193: pub fn done(&self, now: Instant) -> bool {
198: pub use crate::platform::current::timing::{Ticker, display_frame};
```


### platform/linux/app_window.rs

```rust
4: pub enum Chrome {
16: pub enum Cursor {
32: pub fn set_modal_hook(_: Option<Box<dyn FnMut()>>) {}
```


### platform/linux/clipboard.rs

```rust
26: pub manager: Option<wl_data_device_manager::WlDataDeviceManager>,
43: pub struct System;
```


### platform/linux/glass.rs

```rust
6: pub trait Backdrop {
42: pub fn install(backdrop: Rc<dyn Backdrop>) {
```


### platform/linux/gpu.rs

```rust
71: pub struct Canvas {
80: pub fn new(width: u32, height: u32) -> Result<Self> {
109: pub fn draw(&mut self, ui: &mut Ui, list: &DisplayList, area: PixelRect) -> Result<()> {
176: pub fn image(&self) -> Result<Image> {
225: pub fn render(ui: &mut Ui, list: &DisplayList, width: u32, height: u32) -> Result<Image> {
233: pub glasses: Vec<crate::Glass>,
234: pub style: crate::glass::GlassStyle,
235: pub spacing: f32,
236: pub backdrop: Option<Image>,
237: pub revision: u64,
238: pub backdrop_revision: u64,
255: pub unsafe fn new(
304: pub fn draw(
340: pub fn image(&self) -> Result<Image> {
344: pub fn opacity(&mut self, alpha: u8) {
354: pub fn set_alpha(&mut self, alpha: u8) -> Result<()> {
358: pub fn glass_revision(&self) -> u64 {
361: pub fn set_glass(
382: pub fn animating(&self) -> bool {
385: pub fn present(&mut self) -> Result<()> {
```


### platform/linux/gpu_glass.rs

```rust
16: pub fn new(gpu: &Device, size: (u32, u32), image: Option<&Image>) -> Self {
72: pub fn set(&mut self, glasses: &[Glass], style: &GlassStyle, spacing: f32, scale: f32) {
138: pub fn render(
174: pub fn new(gpu: &Device, alpha: f32) -> Self {
243: pub fn draw(
```


### platform/linux/host.rs

```rust
3: pub fn screen_size() -> (i32, i32) {
7: pub fn shift_down() -> bool {
11: pub fn modifiers() -> crate::el::Modifiers {
16: pub fn wake_at(deadline: Option<std::time::Instant>) {
21: pub fn wait() {
28: pub fn reduced_motion() -> bool {
```


### platform/linux/icons.rs

```rust
5: pub fn system_family(_: &Text) -> Arc<str> {
9: pub fn centered(_: &mut Text, _: &str, icon: Icon, size: f32, rect: Rect, color: Rgba) -> Op {
```


### platform/linux/ime.rs

```rust
17: pub manager: Option<zwp_text_input_manager_v3::ZwpTextInputManagerV3>,
41: pub fn follow(id: SurfaceId, caret: Option<Rect>) {
79: pub fn preedit() -> (String, usize) {
```


### platform/linux/keyboard.rs

```rust
4: pub struct Keyboard {
18: pub enum Typed {
25: pub fn new(fd: std::os::fd::OwnedFd, size: usize) -> Option<Self> {
60: pub fn modifiers(&mut self, depressed: u32, latched: u32, locked: u32, group: u32) {
74: pub fn shift_down(&self) -> bool {
85: pub fn modifiers_held(&self) -> crate::el::Modifiers {
100: pub fn typed(&mut self, key: u32) -> Typed {
123: pub fn text(&self, key: u32) -> String {
139: pub fn symbol(&self, key: u32) -> u32 {
```


### platform/linux/mod.rs

```rust
3: pub mod app_window;
5: pub mod glass;
7: pub mod gpu;
10: pub mod caret;
11: pub mod clipboard;
12: pub mod ime;
13: pub mod host;
15: pub mod render;
16: pub mod screen;
18: pub mod surface;
19: pub mod text;
20: pub mod text_field;
21: pub mod timing;
24: pub mod layered_app {}
26: pub mod compose {}
27: pub mod modules;
29: pub mod identity {
30: pub use crate::identity::{Identity, set_identity};
33: pub mod watch {
35: pub struct FileWatch;
37: pub use render::{Image, Ui};
38: pub use surface::{Event, Kind, Surface, SurfaceId, next_event};
39: pub use text::Text;
40: pub use text_field::TextField;
41: pub mod icons;
42: pub use render::Canvas as Renderer;
44: pub struct Backend;
```


### platform/linux/modules.rs

```rust
3: pub fn loaded() -> Vec<String> {
16: pub fn is_gpu_driver(module: &str) -> bool {
```


### platform/linux/render.rs

```rust
3: pub use crate::bitmap::{Image, TextMode, TextQuality};
14: pub struct Ui {
16: pub text: Text,
20: pub fn new() -> Result<Self> {
24: pub fn isolated() -> Result<Self> {
28: pub fn set_text_quality(&mut self, _: TextQuality) -> Result<()> {
32: pub fn decode_image(&self, bytes: &[u8], max: (u32, u32)) -> Result<Image> {
54: pub fn load(ui: &Ui, path: &std::path::Path) -> Result<Self> {
59: pub fn render(ui: &mut Ui, list: &DisplayList, width: u32, height: u32) -> Result<Image> {
75: pub struct Canvas {
80: pub fn new(_: &Ui, width: u32, height: u32) -> Result<Self> {
97: pub fn size(&self) -> (u32, u32) {
102: pub fn image(&self) -> Image {
106: pub fn draw(&self, text: &mut Text, list: &DisplayList, area: PixelRect) -> Result<()> {
148: pub fn read(&self, area: PixelRect, mut sink: impl FnMut(u32, &[u8])) -> Result<()> {
```


### platform/linux/screen.rs

```rust
4: pub struct Screen {
6: pub work: crate::menu::Edges,
8: pub dpi: u32,
11: pub fn screen_at(_: i32, _: i32) -> Screen {
```


### platform/linux/surface.rs

```rust
5: pub enum Kind {
19: pub struct SurfaceId(isize);
23: pub enum Button {
34: pub struct Pointer {
36: pub at: Point,
38: pub screen: (i32, i32),
43: pub enum Event {
107: pub use wayland_protocols::xdg::shell::client::xdg_toplevel::ResizeEdge;
234: pub struct AnimationFrames(Rc<Cell<bool>>);
256: pub fn next_event() -> Option<(SurfaceId, Event)> {
284: pub fn connection_fd() -> Result<std::os::fd::RawFd> {
344: pub fn queue_event(id: usize, event: Event) {
351: pub struct Surface {
364: pub fn new(kind: Kind, name: &str, role: accesskit::Role) -> Result<Self> {
368: pub fn popup(parent: &Surface, name: &str, at: (i32, i32)) -> Result<Self> {
485: pub fn new_deferred_accessibility(k: Kind, n: &str, r: accesskit::Role) -> Result<Self> {
489: pub fn present(
689: pub fn image(&self) -> Result<crate::Image> {
703: pub fn shown(&self) -> &DisplayList {
707: pub fn id(&self) -> SurfaceId {
711: pub fn origin(&self) -> (i32, i32) {
715: pub fn scale(&self) -> f32 {
719: pub fn place(&self, (_, _, width, height): (i32, i32, i32, i32)) {
739: pub fn client_size(&self) -> (u32, u32) {
743: pub fn show_active(&self, _: bool) {
747: pub fn set_background(&self, color: crate::Rgba) {
767: pub fn animation_frames(&self, duration: Duration) -> AnimationFrames {
784: pub fn set_alpha(&mut self, alpha: u8) -> Result<()> {
810: pub fn move_to(&mut self, _: (i32, i32)) -> Result<()> {
814: pub fn set_accessibility_semantics(&self, semantics: Vec<(crate::HitId, crate::Semantics)>) {
818: pub fn take_accessibility_action(&self, hit: crate::HitId) -> Option<crate::AccessibilityAction> {
822: pub fn set_glass(&self, glass: Option<crate::Glass>) {
826: pub fn set_glass_style(&self, style: crate::glass::GlassStyle) {
841: pub fn set_glass_backdrop(&self, image: crate::Image) -> Result<()> {
855: pub fn with_render(
866: pub fn render(&self) -> crate::platform::Render {
870: pub fn glass_available(&self) -> bool {
874: pub fn set_glass_group(&self, glasses: &[crate::Glass], spacing: f32) {
888: pub fn show(&self) {
935: pub fn hide(&self) {
947: pub fn release(&mut self) {
955: pub fn discard_events(&self) {
962: pub fn take_focus(&self) -> &'static str {
966: pub fn set_chrome(&self, parts: &[(crate::Rect, super::app_window::Chrome)]) {
975: pub fn minimize(&self) -> Result<()> {
985: pub fn toggle_maximize(&self) -> Result<()> {
1004: pub fn start_move(&self) -> Result<()> {
1016: pub fn start_resize(&self, edge: xdg_toplevel::ResizeEdge) -> Result<()> {
```


### platform/linux/text.rs

```rust
2: pub use crate::text_style::{TextStyle, VARIABLE};
7: pub struct Text {
15: pub fn new() -> anyhow::Result<Self> {
99: pub fn measure(&mut self, text: &str, style: &TextStyle) -> (f32, f32) {
115: pub fn clear_cache(&mut self) {
119: pub fn set_capacity(&mut self, capacity: usize) {
124: pub fn has_family(&self, name: &str) -> bool {
131: pub fn families(&self) -> Vec<String> {
143: pub fn fit(&mut self, text: &str, style: &TextStyle, width: f32) -> Arc<str> {
158: pub fn lines(
```


### platform/linux/text_field.rs

```rust
20: pub struct TextField(Rc<RefCell<State>>);
29: pub fn new(
47: pub fn revision(&self) -> u64 {
51: pub fn text(&self) -> anyhow::Result<String> {
55: pub fn set_text(&self, text: &str) -> anyhow::Result<()> {
65: pub fn set_style(&self, style: &TextStyle, color: Rgba) {
72: pub fn set_rect(&self, rect: Rect) {
76: pub fn focus(&self) {
80: pub fn blur(&self) {
88: pub fn caret(&self) -> Option<Rect> {
92: pub fn multiline(&self) -> bool {
```


### platform/linux/timing.rs

```rust
4: pub struct Ticker {
9: pub fn new() -> anyhow::Result<Self> {
13: pub fn schedule(&mut self, moving: bool) {
17: pub fn armed(&self) -> bool {
22: pub fn display_frame() -> std::time::Duration {
```


### platform/macos/app_window.rs

```rust
13: pub enum Chrome {
26: pub enum Cursor {
48: pub fn set_modal_hook(hook: Option<Box<dyn FnMut()>>) {
```


### platform/macos/clipboard.rs

```rust
7: pub struct System;
```


### platform/macos/glass.rs

```rust
20: pub trait Backdrop {
49: pub fn install(backdrop: Rc<dyn Backdrop>) {
54: pub fn install_system() {
77: pub struct SystemBackdrop {
```


### platform/macos/gpu.rs

```rust
65: pub struct Canvas {
74: pub fn new(width: u32, height: u32) -> Result<Self> {
87: pub fn draw(&mut self, ui: &mut Ui, list: &DisplayList, area: PixelRect) -> Result<()> {
129: pub fn image(&self) -> Result<Image> {
168: pub fn render(ui: &mut Ui, list: &DisplayList, width: u32, height: u32) -> Result<Image> {
177: pub glasses: Vec<crate::Glass>,
178: pub style: crate::glass::GlassStyle,
179: pub spacing: f32,
180: pub backdrop: Option<Image>,
181: pub revision: u64,
182: pub backdrop_revision: u64,
202: pub unsafe fn new(view: &super::view::View, size: (u32, u32)) -> Result<Self> {
227: pub fn draw(&mut self, ui: &mut Ui, list: &DisplayList, size: (u32, u32), area: PixelRect, glass: &GlassState, scale: f32) -> Result<()> {
249: pub fn image(&self) -> Result<Image> {
253: pub fn glass_revision(&self) -> u64 {
272: pub fn present(&mut self) -> Result<()> {
```


### platform/macos/host.rs

```rust
10: pub fn screen_size() -> (i32, i32) {
16: pub fn shift_down() -> bool {
22: pub fn modifiers() -> Modifiers {
37: pub fn wake_at(deadline: Option<std::time::Instant>) {
43: pub fn wait() {
48: pub fn wait_for(timeout: std::time::Duration) {
54: pub fn reduced_motion() -> bool {
```


### platform/macos/icons.rs

```rust
6: pub fn system_family(_: &Text) -> Arc<str> {
11: pub fn centered(_: &mut Text, _: &str, icon: Icon, size: f32, rect: Rect, color: Rgba) -> Op {
```


### platform/macos/ime.rs

```rust
20: pub fn preedit() -> (String, usize) {
26: pub fn follow(id: SurfaceId, caret: Option<Rect>) {
```


### platform/macos/mod.rs

```rust
8: pub mod app_window;
9: pub mod caret;
10: pub mod clipboard;
12: pub mod glass;
14: pub mod gpu;
20: pub mod host;
21: pub mod icons;
22: pub mod ime;
24: pub mod modules;
26: pub mod render;
27: pub mod screen;
28: pub mod surface;
29: pub mod text;
30: pub mod text_field;
31: pub mod timing;
34: pub mod layered_app {}
36: pub mod compose {}
38: pub mod identity {
39: pub use crate::identity::{Identity, set_identity};
42: pub mod watch {
44: pub struct FileWatch;
46: pub use render::{Image, Ui};
47: pub use surface::{Event, Kind, Surface, SurfaceId, next_event};
48: pub use text::Text;
49: pub use text_field::TextField;
50: pub use render::Canvas as Renderer;
52: pub struct Backend;
```


### platform/macos/modules.rs

```rust
10: pub fn loaded() -> Vec<String> {
26: pub fn is_gpu_driver(module: &str) -> bool {
```


### platform/macos/pump.rs

```rust
117: pub fn next_event() -> Option<(SurfaceId, Event)> {
```


### platform/macos/render.rs

```rust
5: pub use crate::bitmap::{Image, TextMode, TextQuality};
20: pub struct Ui {
22: pub text: Text,
27: pub fn new() -> Result<Self> {
31: pub fn isolated() -> Result<Self> {
35: pub fn set_text_quality(&mut self, _: TextQuality) -> Result<()> {
39: pub fn decode_image(&self, bytes: &[u8], max: (u32, u32)) -> Result<Image> {
77: pub fn load(ui: &Ui, path: &std::path::Path) -> Result<Self> {
84: pub fn render(ui: &mut Ui, list: &DisplayList, width: u32, height: u32) -> Result<Image> {
97: pub struct Canvas {
116: pub fn new(_: &Ui, width: u32, height: u32) -> Result<Self> {
128: pub fn size(&self) -> (u32, u32) {
141: pub fn image(&self) -> Image {
154: pub fn draw(&self, text: &mut Text, list: &DisplayList, area: PixelRect) -> Result<()> {
164: pub fn read(&self, area: PixelRect, mut sink: impl FnMut(u32, &[u8])) -> Result<()> {
```


### platform/macos/screen.rs

```rust
13: pub struct Screen {
15: pub work: crate::menu::Edges,
17: pub dpi: u32,
24: pub size: (i32, i32),
25: pub height: f64,
26: pub scale: f64,
66: pub fn screen_at(x: i32, y: i32) -> Screen {
```


### platform/macos/surface.rs

```rust
40: pub enum Kind {
53: pub struct SurfaceId(isize);
64: pub enum Button {
75: pub struct Pointer {
77: pub at: Point,
79: pub screen: (i32, i32),
84: pub enum Event {
134: pub use super::pump::next_event;
137: pub fn queue_event(id: usize, event: Event) {
142: pub struct AnimationFrames(Rc<Cell<bool>>);
275: pub struct Surface {
297: pub fn new(kind: Kind, name: &str, role: accesskit::Role) -> Result<Self> {
302: pub fn new_deferred_accessibility(kind: Kind, name: &str, role: accesskit::Role) -> Result<Self> {
307: pub fn with_render(kind: Kind, name: &str, role: accesskit::Role, render: crate::platform::Render) -> Result<Self> {
364: pub fn render(&self) -> crate::platform::Render {
374: pub fn present(&mut self, ui: &mut Ui, origin: (i32, i32), size: (u32, u32), list: DisplayList) -> Result<bool> {
444: pub fn image(&self) -> Result<crate::Image> {
453: pub fn shown(&self) -> &DisplayList {
458: pub fn id(&self) -> SurfaceId {
463: pub fn origin(&self) -> (i32, i32) {
468: pub fn animation_frames(&self, duration: Duration) -> AnimationFrames {
475: pub fn set_alpha(&mut self, alpha: u8) -> Result<()> {
481: pub fn move_to(&mut self, origin: (i32, i32)) -> Result<()> {
492: pub fn set_accessibility_semantics(&self, semantics: Vec<(HitId, crate::Semantics)>) {
497: pub fn take_accessibility_action(&self, hit: HitId) -> Option<crate::AccessibilityAction> {
503: pub fn set_glass(&self, glass: Option<crate::Glass>) {
508: pub fn set_glass_style(&self, style: crate::glass::GlassStyle) {
529: pub fn set_glass_backdrop(&self, image: crate::Image) -> Result<()> {
543: pub fn glass_available(&self) -> bool {
549: pub fn set_glass_group(&self, glasses: &[crate::Glass], spacing: f32) {
620: pub fn show(&self) {
628: pub fn hide(&self) {
634: pub fn is_visible(&self) -> bool {
641: pub fn set_click_through(&self, through: bool) {
648: pub fn release(&mut self) {
655: pub fn discard_events(&self) {
660: pub fn take_focus(&self) -> &'static str {
673: pub fn scale(&self) -> f32 {
678: pub fn place(&self, (x, y, w, h): (i32, i32, i32, i32)) {
684: pub fn client_size(&self) -> (u32, u32) {
689: pub fn show_active(&self, maximized: bool) {
700: pub fn is_maximized(&self) -> bool {
705: pub fn minimize(&self) {
710: pub fn toggle_maximize(&self) {
715: pub fn set_chrome(&self, parts: &[(crate::Rect, Chrome)]) {
723: pub fn set_background(&self, color: crate::Rgba) {
732: pub fn set_cursor(&self, cursor: Cursor) {
```


### platform/macos/text.rs

```rust
6: pub use crate::text_style::{TextStyle, VARIABLE};
23: pub const SYSTEM: &str = "SF Pro";
30: pub struct Layout {
45: pub struct Text {
54: pub fn new() -> anyhow::Result<Self> {
112: pub fn measure(&mut self, text: &str, style: &TextStyle) -> (f32, f32) {
121: pub fn clear_cache(&mut self) {
126: pub fn set_capacity(&mut self, capacity: usize) {
143: pub fn has_family(&self, name: &str) -> bool {
148: pub fn families(&self) -> Vec<String> {
158: pub fn fit(&mut self, text: &str, style: &TextStyle, width: f32) -> Arc<str> {
182: pub fn lines(&mut self, text: &str, style: &TextStyle, width: f32, max: usize) -> Vec<Arc<str>> {
```


### platform/macos/text_field.rs

```rust
24: pub struct TextField(Rc<RefCell<State>>);
36: pub fn new(window: &Surface, rect: Rect, multiline: bool, size: f32, color: Rgba) -> anyhow::Result<Self> {
48: pub fn revision(&self) -> u64 {
52: pub fn text(&self) -> anyhow::Result<String> {
56: pub fn set_text(&self, text: &str) -> anyhow::Result<()> {
66: pub fn set_style(&self, style: &TextStyle, color: Rgba) {
73: pub fn set_rect(&self, rect: Rect) {
77: pub fn focus(&self) {
81: pub fn blur(&self) {
89: pub fn caret(&self) -> Option<Rect> {
93: pub fn multiline(&self) -> bool {
```


### platform/macos/timing.rs

```rust
8: pub struct Ticker {
13: pub fn new() -> anyhow::Result<Self> {
17: pub fn schedule(&mut self, moving: bool) {
21: pub fn armed(&self) -> bool {
27: pub fn display_frame() -> std::time::Duration {
```


### platform/macos/view.rs

```rust
25: pub id: SurfaceId,
27: pub app: bool,
28: pub chrome: RefCell<Vec<(Rect, Chrome)>>,
30: pub hits: RefCell<Vec<Rect>>,
33: pub caret: Cell<Option<Rect>>,
35: pub marked: RefCell<String>,
36: pub cursor: Cell<Cursor>,
37: pub a11y: RefCell<Option<super::a11y::A11y>>,
```


### platform/mod.rs

```rust
3: pub mod linux;
5: pub use linux as current;
7: pub mod macos;
9: pub use macos as current;
11: pub mod stub;
13: pub mod windows;
17: pub use stub as current;
19: pub use windows as current;
21: pub trait Renderer: Sized {
32: pub trait TextSystem {
47: pub trait Window: Sized {
62: pub trait TextInput {
79: pub enum Render {
99: pub fn resolve(self, wants_glass: bool, cpu_backdrop: bool) -> Render {
112: pub fn set_render(render: Render) {
119: pub fn render() -> Render {
132: pub trait Backend {
145: pub const DOWN: u16 = 0x28;
146: pub const UP: u16 = 0x26;
147: pub const LEFT: u16 = 0x25;
148: pub const RIGHT: u16 = 0x27;
149: pub const ESCAPE: u16 = 0x1b;
150: pub const RETURN: u16 = 0x0d;
```


### platform/stub/glass.rs

```rust
6: pub trait Backdrop {
38: pub fn install(backdrop: Rc<dyn Backdrop>) {
```


### platform/stub/host.rs

```rust
3: pub fn screen_size() -> (i32, i32) {
6: pub fn shift_down() -> bool {
9: pub fn modifiers() -> crate::el::Modifiers {
12: pub fn wake_at(_: Option<std::time::Instant>) {
15: pub fn wait() {
19: pub fn reduced_motion() -> bool {
```


### platform/stub/icons.rs

```rust
6: pub fn system_family(text: &Text) -> Arc<str> {
15: pub fn centered(
```


### platform/stub/mod.rs

```rust
2: pub mod glass;
3: pub mod caret;
4: pub mod host;
5: pub mod render;
6: pub mod screen;
7: pub mod surface;
8: pub mod text;
9: pub mod text_field;
10: pub mod timing;
11: pub mod app_window {
14: pub enum Chrome {
20: pub enum Cursor {}
21: pub fn set_modal_hook(_: Option<fn(bool)>) {
25: pub mod layered_app {}
26: pub mod compose {}
27: pub mod modules {}
29: pub mod identity {
30: pub use crate::identity::{Identity, set_identity};
32: pub mod watch {
33: pub struct FileWatch;
35: pub use render::{Image, Ui};
36: pub use surface::{Event, Kind, Surface, SurfaceId, next_event};
37: pub use text::Text;
38: pub use text_field::TextField;
39: pub mod icons;
40: pub use render::Canvas as Renderer;
42: pub struct Backend;
```


### platform/stub/render.rs

```rust
1: pub use crate::bitmap::{Image, TextMode, TextQuality};
3: pub struct Ui {
4: pub text: super::Text,
7: pub fn new() -> Result<Self> {
10: pub fn isolated() -> Result<Self> {
13: pub fn set_text_quality(&mut self, _: TextQuality) -> Result<()> {
16: pub fn decode_image(&self, _: &[u8], _: (u32, u32)) -> Result<Image> {
21: pub fn load(_: &Ui, _: &std::path::Path) -> Result<Self> {
25: pub fn render(_: &mut Ui, _: &crate::DisplayList, _: u32, _: u32) -> Result<Image> {
28: pub struct Canvas;
```


### platform/stub/screen.rs

```rust
2: pub struct Screen {
3: pub work: crate::menu::Edges,
4: pub dpi: u32,
6: pub fn screen_at(_: i32, _: i32) -> Screen {
```


### platform/stub/surface.rs

```rust
5: pub enum Kind {
23: pub struct SurfaceId(isize);
26: pub enum Button {
34: pub struct Pointer {
36: pub at: Point,
38: pub screen: (i32, i32),
43: pub enum Event {
79: pub struct AnimationFrames;
80: pub fn next_event() -> Option<(SurfaceId, Event)> {
83: pub fn queue_event(_: usize, _: Event) {
86: pub struct Surface;
88: pub fn new(_: Kind, _: &str, _: accesskit::Role) -> Result<Self> {
91: pub fn new_deferred_accessibility(
98: pub fn present(
107: pub fn shown(&self) -> &DisplayList {
110: pub fn id(&self) -> SurfaceId {
113: pub fn origin(&self) -> (i32, i32) {
116: pub fn animation_frames(&self, _: std::time::Duration) -> AnimationFrames {
119: pub fn set_alpha(&mut self, _: u8) -> Result<()> {
122: pub fn move_to(&mut self, _: (i32, i32)) -> Result<()> {
125: pub fn set_accessibility_semantics(&self, _: Vec<(crate::HitId, crate::Semantics)>) {}
126: pub fn take_accessibility_action(&self, _: crate::HitId) -> Option<crate::AccessibilityAction> {
129: pub fn set_glass(&self, _: Option<crate::Glass>) {
132: pub fn set_glass_style(&self, _: crate::glass::GlassStyle) {
136: pub fn with_render(kind: Kind, name: &str, role: accesskit::Role, _: crate::platform::Render) -> Result<Self> {
139: pub fn render(&self) -> crate::platform::Render {
142: pub fn glass_available(&self) -> bool {
145: pub fn set_glass_group(&self, glasses: &[crate::Glass], _spacing: f32) {
148: pub fn show(&self) {
151: pub fn hide(&self) {
154: pub fn release(&mut self) {
157: pub fn discard_events(&self) {
160: pub fn take_focus(&self) -> &'static str {
164: pub fn scale(&self) -> f32 {
167: pub fn place(&self, _: (i32, i32, i32, i32)) {
170: pub fn client_size(&self) -> (u32, u32) {
173: pub fn show_active(&self, _: bool) {
176: pub fn set_chrome(&self, _: &[(crate::Rect, super::app_window::Chrome)]) {
179: pub fn set_background(&self, _: crate::Rgba) {
```


### platform/stub/text.rs

```rust
1: pub use crate::text_style::{TextStyle, VARIABLE};
2: pub struct Text;
4: pub fn measure(&mut self, _: &str, _: &TextStyle) -> (f32, f32) {
7: pub fn clear_cache(&mut self) {
10: pub fn set_capacity(&mut self, _: usize) {
13: pub fn has_family(&self, _: &str) -> bool {
16: pub fn families(&self) -> Vec<String> {
19: pub fn fit(&mut self, _: &str, _: &TextStyle, _: f32) -> std::sync::Arc<str> {
22: pub fn lines(&mut self, _: &str, _: &TextStyle, _: f32, _: usize) -> Vec<std::sync::Arc<str>> {
```


### platform/stub/text_field.rs

```rust
3: pub struct TextField;
5: pub fn revision(&self) -> u64 {
```


### platform/stub/timing.rs

```rust
2: pub struct Ticker;
4: pub fn new() -> Result<Self> {
7: pub fn schedule(&mut self, _: bool) {
10: pub fn armed(&self) -> bool {
14: pub fn display_frame() -> std::time::Duration {
```


### platform/windows/app_window.rs

```rust
59: pub enum Chrome {
72: pub enum Cursor {
135: pub fn set_modal_hook(hook: Option<Box<dyn FnMut()>>) {
587: pub fn place(&self, bounds: (i32, i32, i32, i32)) {
603: pub fn show_active(&self, maximized: bool) {
619: pub fn client_size(&self) -> (u32, u32) {
638: pub fn scale(&self) -> f32 {
643: pub fn is_maximized(&self) -> bool {
648: pub fn minimize(&self) {
661: pub fn toggle_maximize(&self) {
666: pub fn set_chrome(&self, parts: &[(Rect, Chrome)]) {
674: pub fn set_background(&self, color: Rgba) {
679: pub fn set_cursor(&self, cursor: Cursor) {
684: pub fn set_frame(&self, dark: bool, border: Rgba) {
706: pub fn set_icon_png(&self, png: &[u8]) {
```


### platform/windows/compose.rs

```rust
32: pub struct Compositor {
42: pub struct Sprite {
58: pub fn new(hwnd: HWND) -> Result<Self> {
97: pub fn sprite(&self, image: &Image) -> Result<Sprite> {
143: pub fn show(&mut self, sprites: &[(&Sprite, Rect, f32)]) -> Result<()> {
```


### platform/windows/glass.rs

```rust
6: pub trait Backdrop {
43: pub fn install(backdrop: Rc<dyn Backdrop>) {
59: pub fn system() -> Rc<super::system_backdrop::SystemBackdrop> {
```


### platform/windows/gpu.rs

```rust
55: pub enum Power {
67: pub fn set_power(power: Power) {
72: pub fn power() -> Power {
79: pub struct Adapter {
81: pub name: String,
83: pub vendor: u32,
85: pub device: u32,
89: pub fn adapter(power: Power) -> Option<Adapter> {
123: pub struct Device {
125: pub adapter: Adapter,
138: pub fn device() -> Option<Rc<Device>> {
355: pub fn render(ui: &mut Ui, list: &DisplayList, width: u32, height: u32) -> Result<Option<Image>> {
```


### platform/windows/gpu_glass.rs

```rust
37: pub struct GlassDevice {
39: pub adapter_name: String,
56: pub fn device() -> Option<Rc<GlassDevice>> {
229: pub struct GlassLayer {
251: pub fn new(gpu: Rc<GlassDevice>, visual: IDCompositionVisual, size: (u32, u32), scale: f32) -> Result<Self> {
291: pub fn set(&mut self, glasses: &[Glass], style: &GlassStyle, spacing: f32) {
338: pub fn place(&mut self, origin: (i32, i32)) {
370: pub fn resize(&mut self, size: (u32, u32)) {
385: pub fn render(&mut self) -> bool {
```


### platform/windows/host.rs

```rust
22: pub fn screen_size() -> (i32, i32) {
33: pub fn shift_down() -> bool {
38: pub fn modifiers() -> Modifiers {
49: pub fn wake_at(deadline: Option<Instant>) {
55: pub fn wait() {
65: pub fn wait_for(timeout: std::time::Duration) {
83: pub fn reduced_motion() -> bool {
```


### platform/windows/icons.rs

```rust
6: pub fn system_family(text: &Text) -> Arc<str> {
15: pub fn centered(
```


### platform/windows/identity.rs

```rust
9: pub use crate::identity::{Identity, set_identity};
```


### platform/windows/layered_app.rs

```rust
68: pub struct LayeredApp {
78: pub fn new(surface: &Surface) -> Result<Self> {
97: pub fn set_enabled(&mut self, enabled: bool) {
129: pub fn present(
```


### platform/windows/mod.rs

```rust
2: pub mod a11y;
3: pub mod app_window;
4: pub mod compose;
6: pub mod glass;
8: pub mod gpu;
10: pub mod gpu_glass;
11: pub mod caret;
12: pub mod host;
13: pub mod identity;
14: pub mod layered_app;
15: pub mod modules;
16: pub mod render;
18: pub mod screen;
19: pub mod surface;
20: pub mod system_backdrop;
21: pub mod text;
22: pub mod text_field;
24: pub mod timing;
25: pub mod watch;
26: pub use render::{Image, Ui};
27: pub use surface::{Event, Kind, Surface, SurfaceId, next_event};
28: pub use text::Text;
29: pub use text_field::TextField;
31: pub mod icons;
32: pub use render::Canvas as Renderer;
34: pub struct Backend;
```


### platform/windows/modules.rs

```rust
9: pub fn loaded() -> Vec<String> {
41: pub fn is_gpu_driver(name: &str) -> bool {
53: pub fn gpu_drivers() -> Vec<String> {
```


### platform/windows/render.rs

```rust
296: pub struct Ui {
298: pub text: Text,
304: pub use crate::bitmap::{TextQuality, TextMode};
338: pub fn set_text_quality(&mut self, quality: TextQuality) -> Result<()> {
345: pub fn decode_image(&self, bytes: &[u8], max_size: (u32, u32)) -> Result<Image> {
399: pub fn new() -> Result<Self> {
404: pub fn isolated() -> Result<Self> {
430: pub use crate::bitmap::Image;
433: pub fn load(ui: &Ui, path: &Path) -> Result<Self> {
479: pub fn render(ui: &mut Ui, list: &DisplayList, width: u32, height: u32) -> Result<Image> {
505: pub struct Canvas {
```


### platform/windows/screen.rs

```rust
9: pub struct Screen {
11: pub work: Edges,
13: pub dpi: u32,
17: pub fn screen_at(x: i32, y: i32) -> Screen {
```


### platform/windows/surface.rs

```rust
52: pub enum Kind {
70: pub struct SurfaceId(isize);
74: pub enum Button {
85: pub struct Pointer {
87: pub at: Point,
89: pub screen: (i32, i32),
94: pub enum Event {
140: pub struct AnimationFrames(std::sync::Arc<std::sync::atomic::AtomicBool>);
154: pub fn next_event() -> Option<(SurfaceId, Event)> {
160: pub fn queue_event(hwnd: HWND, event: Event) {
266: pub struct Surface {
285: pub fn animation_frames(&self, duration: std::time::Duration) -> AnimationFrames {
327: pub fn new(kind: Kind, name: &str, role: Role) -> Result<Self> {
332: pub fn with_render(kind: Kind, name: &str, role: Role, render: crate::platform::Render) -> Result<Self> {
338: pub fn new_deferred_accessibility(kind: Kind, name: &str, role: Role) -> Result<Self> {
492: pub fn render(&self) -> crate::platform::Render {
503: pub fn glass_available(&self) -> bool {
522: pub fn id(&self) -> SurfaceId {
527: pub fn hwnd(&self) -> HWND {
533: pub fn set_text_field(&self, field: Option<crate::TextField>) {
545: pub fn set_glass(&self, glass: Option<crate::glass::Glass>) {
573: pub fn set_glass_style(&self, style: crate::glass::GlassStyle) {
590: pub fn has_glass(&self) -> bool {
598: pub fn set_glass_group(&self, glasses: &[crate::glass::Glass], spacing: f32) {
697: pub fn origin(&self) -> (i32, i32) {
702: pub fn shown(&self) -> &DisplayList {
708: pub fn present(
719: pub fn present_sparse(
840: pub fn move_to(&mut self, origin: (i32, i32)) -> Result<()> {
877: pub fn set_alpha(&mut self, alpha: u8) -> Result<()> {
897: pub fn show(&self) {
917: pub fn keep_above_desktop(&self) -> Result<bool> {
970: pub fn desktop_order_valid(&self) -> bool {
992: pub fn raise_in_desktop_band(&self) -> Result<()> {
1030: pub fn hide(&self) {
1037: pub fn is_visible(&self) -> bool {
1043: pub fn set_click_through(&self, through: bool) {
1058: pub fn release(&mut self) {
1077: pub fn discard_events(&self) {
1087: pub fn set_click_activates(&self, rects: Vec<crate::geom::Rect>) {
1093: pub fn set_no_activate(&self, disabled: bool) {
1117: pub fn take_focus(&self) -> &'static str {
1246: pub fn set_accessibility_semantics(&self, semantics: Vec<(HitId, crate::Semantics)>) {
1251: pub fn take_accessibility_action(&self, hit: HitId) -> Option<crate::AccessibilityAction> {
```


### platform/windows/system_backdrop.rs

```rust
57: pub struct SystemBackdrop {
90: pub fn active(&self) -> bool {
95: pub fn window(&self, client: HWND) -> Option<HWND> {
101: pub fn follow(&self, client: HWND) {
```


### platform/windows/text.rs

```rust
23: pub use crate::text_style::{TextStyle, VARIABLE};
54: pub struct Text {
70: pub fn clear_cache(&mut self) {
104: pub fn set_capacity(&mut self, layouts: usize) {
109: pub fn measure(&mut self, text: &str, style: &TextStyle) -> (f32, f32) {
116: pub fn fit(&mut self, text: &str, style: &TextStyle, width: f32) -> Arc<str> {
146: pub fn lines(
203: pub fn has_family(&self, family: &str) -> bool {
221: pub fn families(&self) -> Vec<String> {
```


### platform/windows/text_field.rs

```rust
32: pub struct HostVTable {
91: pub struct TextField {
113: pub fn new(window: HWND, rect: Rect, multiline: bool, size: f32, color: Rgba) -> Result<Self> {
192: pub fn caret(&self) -> Option<Rect> {
199: pub fn revision(&self) -> u64 {
205: pub fn set_style(&self, style: &crate::TextStyle, color: Rgba) {
224: pub fn set_rect(&self, rect: Rect) {
237: pub fn set_text(&self, text: &str) -> Result<()> {
245: pub fn text(&self) -> Result<String> {
254: pub fn select_all(&self) {
258: pub fn select_range(&self, range: std::ops::Range<u32>) {
263: pub fn request_redraw(&self) {
269: pub fn prepare(&self) -> Result<()> {
276: pub fn focus(&self) {
291: pub fn blur(&self) {
330: pub fn multiline(&self) -> bool {
```


### platform/windows/timing.rs

```rust
13: pub struct Ticker {
22: pub fn new() -> Result<Self> {
41: pub fn handle(&self) -> HANDLE {
47: pub fn schedule(&mut self, moving: bool) {
66: pub fn armed(&self) -> bool {
79: pub fn display_frame() -> Duration {
```


### platform/windows/watch.rs

```rust
24: pub struct FileWatch {
29: pub fn new(path: &Path, changed: impl Fn() + Send + 'static) -> Result<Self> {
```


### reveal.rs

```rust
7: pub struct Transform {
9: pub from: Rect,
11: pub to: Rect,
15: pub fn scale(self) -> f32 {
19: pub fn rect(self, rect: Rect) -> Rect {
29: pub fn inverse(self, point: Point) -> Point {
41: pub enum Reveal {
58: pub fn scales(self) -> bool {
63: pub fn body(self, full: Rect, anchor: Rect, progress: f32) -> Rect {
95: pub struct Snapshot {
97: pub original: DisplayList,
104: pub fn new(ui: &mut Ui, list: DisplayList, size: (u32, u32)) -> anyhow::Result<Self> {
114: pub fn frame(&self, full: Rect, body: Rect) -> DisplayList {
```


### scene.rs

```rust
15: pub type HitId = u32;
19: pub enum Op {
202: pub fn bounds(&self) -> Rect {
260: pub struct Hit {
262: pub id: HitId,
264: pub rect: Rect,
266: pub role: Role,
268: pub label: Arc<str>,
270: pub enabled: bool,
274: pub actionable: bool,
276: pub checked: Option<bool>,
282: pub fn new(
304: pub struct DisplayList {
306: pub ops: Vec<Op>,
308: pub hits: Vec<Hit>,
310: pub focus: Option<HitId>,
315: pub fn translated(&self, dx: f32, dy: f32) -> Self {
358: pub fn hit_at(&self, p: Point) -> Option<&Hit> {
364: pub fn damage(&self, old: &DisplayList) -> Option<Rect> {
370: pub fn damage_regions(&self, old: &DisplayList) -> Vec<Rect> {
```


### scroll.rs

```rust
24: pub const NOTCH: f32 = 48.0;
31: pub struct Scroll {
58: pub fn new(motion: &MotionTokens) -> Self {
71: pub fn max(&self) -> f32 {
77: pub fn set_extent(&self, content: f32, viewport: f32) {
82: pub fn offset(&self, now: Instant) -> f32 {
95: pub fn wheel(&mut self, delta: i16, s: f32, now: Instant) -> bool {
102: pub fn scroll_by(&mut self, by: f32, glide: bool, now: Instant) -> bool {
116: pub fn page(&mut self, down: bool, now: Instant) -> bool {
124: pub fn reveal(&mut self, top: f32, bottom: f32, now: Instant) -> bool {
137: pub fn reset(&mut self) {
144: pub fn bar(&self, now: Instant) -> f32 {
162: pub fn tick(&mut self, now: Instant) -> (bool, Option<Instant>) {
```


### shadow.rs

```rust
62: pub src: u32,
63: pub src_len: u32,
64: pub dst: f32,
65: pub dst_len: f32,
72: pub key: Key,
73: pub x: Vec<Slice>,
74: pub y: Vec<Slice>,
79: pub fn pieces(&self) -> impl Iterator<Item = (Rect, Rect)> + '_ {
94: pub width: u32,
95: pub height: u32,
96: pub alpha: Vec<u8>,
120: pub fn size(self) -> (u32, u32) {
125: pub fn bytes(self) -> usize {
272: pub const BUDGET: usize = 1 << 20;
274: pub fn new(budget: usize) -> Self {
279: pub fn get(&mut self, key: Key, make: impl FnOnce(Key) -> Option<T>) -> Option<&T> {
297: pub fn len(&self) -> usize {
302: pub fn bytes(&self) -> usize {
```


### sortable.rs

```rust
8: pub struct Row<'a, A> {
10: pub key: Key,
12: pub rect: Rect,
14: pub label: &'a str,
16: pub detail: &'a str,
18: pub separator: bool,
20: pub icon: Option<Glyph>,
22: pub select: A,
24: pub drag: A,
26: pub remove: A,
30: pub fn row<A: Clone>(b: &mut Builder<'_, A>, row: Row<'_, A>) -> Rect {
154: pub fn reorder<T>(items: &mut Vec<T>, from: usize, to: usize) -> bool {
```


### text_style.rs

```rust
9: pub struct TextStyle {
11: pub family: Arc<str>,
13: pub size: f32,
15: pub weight: u16,
17: pub tracking: f32,
19: pub optical: f32,
23: pub const VARIABLE: &str = "Segoe UI Variable";
27: pub fn new(family: &str, size: f32) -> Self {
38: pub fn scaled(&self, scale: f32) -> Self {
```


### theme/data.rs

```rust
429: pub fn from_toml(source: &str) -> anyhow::Result<Self> {
433: pub fn from_json(source: &str) -> anyhow::Result<Self> {
```


### theme/parts.rs

```rust
5: pub enum Material {
31: pub type MaterialOverrides = Knobs;
35: pub fn liquid() -> Self {
47: pub fn one_ui() -> Self {
59: pub fn fluent() -> Self {
71: pub fn solid() -> Self {
75: pub fn mono() -> Self {
79: pub fn tint(mut self, value: Rgba) -> Self {
92: pub fn clarity(self, value: f32) -> Self {
99: pub fn blur(self, value: f32) -> Self {
106: pub fn refraction(self, value: f32) -> Self {
113: pub fn highlight(self, value: f32) -> Self {
120: pub fn overrides(self, knobs: &MaterialOverrides) -> Self {
193: pub struct Shape {
195: pub surface: f32,
197: pub card_min: f32,
199: pub control: Option<f32>,
201: pub small: f32,
205: pub fn liquid() -> Self {
214: pub fn one_ui() -> Self {
223: pub fn solid() -> Self {
232: pub fn fluent() -> Self {
241: pub fn mono() -> Self {
252: pub enum Radius {
263: pub type Spacing = Metrics;
265: pub type Motion = MotionTokens;
268: pub struct Icons {
270: pub set: IconSet,
272: pub weight: f32,
276: pub fn liquid() -> Self {
283: pub fn one_ui() -> Self {
290: pub fn solid() -> Self {
294: pub fn fluent() -> Self {
301: pub fn mono() -> Self {
307: pub fn liquid() -> Self {
325: pub fn one_ui() -> Self {
333: pub fn solid() -> Self {
340: pub fn fluent() -> Self {
349: pub fn mono() -> Self {
358: pub fn liquid() -> Self {
370: pub fn one_ui() -> Self {
384: pub fn fluent() -> Self {
398: pub fn solid() -> Self {
410: pub fn mono() -> Self {
423: pub fn liquid() -> Self {
434: pub fn one_ui() -> Self {
438: pub fn solid() -> Self {
442: pub fn fluent() -> Self {
446: pub fn mono() -> Self {
502: pub struct Problem {
504: pub role: &'static str,
506: pub ratio: f32,
508: pub required: f32,
558: pub fn material(mut self, material: Material) -> Self {
564: pub fn shape(mut self, shape: Shape) -> Self {
573: pub fn radius(mut self, radius: Radius, value: f32) -> Self {
584: pub fn type_ramp(mut self, ramp: TypeRamp) -> Self {
590: pub fn spacing(mut self, spacing: Spacing) -> Self {
596: pub fn shadows(mut self, shadows: Shadows) -> Self {
602: pub fn motion(mut self, motion: Motion) -> Self {
608: pub fn icons(mut self, icons: Icons) -> Self {
615: pub fn palettes(mut self, light: Palette, dark: Palette) -> Self {
621: pub fn palette(mut self, palette: Palette) -> Self {
627: pub fn focus_ring(mut self, ring: FocusRing) -> Self {
633: pub fn border(mut self, border: Option<Rgba>) -> Self {
707: pub fn check(&self) -> Vec<Problem> {
743: pub struct FocusRing {
745: pub width: f32,
747: pub gap: f32,
749: pub opacity: f32,
```


### theme.rs

```rust
25: pub use parts::*;
37: pub enum Style {
53: pub const ALL: [Self; 5] = [
62: pub const fn name(self) -> &'static str {
75: pub enum Scheme {
84: pub struct Palette {
86: pub monochrome: bool,
88: pub label: Rgba,
90: pub secondary: Rgba,
92: pub tertiary: Rgba,
94: pub quaternary: Rgba,
96: pub fill: Rgba,
98: pub card: Rgba,
100: pub well: Rgba,
102: pub separator: Rgba,
104: pub accent: Rgba,
106: pub on_accent: Rgba,
108: pub accent_wash: Rgba,
110: pub destructive: Rgba,
112: pub success: Rgba,
114: pub warning: Rgba,
116: pub surface: Rgba,
118: pub elevated: Rgba,
120: pub hover: Rgba,
122: pub pressed: Rgba,
124: pub knob: Rgba,
126: pub scrollbar: Rgba,
128: pub shadow: Rgba,
133: pub enum TextRole {
158: pub const ALL: [Self; 10] = [
174: pub struct Font {
176: pub size: f32,
178: pub weight: u16,
180: pub tracking: f32,
182: pub line: f32,
187: pub struct TypeRamp {
189: pub family: Arc<str>,
191: pub roles: [Font; 10],
196: pub fn font(&self, role: TextRole) -> Font {
201: pub fn style(&self, role: TextRole, s: f32) -> TextStyle {
213: pub fn line(&self, role: TextRole, s: f32) -> f32 {
220: pub struct Metrics {
222: pub pad: f32,
224: pub gap: f32,
226: pub item: f32,
228: pub row: f32,
230: pub row_single: f32,
232: pub control: f32,
234: pub control_small: f32,
236: pub icon: f32,
238: pub icon_small: f32,
240: pub icon_large: f32,
242: pub hit: f32,
244: pub scrollbar: f32,
246: pub scrollbar_inset: f32,
251: pub struct Radii {
253: pub surface: f32,
255: pub card: f32,
257: pub control: Option<f32>,
259: pub small: f32,
266: pub fn concentric(outer: f32, inset: f32, min: f32) -> f32 {
275: pub struct Elevation {
277: pub spread: f32,
279: pub drop: f32,
281: pub color: Rgba,
287: pub enum ShadowLevel {
307: pub struct SoftShadow {
309: pub blur: f32,
311: pub offset: (f32, f32),
313: pub spread: f32,
315: pub color: Rgba,
322: pub struct ShadowLayer {
324: pub blur: f32,
326: pub y: f32,
328: pub spread: f32,
330: pub strength: f32,
335: pub const NONE: Self = Self { blur: 0.0, y: 0.0, spread: 0.0, strength: 0.0 };
338: pub const fn new(blur: f32, y: f32, spread: f32, strength: f32) -> Self {
347: pub struct Shadows {
349: pub card: [ShadowLayer; 2],
351: pub raised: [ShadowLayer; 2],
353: pub menu: [ShadowLayer; 2],
355: pub popover: [ShadowLayer; 2],
357: pub dialog: [ShadowLayer; 2],
359: pub toast: [ShadowLayer; 2],
364: pub fn level(&self, level: ShadowLevel) -> &[ShadowLayer; 2] {
378: pub struct MotionTokens {
380: pub interactive: Spring,
382: pub open: Spring,
384: pub layout: Spring,
386: pub scroll: Spring,
388: pub scrollbar_idle: f32,
390: pub scrollbar_fade: f32,
395: pub struct Theme {
397: pub material: Material,
399: pub focus_ring: FocusRing,
401: pub monochrome: bool,
406: pub style: Style,
408: pub scheme: Scheme,
410: pub color: Palette,
412: pub font: TypeRamp,
414: pub metrics: Metrics,
416: pub radius: Radii,
418: pub elevation: Elevation,
420: pub shadows: Shadows,
422: pub motion: MotionTokens,
424: pub icons: IconSet,
426: pub icon_weight: f32,
428: pub glass: Option<GlassMaterial>,
431: pub border: Option<Rgba>,
436: pub const SYSTEM_BLUE: Rgba = Rgba::new(0.0, 136.0 / 255.0, 1.0, 1.0);
440: pub fn with_surface_radius(mut self, radius: f32) -> Self {
449: pub fn new(style: Style, scheme: Scheme, accent: Rgba) -> Self {
460: pub fn dark(&self) -> bool {
466: pub fn shadow(&self, level: ShadowLevel) -> [SoftShadow; 2] {
481: pub fn adaptive(&self, min_side: f32, backdrop: Option<f32>) -> Scheme {
497: pub fn with_scheme(&self, scheme: Scheme) -> Self {
511: pub fn tokens(&self) -> crate::Tokens {
534: pub fn ink(&self) -> crate::Ink {
554: pub fn material(style: Style, tint: Rgba) -> Option<GlassMaterial> {
571: pub const fn lensed(style: Style) -> bool {
578: pub struct Knobs {
580: pub clarity: Option<f32>,
582: pub blur: Option<f32>,
584: pub refraction: Option<f32>,
586: pub highlight: Option<f32>,
591: pub fn over(self, base: Knobs) -> Knobs {
610: pub fn tuned(self, style: Style, knobs: &Knobs) -> Self {
631: pub fn set_active(style: Style) {
637: pub fn active() -> Style {
651: pub fn luminance(c: Rgba) -> f32 {
663: pub fn contrast(a: Rgba, b: Rgba) -> f32 {
669: pub fn over(top: Rgba, under: Rgba) -> Rgba {
```


### tokens.rs

```rust
13: pub struct Tokens {
15: pub font: TextStyle,
17: pub icons: Arc<str>,
19: pub icon_set: Option<crate::icons::IconSet>,
21: pub colors: Colors,
23: pub material: Material,
25: pub radius: f32,
27: pub border: bool,
29: pub shadow: bool,
31: pub scale: f32,
33: pub menu: MenuMetrics,
36: pub glass: Option<crate::glass::GlassMaterial>,
38: pub glass_style: crate::glass::GlassStyle,
65: pub struct Colors {
67: pub text: Rgba,
69: pub text_dim: Rgba,
71: pub hover: Rgba,
73: pub line: Rgba,
75: pub shadow: Rgba,
81: pub fn neutral(dark: bool) -> Self {
103: pub fn new(dark: bool, accent: [u8; 3]) -> Self {
117: pub struct Ink {
119: pub primary: Rgba,
121: pub secondary: Rgba,
123: pub tertiary: Rgba,
125: pub disabled: Rgba,
130: pub fn of(ink: Rgba) -> Self {
144: pub enum Step {
163: pub const fn metrics(self) -> (f32, u16) {
179: pub struct Typeface {
181: pub text: Arc<str>,
183: pub display: Arc<str>,
188: pub fn system(text: &crate::Text) -> Self {
203: pub fn single(family: &str) -> Self {
211: pub fn style(&self, step: Step, scale: f32) -> TextStyle {
217: pub fn sized(&self, size: f32, weight: u16, scale: f32) -> TextStyle {
232: pub struct Material {
234: pub tint: Rgba,
239: pub struct MenuMetrics {
241: pub row: f32,
243: pub separator: f32,
245: pub pad: f32,
247: pub gutter: f32,
249: pub arrow: f32,
251: pub gap: f32,
253: pub min_width: f32,
255: pub shadow: f32,
257: pub glyph: f32,
278: pub fn scaled(&self, scale: f32) -> Self {
```


### transition.rs

```rust
9: pub enum Easing {
26: pub struct Timing {
28: pub enabled: bool,
30: pub duration: Duration,
32: pub easing: Easing,
37: pub fn new(enabled: bool, milliseconds: f32, easing: Easing) -> Self {
50: pub fn start(self, from: f32, to: f32, now: Instant) -> Transition {
62: pub struct Transition {
66: pub timing: Timing,
72: pub fn done(self, now: Instant) -> bool {
77: pub fn value(self, now: Instant) -> f32 {
96: pub fn bezier_at([x1, y1, x2, y2]: [f32; 4], x: f32) -> f32 {
```
