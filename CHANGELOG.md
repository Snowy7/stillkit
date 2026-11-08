# Changelog

## [Unreleased]

### Added
- Real soft shadows (`Op::SoftShadow`) with elevation levels per look, and Windows system backdrop frost for CPU surfaces.
- Kit text components (text, secure and search fields, text area, combobox, command palette) and Linux Vulkan/Vello Hybrid GPU rendering and glass.
- Portable process `Identity` and Wayland app_id, preserving Windows window class names.
- Linux AT-SPI via AccessKit Unix, on by default but only while assistive technology is enabled (one idle socket otherwise); `STILLKIT_ACCESSIBILITY=1/0` overrides. Compile-checked; compositor verification pending.
- el round 3: eased hover/press washes and focus ring, focus/blur by key, `focus_within`, `invalid` and `active_descendant`, cell hits that pass presses through, `rotate`, `spin` (only while visible), arcs from any angle.
- Newcomer guides, executable examples and a settings showcase.
- 0.1 metadata groundwork and Windows/Linux CI with an advisory macOS cross-check.

### Changed
- Interaction motion has one implementation; `interact::motion` remains a compatibility re-export.

### Preview, planned for 0.2
- macOS AppKit/CoreGraphics/CoreText backend, native input and IME, AccessKit, system frost and optional Metal. It is compile-checked only and has not run on a Mac; it is not part of 0.1. The test checklist is in `docs/ui.md` ("macOS backend").

## [0.1.0] - unreleased draft

A draft inventory, not a published release. The target is a real small Windows/Wayland app built from these docs with el + kit, CPU or GPU.

### Added
- Native surfaces extracted from mypaper with history and MIT licensing.
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
