use super::{Font, FontFallbackList};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct LineLayoutMetrics {
    pub ascent: f32,
    pub descent: f32,
    pub h_advance: f32,
    pub h_side_bearing: f32,
}

pub trait LineLayoutLibrary {
    /// returns `None` if the character is not supported by the library.
    fn metrics(&self, ch: char, px: f32) -> Option<LineLayoutMetrics>;
}

/// `(0, 0)` is the left-most point of the baseline.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct GlyphLineLayout {
    /// corresponding character
    pub ch: char,
    /// x position of the left-most edge of the bitmap
    pub x: i32,
}

/// `(0, baseline)` is the left-most point of the baseline.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct GlyphMultiLineLayout {
    /// corresponding character
    pub ch: char,
    /// x position of the left-most edge of the bitmap
    pub x: i32,
    /// y position of the baseline
    pub baseline: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bounds {
    x_min: f32,
    y_min: f32,
    x_max: f32,
    y_max: f32,
}

#[derive(Debug, Clone)]
pub struct LineLayout {
    font_size: f32,
    cursor: f32,
    bounds: Bounds,
    layout: Vec<GlyphLineLayout>,
}

impl LineLayout {
    pub fn new(font_size: f32) -> Self {
        Self::with_vec(font_size, Vec::new())
    }

    pub fn with_capacity(font_size: f32, capacity: usize) -> Self {
        Self::with_vec(font_size, Vec::with_capacity(capacity))
    }

    #[inline]
    pub fn clear(&mut self) {
        let old_self = core::mem::replace(self, LineLayout::new(self.font_size));
        self.layout = old_self.layout;
        self.layout.clear();
    }

    #[inline]
    pub fn reset(&mut self, font_size: f32) {
        self.font_size = font_size;
        self.clear();
    }

    pub fn append(&mut self, library: &impl LineLayoutLibrary, text: impl AsRef<str>) {
        let text = text.as_ref();
        self.layout.reserve(text.len()); // large enough to avoid reallocation
        text.chars().for_each(|ch| {
            let Some(LineLayoutMetrics {
                ascent,
                descent,
                h_advance,
                h_side_bearing,
            }) = library.metrics(ch, self.font_size)
            else {
                return;
            };
            let current_x = self.cursor + h_side_bearing;
            let next_cursor = self.cursor + h_advance;
            let current_y_min = -ascent;
            let current_y_max = -descent;
            self.bounds.x_min = self.bounds.x_min.min(current_x).min(next_cursor);
            self.bounds.x_max = self.bounds.x_max.max(current_x).max(next_cursor);
            self.bounds.y_min = self.bounds.y_min.min(current_y_min);
            self.bounds.y_max = self.bounds.y_max.max(current_y_max);

            let x = current_x.round() as i32;
            self.layout.push(GlyphLineLayout { ch, x });

            self.cursor = next_cursor;
        });
    }

    /// font size of the text.
    #[inline]
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// bounding box of the text.
    #[inline]
    pub fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    /// x center position of the text.
    #[inline]
    pub fn center_x(&self) -> f32 {
        (self.bounds.x_min + self.bounds.x_max) / 2.0
    }

    /// y center position of the text.
    #[inline]
    pub fn center_y(&self) -> f32 {
        (self.bounds.y_max + self.bounds.y_min) / 2.0
    }

    /// center position of the text.
    ///
    /// returns `[x, y]`
    #[inline]
    pub fn center(&self) -> [f32; 2] {
        [self.center_x(), self.center_y()]
    }

    /// left-most edge of the text.
    #[inline]
    pub fn left(&self) -> f32 {
        self.bounds.x_min
    }

    /// right-most edge of the text.
    #[inline]
    pub fn right(&self) -> f32 {
        self.bounds.x_max
    }

    /// bottom-most edge of the text.
    #[inline]
    pub fn bottom(&self) -> f32 {
        self.bounds.y_max
    }

    /// top-most edge of the text.
    #[inline]
    pub fn top(&self) -> f32 {
        self.bounds.y_min
    }

    /// height of the text.
    #[inline]
    pub fn text_height(&self) -> u32 {
        (self.bounds.y_max - self.bounds.y_min) as u32
    }

    /// width of the text.
    #[inline]
    pub fn text_width(&self) -> u32 {
        (self.bounds.x_max - self.bounds.x_min) as u32
    }

    /// current cursor position of the text.
    ///
    /// Whitespace characters could have zero bitmap width, but advance width may not be zero.
    /// So [`Self::text_width`] may not include the space that the last whitespace character
    /// should occupy.
    #[inline]
    pub fn cursor_position(&self) -> f32 {
        self.cursor
    }

    #[inline]
    pub fn layout(&self) -> &[GlyphLineLayout] {
        &self.layout
    }

    #[inline]
    pub fn into_layout(self) -> Vec<GlyphLineLayout> {
        self.layout
    }
}

