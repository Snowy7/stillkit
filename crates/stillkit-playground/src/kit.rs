//! Interactive, controlled kit stories. Helpers such as ChartSeries and TreeNode appear
//! inside their parent component's story; the deprecated TextFieldFrame has a compatibility story.
use super::{Session, Story};
use stillkit::el::{El, col, text};
use stillkit_kit::*;
/// Shared values for the built-in component stories.
#[derive(Default)]
pub struct Model {
    /// Toggle, disclosure and selection state.
    pub on: bool,
    /// Numeric control value.
    pub value: f32,
    /// Selected option.
    pub index: usize,
    /// Editor content.
    pub text: String,
    /// Combobox and palette state.
    pub query: QueryState,
    /// Current colour, including hue while grey.
    pub color: Option<Hsva>,
}
type S = Session<Model>;
/// Every supported kit component has a named story; all button kinds have separate stories.
pub fn stories() -> Vec<Story<Model>> {
    let mut stories = Vec::new();
    for name in [
        "Button/Filled",
        "Button/Plain",
        "Button/Tinted",
        "Button/Gray",
        "Button/Destructive",
        "IconButton",
        "Switch",
        "Checkbox",
        "RadioGroup",
        "Slider",
        "Segmented",
        "Progress",
        "Badge",
        "ListRow",
        "SectionHeader",
        "Tile",
        "Card",
        "Separator",
        "EmptyState",
        "CalendarMonth",
        "TextField",
        "TextFieldFrame",
        "SecureField",
        "SearchField",
        "TextArea",
        "Combobox",
        "CommandPalette",
        "Dialog",
        "AlertDialog",
        "Menu",
        "ContextMenu",
        "Popover",
        "Select",
        "Toast",
        "Toaster",
        "Tooltip",
        "ScrollView",
        "Table",
        "Tree",
        "Tabs",
        "Disclosure",
        "Accordion",
        "Sidebar",
        "Toolbar",
        "Stepper",
        "Chart",
        "QuotaBar",
        "Avatar",
        "Ring",
        "Skeleton",
        "Spinner",
        "ColorPicker",
    ] {
        stories.push(Story {
            name,
            view: move_view(name),
            before_snapshot: if name == "Tooltip" {
                Some(|rt| {
                    if let Some(r) = rt.find("subject") {
                        let p = r.center();
                        rt.pointer_move(p.x, p.y);
                        rt.frame(std::time::Duration::from_secs(1));
                    }
                })
            } else {
                None
            },
        });
    }
    stories
}
// Each function pointer remains a plain view; no erased model or serialized handlers.
#[allow(deprecated)]
fn move_view(name: &'static str) -> fn(&S) -> El<S> {
    macro_rules! cases { ($($name:literal => $body:expr),* $(,)?) => { match name { $($name => |s: &S| { let node: El<S> = ($body)(s); node.key("subject").preview_flags(s.knobs.disabled,s.knobs.invalid) },)* _ => unreachable!() } }; }
    cases! {
    "Button/Filled" => |_: &S| Button::new("Save changes").kind(ButtonKind::Filled).on_press(|s: &mut S|s.model.on= !s.model.on).build(),
    "Button/Plain" => |_: &S| Button::new("Plain").kind(ButtonKind::Plain).on_press(|_: &mut S|{}).build(),
    "Button/Tinted" => |_: &S| Button::new("Tinted").kind(ButtonKind::Tinted).on_press(|_: &mut S|{}).build(),
    "Button/Gray" => |_: &S| Button::new("Gray").kind(ButtonKind::Gray).on_press(|_: &mut S|{}).build(),
    "Button/Destructive" => |_: &S| Button::new("Delete").kind(ButtonKind::Destructive).on_press(|_: &mut S|{}).build(),
    "IconButton" => |_: &S| IconButton::new(Icon::Check,"Save").on_press(|_: &mut S|{}).build(),
    "Switch" => |s: &S| Switch::new("Sync",s.model.on).on_toggle(|s: &mut S|s.model.on= !s.model.on).build(),
    "Checkbox" => |s: &S| Checkbox::new("Send reports",s.model.on).on_toggle(|s: &mut S|s.model.on= !s.model.on).build(),
    "RadioGroup" => |s: &S| RadioGroup::new("Quality",["Fast","Balanced","Best"],s.model.index).on_change(|s: &mut S,v|s.model.index=v).build(),
    "Slider" => |s: &S| Slider::new("Volume",s.model.value).on_change(|s: &mut S,v|s.model.value=v).build(),
    "Segmented" => |s: &S| Segmented::new("View",["List","Grid"],s.model.index).on_change(|s: &mut S,v|s.model.index=v).build(),
    "Progress" => |_: &S| Progress::new("Downloading",0.65).build(),
    "Badge" => |_: &S| Badge::new("New").build(),
    "ListRow" => |_: &S| ListRow::new("Downloads").subtitle("3 files available offline").on_press(|_: &mut S|{}).build(),
    "SectionHeader" => |_: &S| SectionHeader::new("General").build(),
    "Tile" => |s: &S| Tile::new("Sync",Icon::Check,s.model.on).on_toggle(|s: &mut S|s.model.on= !s.model.on).build(),
    "Card" => |_: &S| Card::new().child(text("Account")).child(text("Your settings sync across devices.")).build(),
    "Separator" => |_: &S| col().child(text("Before")).child(Separator.build()).child(text("After")),
    "EmptyState" => |_: &S| EmptyState::new("All caught up","New downloads appear here.",Icon::Check).build(),
    "CalendarMonth" => |_: &S| CalendarMonth::new(2026,11).today(9).selected(12).on_select(|s: &mut S,v|s.model.index=v as usize).build(),
    "TextField" => |s: &S| TextField::new("Name").value(s.model.text.clone()).on_input(|s: &mut S,v|s.model.text=v.into()).build(),
    "TextFieldFrame" => |_: &S| TextFieldFrame::new("Legacy frame").value("Compatibility story").build(),
    "SecureField" => |s: &S| SecureField::new("Password").value(s.model.text.clone()).on_input(|s: &mut S,v|s.model.text=v.into()).build(),
    "SearchField" => |s: &S| SearchField::new("Search").value(s.model.text.clone()).on_input(|s: &mut S,v|s.model.text=v.into()).build(),
    "TextArea" => |s: &S| TextArea::new("Notes").value(s.model.text.clone()).on_input(|s: &mut S,v|s.model.text=v.into()).build(),
    "Combobox" => |s: &S| Combobox::new("Destination",["Doha","Oslo","Tokyo"],&s.model.query,|s: &mut S,v|s.model.query=v).build(),
    "CommandPalette" => |s: &S| col().overlay(CommandPalette::new(&QueryState {open:true,..s.model.query.clone()},|s: &mut S,v|s.model.query=v).command(Command::new("save","Save",|_: &mut S|{})).build()),
    "Dialog" => |_: &S| col().overlay(Dialog::new("Save settings?","Your changes apply immediately.",|_: &mut S|{}).build()),
    "AlertDialog" => |_: &S| col().overlay(AlertDialog::new("Delete profile?","This removes the saved profile.",|_: &mut S|{}).build()),
    "Menu" => |_: &S| Menu::new().item(MenuItem::new("Open").on_press(|_: &mut S|{})).item(MenuItem::new("Save").on_press(|_: &mut S|{})).build(),
    "ContextMenu" => |_: &S| ContextMenu::attach(text("Right click here"), Some((20.,60.)), Menu::new().item(MenuItem::new("Copy").on_press(|_: &mut S|{})), |_: &mut S,_,_| {}),
    "Popover" => |_: &S| Popover::new(text("Saved locally")).build(),
    "Select" => |s: &S| Select::new("Quality",["Fast","Best"],s.model.index,s.model.on,|s: &mut S,v|s.model.on=v,|s: &mut S,v|s.model.index=v).build(),
    "Toast" => |_: &S| Toast::new("saved","Settings saved").build(),
    "Toaster" => |_: &S| col().overlay(Toaster::new().child(Toast::new("saved","Settings saved").build()).build()),
    "Tooltip" => |_: &S| Tooltip::attach(Button::new("Hover me").on_press(|_: &mut S|{}).build(),"Save your changes"),
    "ScrollView" => |_: &S| ScrollView::new("Downloads",160.).child(text("First download")).child(text("Second download")).build(),
    "Table" => |_: &S| Table::new("Files",["Name","Size"]).row("one",["notes.txt","12 KB"],false).row("two",["photo.png","2 MB"],true).build(),
    "Tree" => |_: &S| Tree::new("Folders",vec![TreeNode::new("root","Home").expanded(true).child(TreeNode::new("notes","Notes"))]).build(),
    "Tabs" => |s: &S| Tabs::new("Settings",s.model.index).tab("General",text("General preferences")).tab("Advanced",text("Advanced preferences")).on_change(|s: &mut S,v|s.model.index=v).build(),
    "Disclosure" => |s: &S| Disclosure::new("Details",s.model.on,text("Additional settings")).on_toggle(|s: &mut S|s.model.on= !s.model.on).build(),
    "Accordion" => |s: &S| Accordion::new("Details",s.model.on,text("Additional settings")).on_toggle(|s: &mut S|s.model.on= !s.model.on).build(),
    "Sidebar" => |_: &S| Sidebar::new("Settings").item("general","General",Icon::Check,true,|_: &mut S|{}).item("account","Account",Icon::Check,false,|_: &mut S|{}).build(),
    "Toolbar" => |_: &S| Toolbar::new("Actions").action("save","Save",Icon::Check,|_: &mut S|{}).build(),
    "Stepper" => |s: &S| Stepper::new("Copies",0.,10.,1.,s.model.value.into()).on_change(|s: &mut S,v|s.model.value=v as f32).build(),
    "Chart" => |_: &S| Chart::new("Downloads",vec![ChartSeries::new("Files",vec![1.,4.,2.,8.],stillkit::el::token::ACCENT)]).build(),
    "QuotaBar" => |_: &S| QuotaBar::new("Storage",0.65).build(),
    "Avatar" => |_: &S| Avatar::new("Islam","IS").status("Online").build(),
    "Ring" => |_: &S| Ring::new("Complete",0.7).build(),
    "Skeleton" => |_: &S| Skeleton::new(200.,24.).build(),
    "ColorPicker" => |s: &S| ColorPicker::new(s.model.color.unwrap_or_else(|| Hsva::from_rgba(stillkit::theme::SYSTEM_BLUE))).alpha(true).on_change(|s: &mut S, color, _| s.model.color=Some(color)).build(),
    "Spinner" => |_: &S| Spinner::new("Loading",0).build()
    }
}
