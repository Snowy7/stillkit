# Motion

The view states the destination. animate() uses the theme layout spring; motion(token::MOTION_INTERACTIVE) chooses another spring. Stable keys retain velocity on retarget. transition supplies an entry/exit pose: removed items stay painted until exit settles and stop receiving input.

First frames are static. Drags, resize and tx.instant() land at once. The host follows OS reduced motion; --reduced forces it. The ticker stops when motion settles. No permanent timer is needed to drive springs.

```rust
use stillkit::el::{self, App, El, Transition, col, el, text, token};
use stillkit::{Kind, Scheme, Theme};
type State = bool;
fn view(s: &State) -> El<State> {
    col()
        .animate()
        .pad(token::PAD)
        .gap(token::GAP)
        .child(
            text("Resize")
                .key("resize")
                .on_press(|s: &mut State| *s = !*s),
        )
        .child(
            el().key("card")
                .w(if *s { 280. } else { 140. })
                .h(80.)
                .fill(token::CARD)
                .radius(token::RADIUS_CARD)
                .animate(),
        )
        .children(s.then(|| {
            text("Details")
                .key("details")
                .transition(Transition::slide(0., 16.))
        }))
}
fn main() -> anyhow::Result<()> {
    el::run(App {
        title: "Guide",
        size: (560., 400.),
        kind: Kind::App { min: (320, 240) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: false,
        view,
    })
}
```

[Executable source](../../crates/stillkit/examples/guide_motion.rs).


[All guides](../../README.md#guides) ? [Element reference](../el.md)
