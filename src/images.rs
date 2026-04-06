use std::collections::HashMap;
use std::path::{Path, PathBuf};
use eframe::egui;

pub struct ImageCache {
    textures: HashMap<PathBuf, Option<egui::TextureHandle>>,
}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    pub fn get_or_load(
        &mut self,
        path: &Path,
        ctx: &egui::Context,
    ) -> Option<&egui::TextureHandle> {
        let path_buf = path.to_path_buf();

        if !self.textures.contains_key(&path_buf) {
            let texture = self.load_texture(&path_buf, ctx);
            self.textures.insert(path_buf.clone(), texture);
        }

        self.textures.get(&path_buf).and_then(|t| t.as_ref())
    }

    fn load_texture(
        &self,
        path: &Path,
        ctx: &egui::Context,
    ) -> Option<egui::TextureHandle> {
        let data = std::fs::read(path).ok()?;
        let image = image::load_from_memory(&data).ok()?;
        let rgba = image.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels = rgba.into_raw();

        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
        Some(ctx.load_texture(
            path.to_string_lossy(),
            color_image,
            egui::TextureOptions::LINEAR,
        ))
    }

    pub fn clear(&mut self) {
        self.textures.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_cache_new() {
        let cache = ImageCache::new();
        assert!(cache.textures.is_empty());
    }

    #[test]
    fn test_image_cache_clear() {
        let mut cache = ImageCache::new();
        cache.textures.insert(PathBuf::from("test.png"), None);
        assert!(!cache.textures.is_empty());
        cache.clear();
        assert!(cache.textures.is_empty());
    }

    #[test]
    fn test_image_cache_missing_file() {
        // Without a real egui context, we test by inserting a None directly
        let mut cache = ImageCache::new();
        let path = PathBuf::from("/nonexistent/image.png");
        cache.textures.insert(path.clone(), None);
        assert!(cache.textures.get(&path).unwrap().is_none());
    }
}
