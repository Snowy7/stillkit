# Changelog

## [Unreleased]

### Added
- macOS AppKit/CoreGraphics/CoreText backend, native input, AccessKit and optional Metal (compile-checked; native verification pending).
- Real soft shadows and Windows system backdrop frost for CPU surfaces.
- Kit text components and Linux Vulkan/Vello Hybrid GPU rendering and glass.
- Portable process Identity and Wayland app_id, preserving Windows window class names.
- Opt-in Linux AT-SPI via AccessKit Unix, lazy trees and event-driven action delivery (compile-checked; compositor verification pending).

### Changed
- Interaction motion has one implementation; interact::motion remains a compatibility re-export.

### Added
- Newcomer guides, executable examples and a settings showcase.
- 0.1 metadata groundwork and Windows/Linux CI with advisory macOS cross-check.

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
- No Linux X11; AT-SPI requires STILLKIT_ACCESSIBILITY=1 and live verification; GNOME bars need an extension. Native Linux editing needs broader live verification.
- macOS backend is compile-checked; native behavior is unverified.
- el::run does not attach glass bodies from theme materials; use Surface hooks.
- Kit hover/press washes snap; tree parent/child focus and continuous spinner visibility need further runtime support.

History checked through 38c21c4 (text follow-up), ebecc37 (element editing), ef42434 (kit layout/data), 0944ca2 (kit overlays), plus earlier render-mode/glass history. No packages were published by this work.
