//! Cached cosmic-text shaping, shared by measurement and CPU drawing.
pub use crate::text_style::{TextStyle, VARIABLE};
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight};
use std::{collections::HashMap, sync::Arc};
type Key = (String, String, [u32; 4], u16);
/// Per-thread font discovery and bounded shaped-layout cache.
pub struct Text {
    pub(crate) fonts: FontSystem,
    cache: HashMap<Key, Buffer>,
    capacity: usize,
    italic_loaded: bool,
    system_loaded: bool,
}
impl Text {
    /// Creates a new instance.
    pub fn new() -> anyhow::Result<Self> {
        let mut db = cosmic_text::fontdb::Database::new();
        // Fontconfig supplies the user's fallback faces; Inter guarantees a real 600 weight.
        if let Ok(out) = std::process::Command::new("fc-match")
            .args(["-f", "%{file}\n", "sans-serif"])
            .output()
        {
            for file in String::from_utf8_lossy(&out.stdout).lines() {
                let _ = db.load_font_file(file);
            }
        }
        db.load_font_source(cosmic_text::fontdb::Source::Binary(Arc::new(
            include_bytes!("../../../assets/Inter.ttf").as_slice(),
        )));

        db.set_sans_serif_family("Inter");
        Ok(Self {
            fonts: FontSystem::new_with_locale_and_db("en-US".into(), db),
            cache: HashMap::new(),
            capacity: 512,
            italic_loaded: false,
            system_loaded: false,
        })
    }
    pub(crate) fn shape(&mut self, text: &str, style: &TextStyle) -> &Buffer {
        self.shape_emphasis(text, style, false)
    }
    pub(crate) fn shape_emphasis(
        &mut self,
        text: &str,
        style: &TextStyle,
        italic: bool,
    ) -> &Buffer {
        if !self.system_loaded && &*style.family == "monospace" {
            self.fonts.db_mut().load_system_fonts();
            if let Ok(output) = std::process::Command::new("fc-match").args(["-f", "%{family}", "monospace"]).output()
                && output.status.success()
            {
                let name = String::from_utf8_lossy(&output.stdout);
                if let Some(name) = name.split(',').next().filter(|name| !name.trim().is_empty()) {
                    self.fonts.db_mut().set_monospace_family(name.trim());
                }
            }
            self.system_loaded = true;
        }
        if italic && !self.italic_loaded {
            self.fonts
                .db_mut()
                .load_font_source(cosmic_text::fontdb::Source::Binary(Arc::new(
                    include_bytes!("../../../assets/Inter-Italic.ttf").as_slice(),
                )));
            self.italic_loaded = true;
        }
        let key = (
            text.into(),
            style.family.to_string(),
            [
                style.size.to_bits(),
                style.tracking.to_bits(),
                style.optical.to_bits(),
                u32::from(italic),
            ],
            style.weight,
        );
        if !self.cache.contains_key(&key) {
            if self.cache.len() >= self.capacity {
                self.cache.clear();
            }
            let mut b = Buffer::new(
                &mut self.fonts,
                Metrics::new(style.size.max(1.), style.size.max(1.) * 1.25),
            );
            let family = if &*style.family == "monospace" {
                Family::Monospace
            } else if self.has_family(&style.family) {
                Family::Name(&style.family)
            } else {
                Family::SansSerif
            };
            b.set_text(
                text,
                &Attrs::new()
                    .family(family)
                    .weight(Weight(style.weight))
                    .style(if italic {
                        cosmic_text::Style::Italic
                    } else {
                        cosmic_text::Style::Normal
                    })
                    .letter_spacing(style.tracking / style.size.max(1.)),
                Shaping::Advanced,
                None,
            );
            b.shape_until_scroll(&mut self.fonts, false);
            self.cache.insert(key.clone(), b);
        }
        &self.cache[&key]
    }
    /// Measures the text in device pixels.
    pub fn measure(&mut self, text: &str, style: &TextStyle) -> (f32, f32) {
        if text.chars().count() == 1
            && text
                .chars()
                .next()
                .and_then(crate::icons::from_system_glyph)
                .is_some()
        {
            return (style.size, style.size);
        }
        let b = self.shape(text, style);
        b.layout_runs().fold((0f32, 0f32), |(w, h), r| {
            (w.max(r.line_w), h.max(r.line_top + r.line_height))
        })
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
    /// Has family.
    pub fn has_family(&self, name: &str) -> bool {
        self.fonts
            .db()
            .faces()
            .any(|f| f.families.iter().any(|(n, _)| n == name))
    }
    /// Families.
    pub fn families(&self) -> Vec<String> {
        let mut names: Vec<_> = self
            .fonts
            .db()
            .faces()
            .flat_map(|f| f.families.iter().map(|(n, _)| n.clone()))
            .collect();
        names.sort();
        names.dedup();
        names
    }
    /// Fit.
    pub fn fit(&mut self, text: &str, style: &TextStyle, width: f32) -> Arc<str> {
        if self.measure(text, style).0 <= width {
            return text.into();
        }
        let mut s = text.to_string();
        while !s.is_empty() {
            s.pop();
            let candidate = format!("{s}…");
            if self.measure(&candidate, style).0 <= width {
                return candidate.into();
            }
        }
        "".into()
    }
    /// Lines.
    pub fn lines(
        &mut self,
        text: &str,
        style: &TextStyle,
        width: f32,
        max: usize,
    ) -> Vec<Arc<str>> {
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in text.split_whitespace() {
            let candidate = if line.is_empty() {
                word.into()
            } else {
                format!("{line} {word}")
            };
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
impl crate::platform::TextSystem for Text {
    type Format = TextStyle;
    type Layout = Buffer;
    fn format(&mut self, style: &TextStyle) -> Option<TextStyle> {
        Some(style.clone())
    }
    fn layout(&mut self, text: &str, style: &TextStyle) -> Option<&Buffer> {
        Some(self.shape(text, style))
    }
    fn measure(&mut self, text: &str, style: &TextStyle) -> (f32, f32) {
        self.measure(text, style)
    }
    fn clear_cache(&mut self) {
        self.clear_cache();
    }
}
