//! Optional, local-only Markdown document rendering.
use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::sync::Arc;
use stillkit::TextRole;
use stillkit::el::{self, Clipboard, El, Run, token};
type Handler<S> = Arc<dyn Fn(&mut S, &str)>;
/// A selectable Markdown document. Parsing never fetches URLs or images.
pub struct Markdown<S> {
    source: Arc<str>,
    link: Option<Handler<S>>,
    copy: Option<Handler<S>>,
}
impl<S: 'static> Markdown<S> {
    /// Parses source when `build` is called by the app's view.
    pub fn new(source: impl Into<Arc<str>>) -> Self {
        Self {
            source: source.into(),
            link: None,
            copy: None,
        }
    }
    /// Delegates link activation to the app.
    pub fn on_link(mut self, handler: impl Fn(&mut S, &str) + 'static) -> Self {
        self.link = Some(Arc::new(handler));
        self
    }
    /// Overrides copying fenced code, for an app-owned clipboard or host.
    pub fn on_copy(mut self, handler: impl Fn(&mut S, &str) + 'static) -> Self {
        self.copy = Some(Arc::new(handler));
        self
    }
    /// Builds a single selection document using the current theme's typography and paints.
    pub fn build(self) -> El<S> {
        let blocks = parse(&self.source);
        el::col()
            .gap(token::GAP)
            .children(
                blocks
                    .into_iter()
                    .map(|b| render(b, &self.link, &self.copy)),
            )
            .selectable()
    }
}
#[derive(Debug, Clone)]
enum Kind {
    Root,
    Paragraph,
    Heading(u8),
    Code,
    List(Option<u64>),
    Item,
    Quote,
    Table(Vec<Alignment>),
    Row(bool),
    Cell,
    Rule,
}
#[derive(Debug, Clone)]
struct Block {
    kind: Kind,
    runs: Vec<Run>,
    children: Vec<Block>,
}
impl Block {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            runs: Vec::new(),
            children: Vec::new(),
        }
    }
}
fn parse(source: &str) -> Vec<Block> {
    let mut stack = vec![Block::new(Kind::Root)];
    let mut marks = vec![Run::new("")];
    let parser = Parser::new_ext(
        source,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    );
    for event in parser {
        match event {
            Event::Start(tag) => {
                let kind = match tag {
                    Tag::Paragraph => Some(Kind::Paragraph),
                    Tag::Heading { level, .. } => Some(Kind::Heading(match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    })),
                    Tag::CodeBlock(_) => Some(Kind::Code),
                    Tag::List(n) => Some(Kind::List(n)),
                    Tag::Item => Some(Kind::Item),
                    Tag::BlockQuote(_) => Some(Kind::Quote),
                    Tag::Table(align) => Some(Kind::Table(align)),
                    Tag::TableHead => Some(Kind::Row(true)),
                    Tag::TableRow => Some(Kind::Row(false)),
                    Tag::TableCell => Some(Kind::Cell),
                    Tag::Strong | Tag::Emphasis | Tag::Strikethrough | Tag::Link { .. } => {
                        let mut run = marks.last().unwrap().clone();
                        match tag {
                            Tag::Strong => run.weight = Some(700),
                            Tag::Emphasis => run.italic = true,
                            Tag::Strikethrough => run.strike = true,
                            Tag::Link { dest_url, .. } => {
                                run.link = Some(dest_url.to_string().into())
                            }
                            _ => {}
                        }
                        marks.push(run);
                        None
                    }
                    _ => None,
                };
                if let Some(kind) = kind {
                    stack.push(Block::new(kind));
                }
            }
            Event::End(tag) => match tag {
                TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough | TagEnd::Link => {
                    if marks.len() > 1 {
                        marks.pop();
                    }
                }
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::CodeBlock
                | TagEnd::List(_)
                | TagEnd::Item
                | TagEnd::BlockQuote(_)
                | TagEnd::Table
                | TagEnd::TableHead
                | TagEnd::TableRow
                | TagEnd::TableCell => {
                    if stack.len() > 1 {
                        let block = stack.pop().unwrap();
                        stack.last_mut().unwrap().children.push(block);
                    }
                }
                _ => {}
            },
            Event::Code(value) => {
                let mut run = marks.last().unwrap().clone();
                run.code = true;
                run.text = value.to_string().into();
                stack.last_mut().unwrap().runs.push(run);
            }
            Event::Text(value) => {
                let mut run = marks.last().unwrap().clone();
                run.text = value.to_string().into();
                if matches!(stack.last().unwrap().kind, Kind::Code) {
                    run.code = true;
                }
                stack.last_mut().unwrap().runs.push(run);
            }
            Event::SoftBreak | Event::HardBreak => {
                let mut run = marks.last().unwrap().clone();
                run.text = if matches!(event, Event::SoftBreak) {
                    " "
                } else {
                    "\n"
                }
                .into();
                stack.last_mut().unwrap().runs.push(run);
            }
            Event::Rule => stack
                .last_mut()
                .unwrap()
                .children
                .push(Block::new(Kind::Rule)),
            // Raw HTML is text-free; image alt text arrives as ordinary Text events.
            _ => {}
        }
    }
    stack.pop().unwrap().children
}
fn render<S: 'static>(
    block: Block,
    link: &Option<Handler<S>>,
    copy_handler: &Option<Handler<S>>,
) -> El<S> {
    let paragraph = |runs| {
        let mut text = el::rich_text(runs).w_full();
        if let Some(handler) = link {
            let handler = handler.clone();
            text = text.on_link(move |s, url| handler(s, url));
        }
        text
    };
    match block.kind {
        Kind::Paragraph | Kind::Cell => paragraph(block.runs),
        Kind::Heading(level) => paragraph(block.runs)
            .font(if level == 1 {
                TextRole::Title1
            } else if level == 2 {
                TextRole::Title2
            } else {
                TextRole::Headline
            })
            .weight(700)
            .role(stillkit::Role::Heading),
        Kind::Rule => el::el().h(1.0).w_full().fill(token::SEPARATOR),
        Kind::Code => {
            let copy: Arc<str> = block
                .runs
                .iter()
                .map(|r| &*r.text)
                .collect::<String>()
                .into();
            let copy_handler = copy_handler.clone();
            el::col()
                .pad(token::PAD)
                .gap(token::GAP)
                .fill(token::WELL)
                .radius(token::RADIUS_SMALL)
                .child(
                    el::row().justify(el::Justify::End).child(
                        crate::Button::new("Copy code")
                            .kind(crate::ButtonKind::Plain)
                            .on_press(move |state: &mut S| {
                                if let Some(handler) = &copy_handler {
                                    handler(state, &copy);
                                } else {
                                    let mut clipboard = el::SystemClipboard;
                                    clipboard.set(&copy);
                                }
                            })
                            .build(),
                    ),
                )
                .child(paragraph(block.runs))
        }
        Kind::List(first) => {
            el::col()
                .gap(token::GAP)
                .children(block.children.into_iter().enumerate().map(|(i, child)| {
                    el::row()
                        .align(el::Align::Start)
                        .gap(token::GAP)
                        .child(el::text(first.map_or_else(
                            || "•".into(),
                            |n| format!("{}.", n.saturating_add(i as u64)),
                        )))
                        .child(render(child, link, copy_handler).grow(1.0).min_w(0.0))
                }))
        }
        Kind::Quote => el::col()
            .pad(token::PAD)
            .border(1.0, token::SEPARATOR)
            .children(
                block
                    .children
                    .into_iter()
                    .map(|b| render(b, link, copy_handler)),
            ),
        Kind::Row(header) => el::grid(block.children.len().clamp(1, u16::MAX as usize) as u16)
            .gap(token::GAP)
            .children(block.children.into_iter().map(|b| {
                let cell = render(b, link, copy_handler);
                if header { cell.weight(700) } else { cell }
            })),
        Kind::Table(alignments) => el::col()
            .gap(token::GAP)
            .pad(token::GAP)
            .border(1.0, token::SEPARATOR)
            .children(block.children.into_iter().map(|row| {
                let header = matches!(row.kind, Kind::Row(true));
                el::grid(row.children.len().clamp(1, u16::MAX as usize) as u16)
                    .gap(token::GAP)
                    .children(row.children.into_iter().zip(alignments.iter()).map(
                        |(cell, alignment)| {
                            let text = render(cell, link, copy_handler).min_w(0.0).text_align(
                                match alignment {
                                    Alignment::Center => el::TextAlign::Center,
                                    Alignment::Right => el::TextAlign::End,
                                    _ => el::TextAlign::Start,
                                },
                            );
                            if header { text.weight(700) } else { text }
                        },
                    ))
            })),
        Kind::Root | Kind::Item => {
            let mut content = el::col().gap(token::GAP);
            if !block.runs.is_empty() {
                content = content.child(paragraph(block.runs));
            }
            content.children(
                block
                    .children
                    .into_iter()
                    .map(|b| render(b, link, copy_handler)),
            )
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_marks_links_and_lists_preserve_content() {
        let blocks = parse(
            "# Heading\n\n**strong and *italic*** [link](app:docs)\n\n3. first\n   - nested\n4. second\n\n| a | b |\n| - | - |\n| c | d |\n",
        );
        assert!(matches!(blocks[0].kind, Kind::Heading(1)));
        assert!(
            blocks[1]
                .runs
                .iter()
                .any(|r| r.weight == Some(700) && r.italic)
        );
        assert!(
            blocks[1]
                .runs
                .iter()
                .any(|r| r.link.as_deref() == Some("app:docs"))
        );
        assert!(matches!(blocks[2].kind, Kind::List(Some(3))));
        assert!(matches!(
            blocks[2].children[0].children[0].kind,
            Kind::List(None)
        ));
        assert!(matches!(blocks[3].kind, Kind::Table(_)));
    }
    #[test]
    fn code_and_image_alt_text_are_local_only() {
        let blocks = parse(
            "`inline` ![alt](https://example.invalid/image.png)\n\n```rust\nlet n = 1;\n```\n",
        );
        assert!(
            blocks[0]
                .runs
                .iter()
                .any(|r| r.code && &*r.text == "inline")
        );
        assert!(blocks[0].runs.iter().any(|r| &*r.text == "alt"));
        assert!(matches!(blocks[1].kind, Kind::Code));
        assert!(blocks[1].runs.iter().all(|r| r.code));
    }
}
