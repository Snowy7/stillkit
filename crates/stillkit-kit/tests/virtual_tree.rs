use std::time::Duration;
use stillkit::el::{El, KeyCode, Modifiers, Runtime};
use stillkit::{Scheme, Semantics, Theme};
use stillkit_kit::{TreeModel, TreeNode, VirtualTree, natural_cmp};
fn view(s: &TreeModel) -> El<TreeModel> {
    VirtualTree::new("files", s, |s| s)
        .label("Files")
        .height(280.)
        .build()
}
fn rt() -> Runtime<TreeModel> {
    let model = TreeModel::new(vec![
        TreeNode::new("root", "Root")
            .child(TreeNode::new("dir", "Directory").child(TreeNode::new("leaf", "file10.rs")))
            .child(TreeNode::new("other", "file2.rs")),
    ]);
    let mut rt = Runtime::new(
        model,
        view,
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (400, 280),
        1.,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    rt
}
#[test]
fn arrows_expand_descend_and_return_to_parent() {
    let mut rt = rt();
    rt.focus("files");
    rt.key(KeyCode::HOME.0);
    rt.frame(Duration::ZERO);
    rt.key(KeyCode::RIGHT.0);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().visible_len(), 3);
    rt.key(KeyCode::RIGHT.0);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().selection.cursor.as_deref(), Some("dir"));
    rt.key(KeyCode::LEFT.0);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().selection.cursor.as_deref(), Some("root"));
    rt.key(KeyCode::LEFT.0);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().visible_len(), 1);
}
#[test]
fn reveal_opens_deep_paths_filter_keeps_ancestors_and_indexes() {
    let mut rt = rt();
    rt.update(|m| {
        assert!(m.reveal("leaf"));
    });
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().visible_len(), 4);
    assert!(rt.find("leaf").is_some());
    assert!(rt.semantics().iter().any(|(_,s)|matches!(s,Semantics::Detail(d) if d.collection.as_deref().is_some_and(|d| d.level==Some(3) && d.position_in_set==Some(1) && d.size_of_set==Some(1)))));
    rt.update(|m| m.filter(|label| label.contains("10")));
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().visible_len(), 3);
    assert_eq!(rt.state().visible_key(0), Some("root"));
    rt.update(|m| m.selection.multi = true);
    rt.focus("files");
    rt.key(KeyCode::HOME.0);
    rt.frame(Duration::ZERO);
    rt.key_with(KeyCode::END.0, Modifiers::SHIFT);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().selection.selected.len(), 3);
}
#[test]
fn hundred_thousand_tree_rows_have_bounded_builds() {
    let m = TreeModel::new(
        (0..100_000)
            .map(|i| TreeNode::new(format!("n{i}"), format!("file{i}")))
            .collect(),
    );
    let mut rt = Runtime::new(
        m,
        view,
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (400, 280),
        1.,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    assert!(rt.tree().len() < 1000);
    let runs = rt.view_runs();
    rt.set_reduced_motion(true);
    rt.wheel_at(100., 100., -120);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.view_runs(), runs);
    rt.focus("files");
    rt.key(KeyCode::END.0);
    rt.frame(Duration::ZERO);
    assert!(rt.find("n99999").is_some());
}
#[test]
fn filenames_sort_by_numeric_runs_without_overflow() {
    let mut names = [
        "file10",
        "file02",
        "file2",
        "file9999999999999999999999999999999999999999",
        "file3",
    ];
    names.sort_by(|a, b| natural_cmp(a, b));
    assert_eq!(names[..4], ["file2", "file02", "file3", "file10"]);
}
#[test]
fn type_ahead_uses_visible_tree_order() {
    let mut rt = rt();
    rt.update(|m| m.set_expanded("root", true));
    rt.frame(Duration::ZERO);
    rt.focus("files");
    rt.type_text("file");
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().selection.cursor.as_deref(), Some("other"));
}

#[test]
fn right_arrow_does_not_enter_a_sibling_when_filter_hides_every_child() {
    let mut model = TreeModel::new(vec![
        TreeNode::new("a", "Root A").child(TreeNode::new("hidden", "Hidden")),
        TreeNode::new("b", "Root B"),
    ]);
    model.filter(|label| label.starts_with("Root"));
    let mut rt = Runtime::new(
        model,
        view,
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (400, 280),
        1.,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    rt.focus("files");
    rt.frame(Duration::ZERO);
    rt.key(KeyCode::HOME.0);
    rt.frame(Duration::ZERO);
    for _ in 0..2 {
        rt.key(KeyCode::RIGHT.0);
        rt.frame(Duration::ZERO);
    }
    assert_eq!(rt.state().selection.cursor.as_deref(), Some("a"));
}

#[test]
fn enter_and_space_toggle_branches() {
    let mut rt = rt();
    rt.focus("files");
    rt.frame(Duration::ZERO);
    rt.key(KeyCode::HOME.0);
    rt.frame(Duration::ZERO);
    rt.key(KeyCode::ENTER.0);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().visible_len(), 3);
    rt.key(KeyCode::SPACE.0);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().visible_len(), 1);
}

#[test]
fn defaults_fill_parent_and_leaf_open_is_optional() {
    fn defaults(s: &TreeModel) -> El<TreeModel> {
        VirtualTree::new("files", s, |s| s).build()
    }
    let mut rt = Runtime::new(
        TreeModel::new(vec![TreeNode::new("leaf", "Leaf")]),
        defaults,
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (400, 280),
        1.,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    let body = rt.find("files/body").unwrap();
    assert_eq!(body.bottom - body.top, 280.);
    rt.focus("files");
    rt.key(KeyCode::HOME.0);
    rt.frame(Duration::ZERO);
    rt.key(KeyCode::ENTER.0);
    rt.frame(Duration::ZERO);
}

#[test]
fn configured_open_receives_the_leaf_key() {
    fn view(s: &(TreeModel, Option<String>)) -> El<(TreeModel, Option<String>)> {
        VirtualTree::new("files", &s.0, |s: &mut (TreeModel, Option<String>)| {
            &mut s.0
        })
        .on_open(|s, key| s.1 = Some(key.to_string()))
        .build()
    }
    let mut rt = Runtime::new(
        (TreeModel::new(vec![TreeNode::new("leaf", "Leaf")]), None),
        view,
        Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        (400, 280),
        1.,
    )
    .unwrap();
    rt.frame(Duration::ZERO);
    rt.focus("files");
    rt.key(KeyCode::HOME.0);
    rt.frame(Duration::ZERO);
    rt.key(KeyCode::ENTER.0);
    rt.frame(Duration::ZERO);
    assert_eq!(rt.state().1.as_deref(), Some("leaf"));
}
