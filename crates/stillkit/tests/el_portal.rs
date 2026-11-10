//! Portals: a subtree laid out beyond the window, shown in another native window by
//! presenting just its region, with that window's input sent at the region's offset.

use std::time::Duration;

use stillkit::el::{El, Runtime, col, el};
use stillkit::theme::{Scheme, Style, SYSTEM_BLUE};
use stillkit::{Rect, Rgba, Theme};

const FRAME: Duration = Duration::from_millis(16);

/// Whether the portal's square was pressed.
struct Portal {
    pressed: bool,
}

fn view(s: &Portal) -> El<Portal> {
    col().w_full().h_full().child(
        el().key("portal")
            .absolute(1000.0, 0.0)
            .w(120.0)
            .h(80.0)
            .fill(if s.pressed {
                Rgba::new(0.0, 0.0, 1.0, 1.0)
            } else {
                Rgba::new(1.0, 0.0, 0.0, 1.0)
            })
            .on_press(|s: &mut Portal| s.pressed = true),
    )
}

#[test]
fn a_region_beyond_the_window_shows_and_takes_input_like_a_window_of_its_own() {
    for scale in [1.0, 2.0] {
        let size = ((200.0 * scale) as u32, (100.0 * scale) as u32);
        let mut rt = Runtime::new(Portal { pressed: false }, view, Theme::new(Style::Liquid, Scheme::Dark, SYSTEM_BLUE), size, scale).unwrap();
        rt.frame(FRAME);
        let region = Rect::new(1000.0, 0.0, 120.0, 80.0);
        let image = rt.snap_region(region).unwrap();
        assert_eq!((image.width, image.height), ((120.0 * scale) as u32, (80.0 * scale) as u32));
        let [b, _, r, _] = image.pixel(image.width / 2, image.height / 2);
        assert!(r > 200 && b < 50, "the portal's red square at {scale}x");
        // That window's pointer (device pixels, as surfaces report it), offset by the
        // region's origin, reaches it.
        let (x, y) = ((1000.0 + 60.0) * scale, 40.0 * scale);
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(FRAME);
        assert!(rt.state().pressed, "pressed through the portal at {scale}x");
        let [b, _, r, _] = rt.snap_region(region).unwrap().pixel(image.width / 2, image.height / 2);
        assert!(b > 200 && r < 50, "and repainted");
    }
}

/// A menu open from a button at the portal's right edge.
fn menu_view(_: &Portal) -> El<Portal> {
    use stillkit::el::{menu, row};
    col().w_full().h_full().child(
        el().absolute(1000.0, 0.0).w(200.0).h(160.0).child(
            row().w_full().justify(stillkit::el::Justify::End).child(
                el().key("open").w(30.0).h(24.0).overlay(menu(
                    col().key("menu").w(150.0).h(90.0).fill(Rgba::new(0.2, 0.2, 0.2, 1.0)),
                )),
            ),
        ),
    )
}

#[test]
fn a_menu_from_inside_a_portal_fits_the_portal_not_the_main_window() {
    let mut rt =
        Runtime::new(Portal { pressed: false }, menu_view, Theme::new(Style::Liquid, Scheme::Dark, SYSTEM_BLUE), (200, 100), 1.0).unwrap();
    let region = Rect::new(1000.0, 0.0, 200.0, 160.0);
    rt.set_portals(&[region]);
    rt.frame(FRAME);
    let menu = rt.find("menu").expect("the menu shows");
    assert!(
        menu.left >= region.left && menu.right <= region.right && menu.bottom <= region.bottom,
        "{menu:?} inside {region:?}"
    );
}

#[test]
fn a_portal_on_a_monitor_of_another_scale_draws_at_that_scale() {
    let mut rt = Runtime::new(Portal { pressed: false }, view, Theme::new(Style::Liquid, Scheme::Dark, SYSTEM_BLUE), (200, 100), 1.0).unwrap();
    rt.frame(FRAME);
    let region = Rect::new(1000.0, 0.0, 120.0, 80.0);
    let image = rt.snap_region_at(region, 2.0).unwrap();
    assert_eq!((image.width, image.height), (240, 160), "the window's own pixels");
    let [b, _, r, _] = image.pixel(230, 150);
    assert!(r > 200 && b < 50, "the square fills it to the far corner at 2x");
}
