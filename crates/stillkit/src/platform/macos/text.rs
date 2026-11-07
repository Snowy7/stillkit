//! CoreText shaping, cached per thread and shared by measurement and drawing. Families that
//! aren't installed (Segoe UI Variable, the Windows default) and [`SYSTEM`] resolve to the
//! system UI font, SF Pro, at the asked weight; installed families match their nearest face.
//! A string is one `CTLine` per hard line; its box is the font's line height per line, and
//! its width includes trailing spaces, as DirectWrite measures on Windows.
pub use crate::text_style::{TextStyle, VARIABLE};
use objc2_app_kit::NSFont;
use objc2_core_foundation::{
    CFAttributedString, CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType, CGPoint,
};
use objc2_core_text::{
    CTFont, CTFontDescriptor, CTFontManagerCopyAvailableFontFamilyNames, CTFontSymbolicTraits,
    CTLine, kCTFontAttributeName, kCTFontFamilyNameAttribute, kCTFontTraitsAttribute,
    kCTFontWeightTrait, kCTForegroundColorFromContextAttributeName, kCTKernAttributeName,
};
use std::{
    collections::{HashMap, HashSet},
    ptr::NonNull,
    sync::Arc,
};

/// The system UI family's name here; it (and its other spellings) draws with SF Pro.
pub const SYSTEM: &str = "SF Pro";
const SYSTEM_NAMES: [&str; 5] = [SYSTEM, "SF Pro Text", "SF Pro Display", ".AppleSystemUIFont", "system-ui"];

type Key = (Arc<str>, Arc<str>, [u32; 3], u16, bool);
type FontKey = (Arc<str>, u32, u16, bool);

/// One shaped string: a line per hard line, with the font's metrics.
pub struct Layout {
    pub(crate) lines: Vec<(CFRetained<CTLine>, f64)>,
    pub(crate) ascent: f64,
    /// Ascent, descent and leading: the distance from one line's top to the next's.
    pub(crate) line_height: f64,
    pub(crate) width: f32,
}

impl Layout {
    pub(crate) fn height(&self) -> f32 {
        (self.line_height * self.lines.len().max(1) as f64) as f32
    }
}

/// Per-thread font matching and a bounded shaped-line cache.
pub struct Text {
    cache: HashMap<Key, Layout>,
    fonts: HashMap<FontKey, CFRetained<CTFont>>,
    families: std::cell::OnceCell<HashSet<String>>,
    capacity: usize,
}

