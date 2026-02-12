pub mod container;
pub mod layout;
pub mod loader;
pub mod sdf;

pub use container::{Font, FontFallbackList, GlyphBitmap, GlyphMetrics};
pub use layout::{GlyphLineLayout, GlyphMultiLineLayout, LineLayout, MultiLineLayout};
pub use loader::SystemFontsLoader;
pub use sdf::{Sdf, SdfGenerator};

/// print bitmap on console.
pub fn print_bitmap(bitmap: &[u8], width: usize) {
    if width == 0 {
        return;
    }
    debug_assert!(bitmap.len().is_multiple_of(width));

    use std::fmt::Write as _;

    fn write_pixel(buf: &mut String, value: u8) {
        write!(buf, "{}██", format_args!("\x1b[38;2;{0};{0};{0}m", value)).expect("unreachable");
    }
    let mut buf = String::new();
    bitmap.chunks_exact(width).for_each(|line| {
        line.iter().for_each(|&value| write_pixel(&mut buf, value));
        writeln!(&mut buf, "\x1b[0m").expect("unreachable");
    });
    print!("{buf}");
}

#[cfg(test)]
mod tests {

    use crate::font::layout::LineLayoutLibrary;

    use super::*;

    #[test]
    fn test_print_bitmap() {
        let fonts_loader = SystemFontsLoader::new();
        let font_family_names = ["Segoe UI", "Segoe UI emoji"];
        let fonts = font_family_names
            .into_iter()
            .map(|name| {
                let data = fonts_loader.load_by_family_name(name).unwrap();
                Font::try_from_vec(data).unwrap()
            })
            .collect();
        let font_fallback_list = FontFallbackList::new(fonts);
        // let text = "é a";
        let text = "🦌😡🤔abc ";
        let font_size = 64.0;
        text.chars()
            .inspect(|&ch| {
                dbg!(font_fallback_list.metrics(ch, font_size));
            })
            .map(|ch| {
                font_fallback_list
                    .font(ch)
                    .and_then(|f| f.rasterize(ch, font_size))
                    .unwrap_or_default()
            })
            .for_each(|glyph_bitmap| {
                print_bitmap(&glyph_bitmap.bitmap, glyph_bitmap.metrics.width as usize);
            });
    }

    #[test]
    fn test_print_sdf_bitmap() {
        let fonts_loader = SystemFontsLoader::new();
        let font_family_names = ["Segoe UI", "Segoe UI emoji"];
        let fonts = font_family_names
            .into_iter()
            .map(|name| {
                let data = fonts_loader.load_by_family_name(name).unwrap();
                Font::try_from_vec(data).unwrap()
            })
            .collect();
        let font_fallback_list = FontFallbackList::new(fonts);
        // let text = "é a";
        let text = "🦌😡🤔abc ";

        let font_size: f32 = 64.0;
        let radius = font_size / 2.0;
        let cutoff = 0.5;
        let edge_padding = (radius * (1.0 - cutoff)).ceil() as u32;
        let mut sdf_generator = SdfGenerator::new(edge_padding, radius, cutoff);

        text.chars()
            .inspect(|&ch| {
                dbg!(font_fallback_list.metrics(ch, font_size));
            })
            .map(|ch| {
                font_fallback_list
                    .font(ch)
                    .and_then(|f| f.rasterize(ch, font_size))
                    .unwrap_or_default()
            })
            .for_each(|glyph_bitmap| {
                let sdf = sdf_generator.generate(&glyph_bitmap.bitmap, glyph_bitmap.metrics.width);
                print_bitmap(&sdf.bitmap, sdf.width as _);
                println!("width: {}, height: {}", sdf.width, sdf.height);
            });
    }

    #[test]
    fn test_print_bitmap_0_to_255() {
        let bitmap: [u8; 256] = std::array::from_fn(|i| i as u8);
        print_bitmap(&bitmap, 16);
    }
}
