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
    pub variable: Option<VirtualRows>,
}

impl Virt {
    /// The rows in sight at `offset` in a `viewport` (design px).
    pub fn visible(&self, offset: f32, viewport: f32) -> Range<usize> {
        if let Some(rows) = &self.variable {
            let rows = rows.0.borrow();
            return rows.at(offset)..(rows.at(offset + viewport) + 1).min(self.count);
        }
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
    list.virt = Some(Virt { count, row, built, variable: None });
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_in_sight_and_staleness() {
        let v = Virt { count: 1000, row: 20.0, built: 10..70, variable: None };
        assert_eq!(v.visible(400.0, 200.0), 20..30);
        assert!(!v.stale(400.0, 200.0));
        assert!(v.stale(1300.0, 200.0), "past the end of what was built");
        assert!(v.stale(100.0, 200.0), "before its start");
        assert_eq!(v.visible(19_990.0, 200.0), 999..1000, "clamped to the count");
    }
}

/// App-owned keyed measurements. Clones share the cache; use one per list.
#[derive(Debug, Clone)]
pub struct VirtualRows(std::rc::Rc<RefCell<Measurements>>);
#[derive(Debug)]
struct Measurements {
    keys: Vec<Key>,
    index: HashMap<Key, usize>,
    heights: HashMap<Key, f32>,
    sums: Vec<f32>,
    estimate: f32,
    anchor: Option<(Key, f32)>,
    offset: f32,
    pending: Option<Key>,
    focused: Option<Key>,
    reordered: bool,
}
impl Measurements {
    fn height(&self, i: usize) -> f32 { self.heights.get(&self.keys[i]).copied().unwrap_or(self.estimate) }
    fn prefix(&self, mut i: usize) -> f32 {
        let mut sum = 0.;
        while i > 0 { sum += self.sums[i]; i &= i - 1; }
        sum
    }
    fn add(&mut self, mut i: usize, delta: f32) {
        i += 1;
        while i < self.sums.len() { self.sums[i] += delta; i += i & i.wrapping_neg(); }
    }
    fn at(&self, offset: f32) -> usize {
        let (mut lo, mut hi) = (0, self.keys.len());
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.prefix(mid + 1) <= offset { lo = mid + 1; } else { hi = mid; }
        }
        lo.min(self.keys.len().saturating_sub(1))
    }
    fn anchored(&self) -> f32 {
        self.anchor.as_ref().and_then(|(k, within)| self.index.get(k).map(|&i| self.prefix(i) + within)).unwrap_or(self.offset)
    }
}
impl VirtualRows {
    /// Estimates unmeasured rows in design pixels.
    pub fn new(estimate: f32) -> Self {
        Self(std::rc::Rc::new(RefCell::new(Measurements { keys: Vec::new(), index: HashMap::new(), heights: HashMap::new(), sums: vec![0.], estimate: if estimate.is_finite() { estimate.max(1.) } else { 24. }, anchor: None, offset: 0., pending: None, focused: None, reordered: false })))
    }
    /// Replaces the order, retaining measurements by key and the reader's anchor.
    pub fn set_keys(&self, keys: impl IntoIterator<Item = impl Into<Key>>) {
        let mut m = self.0.borrow_mut();
        m.reordered = true;
        m.keys = keys.into_iter().map(Into::into).collect();
        m.index = m.keys.iter().cloned().enumerate().map(|(i,k)| (k,i)).collect();
        assert_eq!(m.keys.len(), m.index.len(), "virtual row keys must be unique");
        let present = m.index.clone();
        m.heights.retain(|k,_| present.contains_key(k));
        m.sums = vec![0.; m.keys.len() + 1];
        for i in 0..m.keys.len() { let h = m.height(i); m.add(i,h); }
    }
    /// Reveals a key on the next app update, even when its element is not built.
    pub fn reveal(&self, key: impl Into<Key>) { self.0.borrow_mut().pending = Some(key.into()); }
    /// Clears measurements after a width or font change, retaining the anchor.
    pub fn invalidate(&self) {
        self.0.borrow_mut().heights.clear();
        let keys = self.0.borrow().keys.clone(); self.set_keys(keys);
    }
    pub(crate) fn prepare(&self, offset: f32, focused: Option<Key>) {
        let mut m = self.0.borrow_mut();
        // A model edit may already have changed the order since the last frame.
        if !m.reordered { 
            let i = m.at(offset);
            m.anchor = m.keys.get(i).cloned().map(|k| (k, offset - m.prefix(i)));
        }
        m.offset = offset; m.focused = focused;
    }
    pub(crate) fn measure(&self, key: &Key, height: f32) -> bool {
        let mut m = self.0.borrow_mut();
        let Some(&i) = m.index.get(key) else { return false };
        let height = height.max(1.);
        let delta = height - m.height(i);
        if delta.abs() < 0.5 { return false; }
        m.heights.insert(key.clone(),height); m.add(i,delta); true
    }
    pub(crate) fn offset(&self) -> f32 { self.0.borrow().anchored() }
}
/// Builds natural-height rows near sight, keeping a focused row outside that range.
/// Each row receives its model key automatically.
pub fn variable_list<S: 'static>(key: impl Into<Key>, rows: &VirtualRows, mut build: impl FnMut(usize) -> El<S>) -> El<S> {
    let key = key.into();
    let (_, viewport) = VIEWPORTS.with_borrow(|v| v.get(&key).copied()).unwrap_or((0., 0.));
    let mut m = rows.0.borrow_mut();
    let viewport = if viewport > 0. { viewport } else { m.estimate * 64. };
    let mut offset = m.anchored();
    if let Some(i) = m.pending.take().and_then(|k| m.index.get(&k).copied()) {
        offset = m.prefix(i); m.anchor = Some((m.keys[i].clone(),0.)); m.offset = offset;
    }
    m.reordered = false;
    let count = m.keys.len();
    let first = m.at((offset - viewport).max(0.));
    let last = (m.at(offset + viewport * 2.) + 1).min(count);
    let built = first..last;
    let mut indices: Vec<usize> = built.clone().collect();
    if let Some(i) = m.focused.as_ref().and_then(|k| m.index.get(k)).copied() { indices.push(i); }
    indices.sort_unstable(); indices.dedup();
    let mut list = col().key(key).scroll(Axis::Vertical);
    let mut end = 0;
    for i in indices {
        if i > end { list = list.child(el().h(m.prefix(i) - m.prefix(end)).key(format!("virtual/gap/{i}"))); }
        list = list.child(build(i).key(m.keys[i].clone())); end = i + 1;
    }
    list = list.child(el().h(m.prefix(count) - m.prefix(end)).key("virtual/after"));
    drop(m);
    list.virt = Some(Virt { count, row: 1., built, variable: Some(rows.clone()) }); list
}
impl PartialEq for VirtualRows {
    fn eq(&self, other: &Self) -> bool { std::rc::Rc::ptr_eq(&self.0, &other.0) }
}