impl Text {
    /// Creates a new instance.
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self { cache: HashMap::new(), fonts: HashMap::new(), families: Default::default(), capacity: 512 })
    }

    /// The font for a style, matched once per family, size, weight and slant.
    pub(crate) fn font(&mut self, style: &TextStyle, italic: bool) -> CFRetained<CTFont> {
        let key = (style.family.clone(), style.size.max(1.0).to_bits(), style.weight, italic);
        if let Some(font) = self.fonts.get(&key) {
            return font.clone();
        }
        let size = style.size.max(1.0) as f64;
        let installed = !SYSTEM_NAMES.contains(&&*style.family) && self.has_family(&style.family);
        let font = installed
            .then(|| named(&style.family, size, style.weight))
            .flatten()
            .unwrap_or_else(|| system(size, style.weight));
        let font = if italic {
            // SAFETY: a live font; a null matrix keeps the font's own.
            unsafe {
                font.copy_with_symbolic_traits(
                    0.0,
                    std::ptr::null(),
                    CTFontSymbolicTraits::ItalicTrait,
                    CTFontSymbolicTraits::ItalicTrait,
                )
            }
            .unwrap_or(font)
        } else {
            font
        };
        if self.fonts.len() >= 256 {
            self.fonts.clear();
        }
        self.fonts.insert(key, font.clone());
        font
    }

    /// The shaped string, from the cache when it was shaped before.
    pub(crate) fn shape(&mut self, text: &str, style: &TextStyle, italic: bool) -> &Layout {
        let key = (
            Arc::<str>::from(text),
            style.family.clone(),
            [style.size.to_bits(), style.tracking.to_bits(), style.optical.to_bits()],
            style.weight,
            italic,
        );
        if !self.cache.contains_key(&key) {
            if self.cache.len() >= self.capacity {
                self.cache.clear();
            }
            let font = self.font(style, italic);
            let layout = lay_out(text, font, style.tracking);
            self.cache.insert(key.clone(), layout);
        }
        &self.cache[&key]
    }

    /// Measures the text in device pixels.
    pub fn measure(&mut self, text: &str, style: &TextStyle) -> (f32, f32) {
        if text.chars().count() == 1 && text.chars().next().and_then(crate::icons::from_system_glyph).is_some() {
            return (style.size, style.size);
        }
        let l = self.shape(text, style, false);
        (l.width, l.height())
    }

    /// Releases cached native text layouts.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Sets capacity.
    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity.max(1);
        self.cache.clear();
    }

    /// Installed families, listed once per thread when first asked.
    fn family_set(&self) -> &HashSet<String> {
        self.families.get_or_init(|| {
            // SAFETY: CoreText's family list, an array of CFStrings.
            let names = unsafe { CTFontManagerCopyAvailableFontFamilyNames() };
            // SAFETY: the array holds only CFStrings.
            let names = unsafe { names.cast_unchecked::<CFString>() };
            names.iter().map(|n| n.to_string()).collect()
        })
    }

    /// Has family.
    pub fn has_family(&self, name: &str) -> bool {
        SYSTEM_NAMES.contains(&name) || self.family_set().contains(name)
    }

    /// Families.
    pub fn families(&self) -> Vec<String> {
        let mut names: Vec<String> = self.family_set().iter().cloned().collect();
        names.push(SYSTEM.into());
        names.sort();
        names.dedup();
        names
    }

    /// `text` as it fits in `width` device pixels: whole, or cut with an ellipsis where the
    /// line's own hit test says it must end.
    pub fn fit(&mut self, text: &str, style: &TextStyle, width: f32) -> Arc<str> {
        if self.measure(text, style).0 <= width {
            return text.into();
        }
        let ellipsis = self.measure("…", style).0;
        let first = text.lines().next().unwrap_or("");
        let at = {
            let l = self.shape(first, style, false);
            let Some((line, _)) = l.lines.first() else { return "".into() };
            // SAFETY: a live line; the position is in its own coordinates.
            let utf16 = unsafe { line.string_index_for_position(CGPoint::new((width - ellipsis).max(0.0) as f64, 0.0)) };
            byte_at(first, utf16.max(0) as usize)
        };
        let mut cut: String = first[..at].into();
        loop {
            let candidate = format!("{}…", cut.trim_end());
            if cut.is_empty() || self.measure(&candidate, style).0 <= width {
                return if cut.is_empty() && ellipsis > width { "".into() } else { candidate.into() };
            }
            cut.pop();
        }
    }

    /// Lines.
    pub fn lines(&mut self, text: &str, style: &TextStyle, width: f32, max: usize) -> Vec<Arc<str>> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in text.split_whitespace() {
            let candidate = if line.is_empty() { word.into() } else { format!("{line} {word}") };
            if !line.is_empty() && self.measure(&candidate, style).0 > width {
                lines.push(Arc::from(line.as_str()));
                line = word.into();
            } else {
                line = candidate;
            }
        }
        if !line.is_empty() {
            lines.push(line.into());
        }
        if lines.len() > max {
            lines.truncate(max);
            if let Some(last) = lines.last_mut() {
                *last = self.fit(&format!("{last}…"), style, width);
            }
        }
        lines
    }
}

/// The byte offset of the char boundary at or before a UTF-16 index.
pub(crate) fn byte_at(s: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units + c.len_utf16() > utf16 {
            return i;
        }
        units += c.len_utf16();
    }
    s.len()
}

/// NSFontWeight for a CSS weight (100..=900), between AppKit's named weights.
fn ns_weight(weight: u16) -> f64 {
    const STEPS: [f64; 9] = [-0.8, -0.6, -0.4, 0.0, 0.23, 0.3, 0.4, 0.56, 0.62];
    let w = (weight.clamp(100, 900) as f64 - 100.0) / 100.0;
    let (i, t) = (w.floor() as usize, w.fract());
    STEPS[i] + (STEPS[(i + 1).min(8)] - STEPS[i]) * t
}

