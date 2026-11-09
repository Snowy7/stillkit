//! Virtual lists: a scroll view whose view builds only the rows in sight (plus a viewport's
//! worth either side), between spacers that keep the scroll extent exact. When scrolling
//! nears the edge of what was built, the runtime runs the view again in the same frame, with
//! the new place in [`virtual_list`]'s reach.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;

use super::element::{Axis, El, Key, col, el};

/// What a virtual list built: its row count and height (design px) and the rows it holds.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Virt {
    pub count: usize,
    pub row: f32,
    pub built: Range<usize>,
}

impl Virt {
    /// The rows in sight at `offset` in a `viewport` (design px).
    pub fn visible(&self, offset: f32, viewport: f32) -> Range<usize> {
        let first = (offset / self.row).floor().max(0.0) as usize;
        let last = ((offset + viewport) / self.row).ceil().max(0.0) as usize;
        first.min(self.count)..last.min(self.count)
    }

    /// Whether the rows in sight left what was built.
    pub fn stale(&self, offset: f32, viewport: f32) -> bool {
        let seen = self.visible(offset, viewport);
        !seen.is_empty() && (seen.start < self.built.start || seen.end > self.built.end)
    }
}

thread_local! {
    /// Each virtual list's offset and viewport (design px) while the view runs.
    static VIEWPORTS: RefCell<HashMap<Key, (f32, f32)>> = RefCell::new(HashMap::new());
}

/// Runs `view` with the virtual lists' places in reach.
pub(crate) fn with_viewports<T>(places: HashMap<Key, (f32, f32)>, view: impl FnOnce() -> T) -> T {
    VIEWPORTS.with_borrow_mut(|v| *v = places);
    let out = view();
    VIEWPORTS.with_borrow_mut(HashMap::clear);
    out
}

/// A vertical list of `count` rows each `row` design px tall that builds only those near
/// sight: `rows(range)` makes the rows in `range`, in order, each exactly `row` tall (taller
/// rows overlap, shorter ones leave gaps). Key it: the key finds its scroll place across
/// view runs. Size it like any scroll view (`h`, `grow`).
///
/// ```ignore
/// virtual_list("hierarchy", s.rows.len(), 22.0, |range| range.map(|i| row_of(&s.rows[i])).collect())
///     .grow(1.0)
/// ```
///
/// Scrolling runs no view while the rows in sight stay within what was built (a viewport's
/// worth either side); past it, the runtime runs the view again in the same frame.
pub fn virtual_list<S: 'static>(
    key: impl Into<Key>,
    count: usize,
    row: f32,
    rows: impl FnOnce(Range<usize>) -> Vec<El<S>>,
) -> El<S> {
    let key = key.into();
    let row = row.max(1.0);
    let (offset, viewport) = VIEWPORTS.with_borrow(|v| v.get(&key).copied()).unwrap_or((0.0, 0.0));
    // Before the first layout the viewport is unknown: build a screenful.
    let viewport = if viewport > 0.0 { viewport } else { 64.0 * row };
    let first = ((offset - viewport) / row).floor().max(0.0) as usize;
    let last = (((offset + 2.0 * viewport) / row).ceil().max(0.0) as usize).min(count);
    let first = first.min(last);
    let built = first..last;
    let content = rows(built.clone());
    let mut list = col()
        .key(key)
        .scroll(Axis::Vertical)
        .child(el().h(first as f32 * row).key("virtual/before"))
        .children(content)
        .child(el().h((count - last) as f32 * row).key("virtual/after"));
    list.virt = Some(Virt { count, row, built });
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_in_sight_and_staleness() {
        let v = Virt { count: 1000, row: 20.0, built: 10..70 };
        assert_eq!(v.visible(400.0, 200.0), 20..30);
        assert!(!v.stale(400.0, 200.0));
        assert!(v.stale(1300.0, 200.0), "past the end of what was built");
        assert!(v.stale(100.0, 200.0), "before its start");
        assert_eq!(v.visible(19_990.0, 200.0), 999..1000, "clamped to the count");
    }
}
