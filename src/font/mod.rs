pub mod container;
pub mod layout;
pub mod loader;

pub use container::FontFallbackList;
pub use layout::LineLayout;
pub use loader::SystemFontsLoader;

/// print bitmap on console.
pub fn print_bitmap(bitmap: &[u8], width: usize) {
    if width == 0 {
        return;
    }
    debug_assert_eq!(bitmap.len() % width, 0);

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
    use fontdue::FontSettings;

    use super::*;

    #[test]
    fn test_print_bitmap() {
        let fonts_loader = SystemFontsLoader::new();
        let font_family_names = ["Segoe UI", "Segoe UI emoji"];
        let fonts: Vec<_> = font_family_names
            .into_iter()
            .map(|name| {
                let font_data = fonts_loader.load_by_family_name(name).unwrap();
                fontdue::Font::from_bytes(
                    font_data,
                    FontSettings {
                        scale: 16.0,
                        ..Default::default()
                    },
                )
                .unwrap()
            })
            .collect();
        let font_fallback_list = FontFallbackList::new(fonts).unwrap();
        // let text = "é a";
        let text = "🦌😡🤔 ";
        text.chars()
            .inspect(|&ch| {
                dbg!(font_fallback_list.horizontal_line_metrics(ch, 16.0));
            })
            .map(|ch| font_fallback_list.rasterize(ch, 16.0).unwrap_or_default())
            .for_each(|(metrics, bitmap)| {
                print_bitmap(&bitmap, metrics.width);
                dbg!(metrics);
            });
    }

    #[test]
    fn test_print_bitmap_0_to_255() {
        let bitmap: [u8; 256] = std::array::from_fn(|i| i as u8);
        print_bitmap(&bitmap, 16);
    }
}
