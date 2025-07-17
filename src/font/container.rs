use fontdue::{Font, LineMetrics, Metrics};

#[derive(Debug, Clone)]
pub struct FontFallbackList {
    fonts: Vec<Font>,
}

impl FontFallbackList {
    /// Returns `None` if `fonts` is empty.
    pub fn new(fonts: Vec<Font>) -> Option<Self> {
        if fonts.is_empty() {
            return None;
        }
        Some(Self { fonts })
    }

    pub fn fonts(&self) -> &[Font] {
        &self.fonts
    }

    /// Returns `None` if `ch` is not present in any of the fonts.
    pub fn font(&self, ch: char) -> Option<&Font> {
        self.fonts.iter().find(|f| f.has_glyph(ch))
    }

    /// Returns `None` if `ch` is not present in any of the fonts.
    pub fn rasterize(&self, ch: char, px: f32) -> Option<(Metrics, Vec<u8>)> {
        self.font(ch).map(|f| f.rasterize(ch, px))
    }

    /// Returns `None` if `ch` is not present in any of the fonts.
    pub fn metrics(&self, ch: char, px: f32) -> Option<Metrics> {
        self.font(ch).map(|f| f.metrics(ch, px))
    }

    /// Returns `None` if `ch` is not present in any of the fonts or the font does not support
    /// vertical metrics.
    pub fn horizontal_line_metrics(&self, ch: char, px: f32) -> Option<LineMetrics> {
        self.font(ch)
            .map(|f| f.horizontal_line_metrics(px))
            .flatten()
    }
}
