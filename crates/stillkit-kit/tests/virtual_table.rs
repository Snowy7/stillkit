use std::time::Duration;
use stillkit::el::{El, KeyCode, Modifiers, Runtime, text};
use stillkit::{Scheme, Semantics, Theme};
use stillkit_kit::{TableColumn, TableModel, VirtualTable};
struct State {
    table: TableModel,
    opened: Option<String>,
}
fn view(s: &State) -> El<State> {
    VirtualTable::new("grid", &s.table, |s: &mut State| &mut s.table)
        .label("Records")
        .height(280.)
        .cell(|r, c| text(format!("{r}:{c}")))
        .on_open(|s, k| s.opened = Some(k.to_string()))
        .on_sort(|_, _| {})
        .build()
}
fn rt() -> Runtime<State> {
    let mut table = TableModel::new(vec![
        TableColumn::new("Name", 200., 60., 300.),
        TableColumn::new("Value", 100., 40., 200.),
    ]);
    table.selection.multi = true;
    table.set_rows((0..100_000).map(|i| (format!("r{i}").into(), format!("Record {i}").into())));
    let mut rt = Runtime::new(
        State {
            table,
            opened: None,
        },
        view,
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (400, 308),
        1.,
    )
    .unwrap();
    rt.set_reduced_motion(true);
    rt.frame(Duration::ZERO);
    rt
}
#[test]
fn virtual_table_builds_only_nearby_cells_and_reports_full_counts() {
    let mut rt = rt();
    assert!(rt.tree().len() < 1800);
    assert!(rt.semantics().iter().any(|(_,s)|matches!(s,Semantics::Detail(d) if d.collection.as_deref().is_some_and(|d| d.row_count==Some(100001) && d.column_count==Some(2)))));
    let runs = rt.view_runs();
    rt.wheel_at(100., 100., -120);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.view_runs(), runs);
    rt.focus("grid");
    rt.key(KeyCode::END.0);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().table.selection.cursor.as_deref(), Some("r99999"));
    assert!(rt.find("r99999").is_some());
    rt.key(KeyCode::ENTER.0);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().opened.as_deref(), Some("r99999"));
}
#[test]
fn shift_keys_extend_selection_and_sort_headers_toggle() {
    let mut rt = rt();
    rt.focus("grid");
    rt.key(KeyCode::HOME.0);
    rt.frame(Duration::ZERO);
    rt.key_with(KeyCode::DOWN.0, Modifiers::SHIFT);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().table.selection.selected.len(), 2);
    assert!(rt.click("grid/sort/0"));
    rt.frame(Duration::ZERO);
    assert_eq!(
        rt.state().table.sort,
        Some((0, stillkit_kit::SortDirection::Ascending))
    );
    assert!(rt.click("grid/sort/0"));
    rt.frame(Duration::ZERO);
    assert_eq!(
        rt.state().table.sort,
        Some((0, stillkit_kit::SortDirection::Descending))
    );
}
#[test]
fn column_drag_persists_and_clamps() {
    let mut rt = rt();
    let rect = rt.find("grid/resize/0").unwrap();
    let x = (rect.left + rect.right) / 2.;
    let y = (rect.top + rect.bottom) / 2.;
    rt.pointer_down(x, y);
    rt.frame(Duration::ZERO);
    rt.pointer_move(x + 1000., y);
    rt.frame(Duration::ZERO);
    rt.pointer_up(x + 1000., y);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().table.columns[0].width, 300.);
}
#[test]
fn ctrl_click_toggles_rows_and_type_ahead_reveals_the_match() {
    let mut rt = rt();
    assert!(rt.click("r0"));
    rt.frame(Duration::ZERO);
    rt.set_modifiers(Modifiers::CTRL);
    assert!(rt.click("r1"));
    rt.set_modifiers(Modifiers::NONE);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().table.selection.selected.len(), 2);
    rt.set_modifiers(Modifiers::SHIFT);
    assert!(rt.click("r4"));
    rt.set_modifiers(Modifiers::NONE);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().table.selection.selected.len(), 4);
    rt.focus("grid");
    rt.frame(Duration::ZERO);
    rt.type_text("Record 90000");
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().table.selection.cursor.as_deref(), Some("r90000"));
    assert!(rt.find("r90000").is_some());
}

#[test]
fn defaults_fill_parent_and_leave_sort_and_open_unhandled() {
    fn defaults(s: &State) -> El<State> {
        VirtualTable::new("grid", &s.table, |s: &mut State| &mut s.table).build()
    }
    let mut table = TableModel::new(vec![TableColumn::new("Name", 200., 60., 300.)]);
    table.set_rows((0..100).map(|i| (format!("r{i}").into(), format!("Record {i}").into())));
    let mut rt = Runtime::new(
        State {
            table,
            opened: None,
        },
        defaults,
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (400, 308),
        1.,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    let body = rt.find("grid/body").unwrap();
    assert_eq!(body.bottom - body.top, 280.);
    rt.click("grid/sort/0");
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().table.sort, None);
    rt.focus("grid");
    rt.key(KeyCode::HOME.0);
    rt.frame(Duration::ZERO);
    rt.key(KeyCode::ENTER.0);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().opened, None);
}
