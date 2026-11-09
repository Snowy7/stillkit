use std::time::Duration;
use stillkit::el::{El, Runtime, VirtualRows, el, variable_list};
use stillkit::{Scheme, Theme};
struct Model { rows: VirtualRows, ids: Vec<usize> }
fn view(s: &Model) -> El<Model> {
    variable_list("list", &s.rows, |i| el().h(if s.ids[i].is_multiple_of(2) { 40. } else { 20. }).focusable()).h(200.)
}
fn runtime() -> Runtime<Model> {
    let rows = VirtualRows::new(25.);
    rows.set_keys((0..100_000).map(|i| format!("r{i}")));
    let mut rt = Runtime::new(Model { rows, ids: (0..100_000).collect() }, view, Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE), (300,200), 1.).unwrap();
    rt.set_reduced_motion(true); rt.frame(Duration::ZERO); rt
}
#[test]
fn measured_rows_stay_bounded_and_small_scrolls_skip_view() {
    let mut rt = runtime();
    assert!(rt.tree().len() < 220);
    let runs = rt.view_runs();
    rt.wheel_at(100.,100.,-120); rt.frame(Duration::from_millis(16));
    assert_eq!(rt.view_runs(),runs);
}
#[test]
fn reveal_and_prepend_keep_the_key_at_the_same_place() {
    let mut rt = runtime();
    rt.update(|s| s.rows.reveal("r50000")); rt.frame(Duration::ZERO);
    assert!(rt.find("r50000").is_some());
    let before = rt.find("r50000").unwrap().top - rt.scroll_offset("list").unwrap();
    rt.update(|s| {
        s.ids.splice(0..0,100_000..100_010);
        s.rows.set_keys(s.ids.iter().map(|i| format!("r{i}")));
    });
    rt.frame(Duration::ZERO);
    let after = rt.find("r50000").unwrap().top - rt.scroll_offset("list").unwrap();
    assert!((before-after).abs() < 1., "{before} -> {after}");
}