impl LineLayout {
    #[inline(always)]
    fn with_vec(font_size: f32, layout: Vec<GlyphLineLayout>) -> Self {
        Self {
            font_size,
            cursor: 0.0,
            bounds: Default::default(),
            layout,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MultiLineLayout {
    font_size: f32,
    line_gap: f32,
    cursor: [f32; 2],
    bounds: Bounds,
    layout: Vec<Vec<GlyphMultiLineLayout>>,
}

impl MultiLineLayout {
    pub fn new(font_size: f32, line_gap: f32) -> Self {
        Self {
            font_size,
            line_gap,
            cursor: [0.0; 2],
            bounds: Default::default(),
            layout: Vec::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        let old_self =
            core::mem::replace(self, MultiLineLayout::new(self.font_size, self.line_gap));
        self.layout = old_self.layout;
        self.layout.clear();
    }

    #[inline]
    pub fn reset(&mut self, font_size: f32, line_gap: f32) {
        self.font_size = font_size;
        self.line_gap = line_gap;
        self.clear();
    }

    pub fn append(&mut self, library: &impl LineLayoutLibrary, text: impl AsRef<str>) {
        let text = text.as_ref();
        text.lines().for_each(|line| {
            self.append_one_line(library, line);
            self.cursor[0] = 0.0;
            self.cursor[1] += self.font_size + self.line_gap;
        });
    }

    /// font size of the text.
    #[inline]
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// line gap of the text.
    #[inline]
    pub fn line_gap(&self) -> f32 {
        self.line_gap
    }

    /// bounding box of the text.
    #[inline]
    pub fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    /// x center position of the text.
    #[inline]
    pub fn center_x(&self) -> f32 {
        (self.bounds.x_min + self.bounds.x_max) / 2.0
    }

    /// y center position of the text.
    #[inline]
    pub fn center_y(&self) -> f32 {
        (self.bounds.y_max + self.bounds.y_min) / 2.0
    }

    /// center position of the text.
    ///
    /// returns `[x, y]`
    #[inline]
    pub fn center(&self) -> [f32; 2] {
        [self.center_x(), self.center_y()]
    }

    /// left-most edge of the text.
    #[inline]
    pub fn left(&self) -> f32 {
        self.bounds.x_min
    }

    /// right-most edge of the text.
    #[inline]
    pub fn right(&self) -> f32 {
        self.bounds.x_max
    }

    /// bottom-most edge of the text.
    #[inline]
    pub fn bottom(&self) -> f32 {
        self.bounds.y_max
    }

    /// top-most edge of the text.
    #[inline]
    pub fn top(&self) -> f32 {
        self.bounds.y_min
    }

    /// height of the text.
    #[inline]
    pub fn text_height(&self) -> u32 {
        (self.bounds.y_max - self.bounds.y_min) as u32
    }

    /// width of the text.
    #[inline]
    pub fn text_width(&self) -> u32 {
        (self.bounds.x_max - self.bounds.x_min) as u32
    }

    /// current cursor position of the text.
    ///
    /// Whitespace characters could have zero bitmap width, but advance width may not be zero.
    /// So [`Self::text_width`] may not include the space that the last whitespace character
    /// should occupy.
    #[inline]
    pub fn cursor_position(&self) -> [f32; 2] {
        self.cursor
    }

    #[inline]
    pub fn layout(&self) -> &[Vec<GlyphMultiLineLayout>] {
        &self.layout
    }

    #[inline]
    pub fn into_layout(self) -> Vec<Vec<GlyphMultiLineLayout>> {
        self.layout
    }
}

impl MultiLineLayout {
    fn append_one_line(&mut self, library: &impl LineLayoutLibrary, line: &str) {
        let text_size = line.len();
        let line_layout = if let Some(line_layout) = self.layout.last_mut() {
            line_layout.reserve(text_size);
            line_layout
        } else {
            self.layout.push(Vec::with_capacity(text_size));
            unsafe { self.layout.last_mut().unwrap_unchecked() }
        };
        line.chars().for_each(|ch| {
            let Some(LineLayoutMetrics {
                ascent,
                descent,
                h_advance,
                h_side_bearing,
            }) = library.metrics(ch, self.font_size)
            else {
                return;
            };
            let current_x = self.cursor[0] + h_side_bearing;
            let next_cursor = self.cursor[0] + h_advance;
            let current_y_min = self.cursor[1] - ascent;
            let current_y_max = self.cursor[1] - descent;
            self.bounds.x_min = self.bounds.x_min.min(current_x).min(next_cursor);
            self.bounds.x_max = self.bounds.x_max.max(current_x).max(next_cursor);
            self.bounds.y_min = self.bounds.y_min.min(current_y_min);
            self.bounds.y_max = self.bounds.y_max.max(current_y_max);

            let x = current_x.round() as i32;
            let baseline = self.cursor[1].round() as i32;
            line_layout.push(GlyphMultiLineLayout { ch, x, baseline });

            self.cursor[0] = next_cursor;
        });
    }
}

impl LineLayoutLibrary for Font {
    fn metrics(&self, ch: char, px: f32) -> Option<LineLayoutMetrics> {
        if !self.has_glyph(ch) {
            return None;
        }
        Some(LineLayoutMetrics {
            ascent: self.ascent(px),
            descent: self.descent(px),
            h_advance: self.h_advance(ch, px),
            h_side_bearing: self.h_side_bearing(ch, px),
        })
    }
}

impl LineLayoutLibrary for FontFallbackList {
    fn metrics(&self, ch: char, px: f32) -> Option<LineLayoutMetrics> {
        self.font(ch).map(|font| LineLayoutMetrics {
            ascent: font.ascent(px),
            descent: font.descent(px),
            h_advance: font.h_advance(ch, px),
            h_side_bearing: font.h_side_bearing(ch, px),
        })
    }
}
