# Credits

`stillkit-glass` is **liquid-rust** by **al1h3n**, adopted into stillkit:

- Upstream: <https://github.com/al1h3n/liquid-rust>, commit `2073cd96c48458093ae3ea23e14e1162e84d5e80`
  (crate version 0.2.5).
- Licence: MIT OR Apache-2.0, at your option. Both licence texts are kept unchanged in
  `LICENSE-MIT` and `LICENSE-APACHE`, and every source file keeps its attribution header. This crate
  stays under the same dual licence; the rest of stillkit is MIT.

The model (Snell's-law refraction through a squircle bevel, per-channel dispersion, frost pyramid,
directional rim light, smooth signed-distance union for liquid merging, Apple's duration/bounce
springs) is upstream's work. Thank you, al1h3n.

## Changes since upstream

The copy first lived in Glaze (`crates/liquid-rust`), which changed it for its engine: shared
pipelines (`Renderer::fork`), per-glass appearance, partial pyramid rebuilds, cheaper shader math,
lenses scaled for thin glass, an inner shadow, optional GPU timestamps. Those changes are marked
`Glaze:` in the source and listed in `UPSTREAM-CHANGES.md`. Changes made in stillkit are marked
`stillkit:`.
