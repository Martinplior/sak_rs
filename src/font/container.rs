use ab_glyph::{Font as _, FontVec, InvalidFont, PxScaleFont, ScaleFont};

#[derive(Debug, Clone)]
pub struct FontFallbackList {
    fonts: Box<[Font]>,
}

impl FontFallbackList {
    pub fn new(fonts: Box<[Font]>) -> Self {
        Self { fonts }
    }

    pub fn fonts(&self) -> &[Font] {
        &self.fonts
    }

    pub fn fonts_mut(&mut self) -> &mut [Font] {
        &mut self.fonts
    }

    /// Returns `None` if `ch` is not present in any of the fonts.
    pub fn font(&self, ch: char) -> Option<&Font> {
        self.fonts.iter().find(|f| f.has_glyph(ch))
    }
}

#[derive(Debug, Default)]
pub struct GlyphBitmap {
    pub bitmap: Box<[u8]>,
    pub metrics: GlyphMetrics,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct GlyphMetrics {
    /// The y offset from the baseline to the top of the bitmap.
    pub y_offset: i32,
    pub width: u32,
    pub height: u32,
}

/// metrics == px / height_unscaled * metrics_unscaled
#[derive(Debug)]
pub struct Font {
    font: FontVec,
}

impl Font {
    pub fn try_from_vec(data: Vec<u8>) -> Result<Self, InvalidFont> {
        let font = FontVec::try_from_vec(data)?;

        Ok(Self { font })
    }

    /// expensive operation, you'd better cache the result if you need it frequently
    pub fn rasterize(&self, ch: char, px: f32) -> Option<GlyphBitmap> {
        let glyph = self.font.glyph_id(ch).with_scale(px);
        let outline_glyph = self.font.outline_glyph(glyph)?;
        let px_bounds = outline_glyph.px_bounds();
        let y_offset = px_bounds.min.y as i32;
        let (width, height) = (px_bounds.width() as u32, px_bounds.height() as u32);
        let size = width * height;
        let mut bitmap = vec![0; size as usize].into_boxed_slice();
        outline_glyph.draw(|x, y, a| {
            let index = (y * width + x) as usize;
            *unsafe { bitmap.get_mut(index).unwrap_unchecked() } = (a * 255.0) as u8;
        });
        Some(GlyphBitmap {
            bitmap,
            metrics: GlyphMetrics {
                y_offset,
                width,
                height,
            },
        })
    }

    /// expensive operation, you'd better cache the result if you need it frequently
    pub fn outline(&self, ch: char) -> Option<ab_glyph::Outline> {
        self.font.outline(self.font.glyph_id(ch))
    }

    #[inline]
    pub fn glyph_metrics(&self, outline_bounds_unscaled: &ab_glyph::Rect, px: f32) -> GlyphMetrics {
        let ab_glyph::Rect { min, max } = outline_bounds_unscaled;
        let scale_factor = self.as_scaled(px).scale_factor();
        let x_min = (min.x * scale_factor.horizontal).floor() as i32;
        let x_max = (max.x * scale_factor.horizontal).ceil() as i32;
        let y_min = (min.y * -scale_factor.vertical).floor() as i32;
        let y_max = (max.y * -scale_factor.vertical).ceil() as i32;

        GlyphMetrics {
            y_offset: y_min,
            width: (x_max - x_min) as u32,
            height: (y_max - y_min) as u32,
        }
    }

    #[inline]
    pub fn has_glyph(&self, ch: char) -> bool {
        self.font.glyph_id(ch).0 != 0
    }

    #[inline]
    pub fn ascent_unscaled(&self) -> f32 {
        self.font.ascent_unscaled()
    }

    #[inline]
    pub fn descent_unscaled(&self) -> f32 {
        self.font.descent_unscaled()
    }

    #[inline]
    pub fn height_unscaled(&self) -> f32 {
        self.font.height_unscaled()
    }

    #[inline]
    pub fn line_gap_unscaled(&self) -> f32 {
        self.font.line_gap_unscaled()
    }

    #[inline]
    pub fn h_advance_unscaled(&self, ch: char) -> f32 {
        self.font.h_advance_unscaled(self.font.glyph_id(ch))
    }

    #[inline]
    pub fn h_side_bearing_unscaled(&self, ch: char) -> f32 {
        self.font.h_side_bearing_unscaled(self.font.glyph_id(ch))
    }

    #[inline]
    pub fn v_advance_unscaled(&self, ch: char) -> f32 {
        self.font.v_advance_unscaled(self.font.glyph_id(ch))
    }

    #[inline]
    pub fn v_side_bearing_unscaled(&self, ch: char) -> f32 {
        self.font.v_side_bearing_unscaled(self.font.glyph_id(ch))
    }

    #[inline]
    pub fn ascent(&self, px: f32) -> f32 {
        self.as_scaled(px).ascent()
    }

    #[inline]
    pub fn descent(&self, px: f32) -> f32 {
        self.as_scaled(px).descent()
    }

    #[inline]
    pub fn height(&self, px: f32) -> f32 {
        self.as_scaled(px).height()
    }

    #[inline]
    pub fn line_gap(&self, px: f32) -> f32 {
        self.as_scaled(px).line_gap()
    }

    #[inline]
    pub fn h_advance(&self, ch: char, px: f32) -> f32 {
        self.as_scaled(px).h_advance(self.font.glyph_id(ch))
    }

    #[inline]
    pub fn h_side_bearing(&self, ch: char, px: f32) -> f32 {
        self.as_scaled(px).h_side_bearing(self.font.glyph_id(ch))
    }

    #[inline]
    pub fn v_advance(&self, ch: char, px: f32) -> f32 {
        self.as_scaled(px).v_advance(self.font.glyph_id(ch))
    }

    #[inline]
    pub fn v_side_bearing(&self, ch: char, px: f32) -> f32 {
        self.as_scaled(px).v_side_bearing(self.font.glyph_id(ch))
    }
}

impl Font {
    #[inline(always)]
    fn as_scaled(&self, px: f32) -> PxScaleFont<&FontVec> {
        self.font.as_scaled(px)
    }
}

impl Clone for Font {
    fn clone(&self) -> Self {
        let data = self.font.font_data().to_vec();
        // SAFETY: data is from existing FontVec
        let font = unsafe { FontVec::try_from_vec(data).unwrap_unchecked() };
        Self { font }
    }
}

#[cfg(test)]
mod tests {
    use crate::font::SystemFontsLoader;

    use super::*;

    #[test]
    fn test_font() {
        let loader = SystemFontsLoader::new();
        let font_data = loader.load_by_family_name("Sarasa Fixed SC").unwrap();
        let font = Font::try_from_vec(font_data).unwrap();
        let text = "é ag";
        let px = 16.0;
        text.chars().for_each(|ch| {
            let bitmap = font.rasterize(ch, px).unwrap_or_default();
            crate::font::print_bitmap(&bitmap.bitmap, bitmap.metrics.width as usize);
            dbg!(bitmap.metrics);
            dbg!(font.ascent(px));
            dbg!(font.descent(px));
            dbg!(font.height(px));
            dbg!(font.line_gap(px));
            dbg!(font.h_advance(ch, px));
            dbg!(font.h_side_bearing(ch, px));
            dbg!(font.v_advance(ch, px));
            dbg!(font.v_side_bearing(ch, px));
            assert!(px / font.height_unscaled() * font.ascent_unscaled() == font.ascent(px));
        });
    }
}
