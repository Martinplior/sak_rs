use crate::font::FontFallbackList;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct LineLayoutMetrics {
    /// pixel offset of the left-most edge of the bitmap
    pub x_min: i32,
    /// pixel offset of the bottom-most edge of the bitmap
    pub y_min: i32,
    /// width of the bitmap
    pub width: u32,
    /// height of the bitmap
    pub height: u32,
    /// advance width of the character
    pub advance: f32,
}

pub trait LineLayoutLibrary {
    fn metrics(&self, ch: char, px: f32) -> LineLayoutMetrics;
}

/// `(0, 0)` is the left-most point of the baseline.
///
/// `y` is positive for downward direction.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CharLayout {
    /// corresponding character
    pub ch: char,
    /// x position of the left-most edge of the bitmap
    pub x: i32,
    /// y position of the top-most edge of the bitmap
    pub y: i32,
    /// width of the bitmap
    pub width: u32,
    /// height of the bitmap
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct LineLayout {
    font_size: f32,
    cursor_position: f32,
    bound: Option<Bound>,
    layout: Vec<CharLayout>,
}

#[derive(Debug, Clone)]
pub struct Bound {
    x_min: i32,
    x_max: i32,
    y_min: i32,
    y_max: i32,
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
            if ch.is_control() {
                return;
            }
            let metrics = library.metrics(ch, self.font_size);
            let current_x = self.cursor_position.round() as i32 + metrics.x_min;
            let current_x_max = current_x + metrics.width as i32;
            let current_y_min = -(metrics.height as i32 + metrics.y_min);
            let current_y_max = -metrics.y_min;
            if let Some(bound) = self.bound.as_mut() {
                bound.x_max = bound.x_max.max(current_x_max);
                bound.y_min = bound.y_min.min(current_y_min);
                bound.y_max = bound.y_max.max(current_y_max);
            } else {
                self.bound = Some(Bound {
                    x_min: metrics.x_min,
                    x_max: current_x_max,
                    y_min: current_y_min,
                    y_max: current_y_max,
                });
            }
            let (x, y) = (current_x, current_y_min);
            let (width, height) = (metrics.width, metrics.height);
            self.layout.push(CharLayout {
                ch,
                x,
                y,
                width,
                height,
            });
            self.cursor_position += metrics.advance;
        });
    }

    /// font size of the text.
    #[inline]
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// bounding box of the text.
    #[inline]
    pub fn bound(&self) -> Option<&Bound> {
        self.bound.as_ref()
    }

    /// x center position of the text.
    #[inline]
    pub fn x_center(&self) -> f32 {
        self.bound_map_or_default(|bound| (bound.x_min + bound.x_max) as f32 / 2.0)
    }

    /// y center position of the text.
    #[inline]
    pub fn y_center(&self) -> f32 {
        self.bound_map_or_default(|bound| (bound.y_max + bound.y_min) as f32 / 2.0)
    }

    /// center position of the text.
    ///
    /// returns `[x, y]`
    #[inline]
    pub fn center(&self) -> [f32; 2] {
        [self.x_center(), self.y_center()]
    }

    /// left-most edge of the text.
    #[inline]
    pub fn left(&self) -> i32 {
        self.bound_map_or_default(|bound| bound.x_min)
    }

    /// right-most edge of the text.
    #[inline]
    pub fn right(&self) -> i32 {
        self.bound_map_or_default(|bound| bound.x_max)
    }

    /// bottom-most edge of the text.
    #[inline]
    pub fn bottom(&self) -> i32 {
        self.bound_map_or_default(|bound| bound.y_max)
    }

    /// top-most edge of the text.
    #[inline]
    pub fn top(&self) -> i32 {
        self.bound_map_or_default(|bound| bound.y_min)
    }

    /// height of the text.
    #[inline]
    pub fn text_height(&self) -> u32 {
        self.bound_map_or_default(|bound| (bound.y_max - bound.y_min) as u32)
    }

    /// width of the text.
    #[inline]
    pub fn text_width(&self) -> u32 {
        self.bound_map_or_default(|bound| (bound.x_max - bound.x_min) as u32)
    }

    /// current cursor position of the text.
    ///
    /// Whitespace characters could have zero bitmap width, but advance width may not be zero.
    /// So [`Self::text_width`] may not include the space that the last whitespace character
    /// should occupy.
    #[inline]
    pub fn cursor_position(&self) -> f32 {
        self.cursor_position
    }

    #[inline]
    pub fn layout(&self) -> &[CharLayout] {
        &self.layout
    }

    #[inline]
    pub fn into_layout(self) -> Vec<CharLayout> {
        self.layout
    }
}

impl LineLayout {
    #[inline(always)]
    fn with_vec(font_size: f32, layout: Vec<CharLayout>) -> Self {
        Self {
            font_size,
            cursor_position: 0.0,
            bound: None,
            layout,
        }
    }

    #[inline(always)]
    fn bound_map_or_default<T: Default>(&self, f: impl FnOnce(&Bound) -> T) -> T {
        self.bound.as_ref().map_or(Default::default(), f)
    }
}

impl LineLayoutLibrary for fontdue::Font {
    fn metrics(&self, ch: char, px: f32) -> LineLayoutMetrics {
        let m = fontdue::Font::metrics(self, ch, px);
        LineLayoutMetrics {
            x_min: m.xmin,
            y_min: m.ymin,
            width: m.width as u32,
            height: m.height as u32,
            advance: m.advance_width,
        }
    }
}

impl LineLayoutLibrary for FontFallbackList {
    fn metrics(&self, ch: char, px: f32) -> LineLayoutMetrics {
        FontFallbackList::metrics(self, ch, px)
            .map(|m| LineLayoutMetrics {
                x_min: m.xmin,
                y_min: m.ymin,
                width: m.width as u32,
                height: m.height as u32,
                advance: m.advance_width,
            })
            .unwrap_or_default()
    }
}
