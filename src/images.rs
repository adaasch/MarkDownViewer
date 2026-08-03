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
        let data = read_image_bytes(path)?;
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

/// Read an image's raw bytes.
///
/// On Android an image referenced from a markdown file that was opened through
/// the Storage Access Framework is not reachable with `std::fs` — the path is
/// relative to a granted folder tree, not to the filesystem. It has to be
/// resolved exactly like the markdown file that references it, otherwise every
/// image in the document renders as its `[Image: alt]` placeholder.
#[cfg(target_os = "android")]
fn read_image_bytes(path: &Path) -> Option<Vec<u8>> {
    crate::android_io::read_bytes(path).ok()
}

#[cfg(not(target_os = "android"))]
fn read_image_bytes(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
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
