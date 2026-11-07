# macOS goldens

CoreGraphics and CoreText draw text differently from DirectWrite and Vello, so macOS keeps its
own reference images here (`el_motion` strips, the gallery's `kit-mix-*`). None are blessed yet:
on a Mac, run `STILLKIT_BLESS=1 cargo test -p stillkit --features golden`, look at every PNG
written here, then commit them (docs/ui.md, "macOS backend").
