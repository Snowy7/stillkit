//! A large table and file tree; --snap and --script use the headless runtime.
use stillkit::el::{self, App, El, col, row, text, token};
use stillkit::{Kind, Scheme, Theme};
use stillkit_kit::{
    SortDirection, TableColumn, TableModel, TreeModel, TreeNode, VirtualTable, VirtualTree,
    natural_cmp,
};
struct State {
    table: TableModel,
    tree: TreeModel,
    opened: String,
}
fn record_number(key: &str) -> usize {
    key.strip_prefix("record")
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}
fn view(s: &State) -> El<State> {
    let table = VirtualTable::new("records", &s.table, |s: &mut State| &mut s.table)
        .label("Records")
        .height(420.)
        .cell(|r, c| {
            let id = s.table.key(r).unwrap_or("record0");
            text(match c {
                0 => id.to_string(),
                1 => format!("{} KB", record_number(id) % 4096),
                _ => "Document".to_string(),
            })
            .pad_x(8.)
            .pad_y(5.)
        })
        .on_open(|s, id| s.opened = format!("Opened {id}"))
        .on_sort(|s, (column, direction)| {
            s.table.sort_by(|a, b| {
                let cmp = match column {
                    1 => (record_number(a) % 4096).cmp(&(record_number(b) % 4096)),
                    2 => std::cmp::Ordering::Equal,
                    _ => natural_cmp(a, b),
                };
                if direction == SortDirection::Ascending {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
        })
        .build();
    let tree = VirtualTree::new("files", &s.tree, |s: &mut State| &mut s.tree)
        .label("Project files")
        .height(390.)
        .on_open(|s, id| s.opened = format!("Opened {id}"))
        .build();
    let actions = row()
        .gap(12.)
        .child(
            text("Reveal file 42")
                .key("reveal")
                .on_press(|s: &mut State| {
                    s.tree.reveal("src4/file42.rs");
                }),
        )
        .child(
            text("Filter .rs")
                .key("filter")
                .on_press(|s: &mut State| s.tree.filter(|name| name.ends_with(".rs"))),
        )
        .child(
            text("Clear")
                .key("clear")
                .on_press(|s: &mut State| s.tree.clear_filter()),
        );
    let records = col()
        .w(520.)
        .gap(8.)
        .child(text("RECORDS").color(token::SECONDARY))
        .child(table);
    let files = col()
        .w(300.)
        .gap(8.)
        .child(text("PROJECT FILES").color(token::SECONDARY))
        .child(actions)
        .child(tree);
    let hint = text(
        "Drag column edges to resize | Click headers to sort | Ctrl/Shift select | Type to jump",
    );
    col()
        .pad(20.)
        .gap(12.)
        .child(text("Data workspace | 100,000 records").text_size(24.))
        .child(hint)
        .child(row().gap(16.).child(records).child(files))
        .child(text(if s.opened.is_empty() {
            "Select a row, then press Enter"
        } else {
            &s.opened
        }))
}
fn main() -> anyhow::Result<()> {
    let mut table = TableModel::new(vec![
        TableColumn::new("Name", 260., 120., 400.),
        TableColumn::new("Size", 120., 60., 200.),
        TableColumn::new("Type", 140., 80., 240.),
    ]);
    table.selection.multi = true;
    table.set_rows((0..100_000).map(|i| {
        let id: std::sync::Arc<str> = format!("record{i}").into();
        (id.clone(), id)
    }));
    let mut tree = TreeModel::new(
        (0..10)
            .map(|d| {
                let mut node = TreeNode::new(format!("src{d}"), format!("src{d}")).expanded(d == 0);
                for i in 0..100 {
                    let extension = if i % 5 == 0 { "md" } else { "rs" };
                    node = node.child(TreeNode::new(
                        format!("src{d}/file{i}.{extension}"),
                        format!("file{i}.{extension}"),
                    ));
                }
                node
            })
            .collect(),
    );
    tree.selection.multi = true;
    tree.sort_natural();
    el::run(App {
        title: "Data workspace",
        size: (900., 640.),
        kind: Kind::App { min: (900, 600) },
        theme: Theme::solid(Scheme::Dark, stillkit::theme::SYSTEM_BLUE),
        state: State {
            table,
            tree,
            opened: String::new(),
        },
        view,
    })
}
