# Component playground

`stillkit-playground` is a separate, optional crate. Applications that do not depend
on it pay no startup or idle cost. It builds on [agent tooling](agent-tooling.md):
the browser is an ordinary `el::run` app and accepts the same snap/tree/script flags.

```sh
cargo run -p stillkit-playground --example stories
cargo run -p stillkit-playground --example stories -- --snap browser.png
cargo run -p stillkit-playground --example stories -- --goldens story-images
cargo run -p stillkit-playground --example stories -- --check-goldens story-images
```

The browser lists every supported kit component (including all five button kinds).
ChartSeries, TreeNode, MenuItem and Command are demonstrated in their parent story;
the deprecated TextFieldFrame compatibility shim also has a named story.
Select a look, light/dark, material and shape independently. Scale changes raster
density in the same window. Reduced motion lands changes immediately. Model state
survives story and environment changes. Disabled and invalid are model knobs;
rest, hover, pressed and focus target the selected story's `subject` key.
Pointer input resumes normal interaction after a forced state preview.

`Story<S>` stores a name and a plain view function over `Session<S>`. Use `before_snapshot` for deterministic setup such as a tooltip hover. Put custom
knobs in `Session::model`; a story can include controls for that model beside its
subject. Names must have unique filename slugs.

Headless rendering uses fresh model state, a fixed zero clock and reduced motion.
It writes one 640x480 PNG per story per look/scheme (510 images for the current
catalog). It does not overwrite the existing repository goldens. Review generated
images before committing them as a baseline. The focused catalog test runs this
same render path and decodes every output; use `--check-goldens DIR` in CI to compare against a committed visual baseline.
The comparison fails on missing or changed PNGs and never rewrites them. Fonts and rasterizers differ across platforms,
so keep baselines per platform.

## Inspector

Compile any el app with `stillkit/inspect`, then press F12. The feature is opt-in
in both debug and release builds. Normal builds compile out all inspection code;
in tools builds the off branch takes no snapshots, clocks or allocations.
F12 closes the overlay and releases its temporary snapshots immediately.

The overlay outlines the deepest element under the pointer, reports its key/path,
computed device bounds converted to design pixels, resolved padding, flex grow,
fill and opacity. Its scrollable tree lists all nodes; click one to pin inspection.
`V` marks nodes rebuilt by a view run and `L` marks elements whose retained Taffy layout was invalidated in the last frame. The header reports view reruns, retained-layout touched count
and runtime frame time (excluding inspector drawing). stillkit rebuilds one view
function, so reruns apply to the entire view; touched count is Taffy input changes,
not a claim that every touched element changed bounds. Use `--tree` for the full
machine-readable tree. Inspection does not install timers and does not alter app
hit testing. The panel scrolls and selects nodes; it consumes pointer input over itself.
