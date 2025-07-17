use font_kit::{
    error::{FontLoadingError, SelectionError},
    source::SystemSource,
};

pub struct SystemFontsLoader {
    source: SystemSource,
}

impl Default for SystemFontsLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemFontsLoader {
    pub fn new() -> Self {
        Self {
            source: SystemSource::new(),
        }
    }

    pub fn all_family_names(&self) -> Result<Vec<String>, SelectionError> {
        self.source.all_families()
    }

    pub fn load_by_family_name(&self, family_name: &str) -> Result<Vec<u8>, LoadFontError> {
        let family_handle = self.source.select_family_by_name(family_name)?;
        let first_font_handle = family_handle.fonts().first().expect("unreachable");
        let font_data = first_font_handle
            .load()?
            .copy_font_data()
            .expect("unreachable");
        let data = owned_ttf_parser::OwnedFace::from_vec((*font_data).clone(), 0)?;
        Ok(data.into_vec())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoadFontError {
    #[error("{0}")]
    SelectionError(#[from] SelectionError),
    #[error("{0}")]
    FontLoadingError(#[from] FontLoadingError),
    #[error("{0}")]
    FaceParsingError(#[from] owned_ttf_parser::FaceParsingError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_font() {
        let loader = SystemFontsLoader::default();
        let font_data = loader.load_by_family_name("Arial").unwrap();
        assert_ne!(font_data.len(), 0);
    }

    #[test]
    fn test_load_font_not_found() {
        let loader = SystemFontsLoader::default();
        let result = loader.load_by_family_name("Not a font");
        println!("{result:?}");
        assert!(result.is_err());
    }
}