/// SF Pro at `size` and `weight` (AppKit's system font, toll-free bridged to CTFont).
fn system(size: f64, weight: u16) -> CFRetained<CTFont> {
    let font = NSFont::systemFontOfSize_weight(size, ns_weight(weight));
    let ptr = NonNull::from(&*font).cast::<CTFont>();
    // SAFETY: NSFont and CTFont are toll-free bridged; retaining keeps it past `font`.
    unsafe { CFRetained::retain(ptr) }
}

/// An installed family's face nearest `weight`, or `None` when CoreText substituted another.
fn named(family: &str, size: f64, weight: u16) -> Option<CFRetained<CTFont>> {
    let name = CFString::from_str(family);
    let traits = CFDictionary::<CFString, CFNumber>::from_slices(
        // SAFETY: extern statics CoreText defines.
        &[unsafe { kCTFontWeightTrait }],
        &[&CFNumber::new_f64(ns_weight(weight))],
    );
    let attributes = CFDictionary::<CFString, CFType>::from_slices(
        // SAFETY: as above.
        &[unsafe { kCTFontFamilyNameAttribute }, unsafe { kCTFontTraitsAttribute }],
        &[name.as_ref(), traits.as_ref()],
    );
    // SAFETY: a descriptor from a dictionary of CoreText's own keys; a null matrix is identity.
    let font = unsafe {
        let descriptor = CTFontDescriptor::with_attributes(attributes.as_opaque());
        CTFont::with_font_descriptor(&descriptor, size, std::ptr::null())
    };
    // SAFETY: a live font.
    (unsafe { font.family_name() }.to_string() == family).then_some(font)
}

/// Shapes `text` with `font`, a line per hard line. Glyphs take the context's fill colour.
fn lay_out(text: &str, font: CFRetained<CTFont>, tracking: f32) -> Layout {
    let kern = CFNumber::new_f64(tracking as f64);
    // SAFETY: extern statics CoreText defines.
    let (font_key, kern_key, context_key) =
        unsafe { (kCTFontAttributeName, kCTKernAttributeName, kCTForegroundColorFromContextAttributeName) };
    let yes = CFBoolean::new(true);
    let mut keys: Vec<&CFString> = vec![font_key, context_key];
    let mut values: Vec<&CFType> = vec![font.as_ref(), yes.as_ref()];
    if tracking != 0.0 {
        keys.push(kern_key);
        values.push(kern.as_ref());
    }
    let attributes = CFDictionary::<CFString, CFType>::from_slices(&keys, &values);
    let lines: Vec<(CFRetained<CTLine>, f64)> = text
        .split('\n')
        .map(|part| {
            let part = part.strip_suffix('\r').unwrap_or(part);
            // SAFETY: a string and a dictionary of CoreText attribute keys, both copied.
            let line = unsafe {
                let string = CFAttributedString::new(None, Some(&CFString::from_str(part)), Some(attributes.as_opaque()))
                    .expect("attributed string");
                CTLine::with_attributed_string(&string)
            };
            // SAFETY: a live line; null out-pointers are allowed.
            let width = unsafe { line.typographic_bounds(std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
            (line, width)
        })
        .collect();
    // SAFETY: a live font's metrics.
    let (ascent, descent, leading) = unsafe { (font.ascent(), font.descent(), font.leading()) };
    let width = lines.iter().fold(0f64, |w, (_, lw)| w.max(*lw)) as f32;
    Layout { lines, ascent, line_height: (ascent + descent + leading).ceil(), width }
}

impl crate::platform::TextSystem for Text {
    type Format = TextStyle;
    type Layout = Layout;
    fn format(&mut self, style: &TextStyle) -> Option<TextStyle> {
        Some(style.clone())
    }
    fn layout(&mut self, text: &str, style: &TextStyle) -> Option<&Layout> {
        Some(self.shape(text, style, false))
    }
    fn measure(&mut self, text: &str, style: &TextStyle) -> (f32, f32) {
        self.measure(text, style)
    }
    fn clear_cache(&mut self) {
        self.clear_cache();
    }
}
