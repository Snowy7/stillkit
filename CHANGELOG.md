# Changelog

## [Unreleased]

### Added
- Component story browser (including ColorPicker), headless story baselines and a runnable ten-minute settings guide.
- Opt-in F12 element inspector and native look-file reloads, with no inspection code in ordinary builds.
- Native linear and radial element gradients for fills and borders, with sprung stops and direction (`el_effects`).
- `stillkit_kit::ColorPicker`: a controlled saturation/brightness square with hue and optional alpha strips; `on_change` previews every drag move and commits on release (`Hsva` keeps the hue through greys).
- el styling for dense tools: `border`, `edges` (a design px of light and shade inside a box's top and bottom, following its corners), `hovered`/`pressed` state looks (sprung, no view run), per-element `family`/`weight`/`text_size`/`text_align`, and `cursor` with six new pointer shapes.
- `stillkit::waker()`: a `Send + Sync` `Waker` that ends the UI thread's idle wait from any thread (worker results, sockets), coalesced, with no timer.
- `Icon::Path(&IconPath)`: apps draw their own SVG-path icons (any Lucide glyph, a brand mark) through every renderer, beside the built-in set.
- `stillkit::dialog`: native folder/file pickers, save panel and error alert (macOS, Windows; Linux has no portal client yet).
- `el::SystemClipboard`: the system clipboard on every platform (Windows now has one outside fields).
- `Runtime::touch`: host bookkeeping on the state (draining outgoing requests) without running the view.
- `El::cascade_ink`: opt-in ink inheritance, so labels and icons follow their control's colour and hover look.
- el file drops: `on_drop_files` and a `dropping` state look on the innermost target.
- File drops: `Event::FilesHover`/`FilesLeave`/`FilesDropped` and `take_dropped_files()` (macOS with hover feedback; Windows on drop).
- `El::flow_wrap` (rows of tiles that wrap with the width) and `El::follow_end` (a scroll view or virtual list that stays at its end while content grows, until scrolled back).
- `el::virtual_list`: lists of any length that build only the rows near sight, rebuilding in the same frame when scrolling passes them.
- Native layers: `el::native(name)` holes and `Surface::native_layer`/`place_native_layer`/`release_native_layer`, so a host's Metal or Vulkan view (a 3D viewport) lives under el content with overlays on top (macOS; Windows GPU surfaces; not yet Wayland).
- el raw pointer and wheel: `on_pointer` (hover, held press with capture, merged moves, deltas, modifiers, click count) and `on_wheel`; `Runtime::set_modifiers`.
- el styling for dense tools: `border`, `hovered`/`pressed` state looks (sprung, no view run), per-element `family`/`weight`/`text_size`/`text_align`, and `cursor` with six new pointer shapes.
- `Surface::set_caption(Caption::Compact)` (a 40-point title area with the macOS traffic lights centred) and `Surface::system_controls()` (their rect, so an app's own title bar leaves them room).
- `Event::KeyUp`: a key going up on macOS, Windows and Linux, for held-key input such as fly navigation.
- `El::focus_ring(false)`: an element that leaves showing focus to its container (`focus_within`), so a field inside a search box is not ringed twice.
- Real soft shadows (`Op::SoftShadow`) with elevation levels per look, and Windows system backdrop frost for CPU surfaces.
- Kit text components (text, secure and search fields, text area, combobox, command palette) and Linux Vulkan/Vello Hybrid GPU rendering and glass.
- Portable process `Identity` and Wayland app_id, preserving Windows window class names.
- Linux AT-SPI via AccessKit Unix, on by default but only while assistive technology is enabled (one idle socket otherwise); `STILLKIT_ACCESSIBILITY=1/0` overrides. Compile-checked; compositor verification pending.
- el round 3: eased hover/press washes and focus ring, focus/blur by key, `focus_within`, `invalid` and `active_descendant`, cell hits that pass presses through, `rotate`, `spin` (only while visible), arcs from any angle.
- Newcomer guides, executable examples and a settings showcase.
- 0.1 metadata groundwork and Windows/Linux CI with an advisory macOS cross-check.

### Changed
- Display-list damage trims the unchanged start and end before comparing, so an inserted or removed op damages only itself; macOS CPU surfaces repaint each disjoint damaged region alone.
- Interaction motion has one implementation; `interact::motion` remains a compatibility re-export.

### Preview, planned for 0.2
- macOS AppKit/CoreGraphics/CoreText backend, native input and IME, AccessKit, system frost and optional Metal. It is compile-checked only and has not run on a Mac; it is not part of 0.1. The test checklist is in `docs/ui.md` ("macOS backend").

## [0.1.0] - unreleased draft

A draft inventory, not a published release. The target is a real small Windows/Wayland app built from these docs with el + kit, CPU or GPU.

### Added
- Native surfaces extracted from Glaze with history and MIT licensing.
- Keyed declarative elements, flex/grid layout, damage-only redraw and snap/tree/script tooling.
- Spring layout/colour/presence motion, reduced motion and idle ticker shutdown.
- Independent theme parts, five light/dark presets, mix diagnostics and TOML/JSON loading.
- Windows software Direct2D and optional GPU DirectComposition; per-surface Cpu/Gpu/Auto.
- Wayland CPU vello_cpu and optional Vulkan/Vello Hybrid GPU rendering and glass.
- stillkit-glass from al1h3n's liquid-rust (MIT OR Apache-2.0), merging and morphs.
- stillkit-kit controls, navigation/data components, menus, selects, dialogs, tooltips and toasts.
- Element text input/area, editing, selection, clipboard and IME; Windows RichEdit and UI Automation text runs.

### Known limits
- No Linux X11; GNOME bars need a Shell extension. Linux editing, IME and AT-SPI need live verification.
- macOS is a preview for 0.2 (see above).
- `el::run` does not attach glass bodies from theme materials; use Surface hooks.
- `rotate` turns arcs, lines and icons exactly; text, images and boxes at other than quarter turns stay upright.

No packages have been published.
