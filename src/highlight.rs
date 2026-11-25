use std::collections::{HashMap, VecDeque};

use eframe::egui::text::LayoutJob;
use eframe::egui::{Color32, FontId, TextFormat};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Maximum number of highlighted code blocks kept in the cache.
///
/// Each entry holds a full `LayoutJob`, so an unbounded cache let a session
/// that walks through many documents (or one with many generated code blocks)
/// grow memory for as long as the app stayed open. 128 is far above what any
/// single document needs — its own blocks stay hot — and entries for
/// documents left behind are simply re-highlighted on demand if navigated
/// back to.
const HIGHLIGHT_CACHE_CAP: usize = 128;

type CacheKey = (u64, String, String);

pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    cache: HashMap<CacheKey, LayoutJob>,
    /// Recency order of the cache keys, back = most recently used. Drives the
    /// eviction in [`Self::evict_lru`].
    lru_order: VecDeque<CacheKey>,
}

impl Highlighter {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            cache: HashMap::new(),
            lru_order: VecDeque::new(),
        }
    }

    /// Clear the highlight cache (e.g., on theme change or file reload).
    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.lru_order.clear();
    }

    /// Highlight a code block and return an egui LayoutJob.
    /// `language` is the info string from the fenced code block (e.g. "rust", "python").
    /// `theme_name` is the syntect theme name (e.g. "InspiredGitHub").
    pub fn highlight(&mut self, code: &str, language: Option<&str>, theme_name: &str) -> LayoutJob {
        let lang_key = language.unwrap_or("").to_string();
        let cache_key = (hash_str(code), lang_key, theme_name.to_string());
        if let Some(cached) = self.cache.get(&cache_key) {
            let job = cached.clone();
            self.touch_lru(&cache_key);
            return job;
        }

        let job = self.build_layout_job(code, language, theme_name);
        self.cache.insert(cache_key.clone(), job.clone());
        self.lru_order.push_back(cache_key);
        self.evict_lru();
        job
    }

    /// Move `key` to the back of the recency order (most recently used).
    fn touch_lru(&mut self, key: &CacheKey) {
        if let Some(pos) = self.lru_order.iter().position(|k| k == key) {
            self.lru_order.remove(pos);
            self.lru_order.push_back(key.clone());
        }
    }

    /// Drop the least recently used entries until the cache is back at its cap.
    fn evict_lru(&mut self) {
        while self.cache.len() > HIGHLIGHT_CACHE_CAP {
            let Some(oldest) = self.lru_order.pop_front() else {
                break;
            };
            self.cache.remove(&oldest);
        }
    }

    fn build_layout_job(&self, code: &str, language: Option<&str>, theme_name: &str) -> LayoutJob {
        let mut job = LayoutJob::default();
        job.wrap.max_width = f32::INFINITY;

        let theme = match self.theme_set.themes.get(theme_name) {
            Some(t) => t,
            None => {
                // Fallback: render as plain text
                job.append(
                    code,
                    0.0,
                    TextFormat {
                        font_id: FontId::monospace(14.0),
                        color: Color32::GRAY,
                        ..Default::default()
                    },
                );
                return job;
            }
        };

        let syntax = language
            .and_then(|lang| self.syntax_set.find_syntax_by_token(lang))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut h = HighlightLines::new(syntax, theme);

        for line in LinesWithEndings::from(code) {
            let ranges = match h.highlight_line(line, &self.syntax_set) {
                Ok(r) => r,
                Err(_) => {
                    job.append(
                        line,
                        0.0,
                        TextFormat {
                            font_id: FontId::monospace(14.0),
                            color: Color32::GRAY,
                            ..Default::default()
                        },
                    );
                    continue;
                }
            };

            for (style, text) in ranges {
                let fg = Color32::from_rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                let font_style = style.font_style;
                let italics = font_style.contains(FontStyle::ITALIC);
                let underline_style = if font_style.contains(FontStyle::UNDERLINE) {
                    eframe::egui::Stroke::new(1.0, fg)
                } else {
                    eframe::egui::Stroke::NONE
                };

                job.append(
                    text,
                    0.0,
                    TextFormat {
                        font_id: FontId::monospace(14.0),
                        color: fg,
                        italics,
                        underline: underline_style,
                        ..Default::default()
                    },
                );
            }
        }

        job
    }
}

/// FNV-style 64-bit string hash, also used by the Android cache naming in
/// `app.rs` to keep distinct documents from sharing a cache filename.
pub(crate) fn hash_str(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_rust_produces_nonempty_job() {
        let mut h = Highlighter::new();
        let job = h.highlight("fn main() {}\n", Some("rust"), "base16-ocean.dark");
        assert!(!job.text.is_empty());
        assert!(job.sections.len() > 1, "Expected multiple colored sections for Rust code");
    }

    #[test]
    fn test_highlight_unknown_language_falls_back() {
        let mut h = Highlighter::new();
        let job = h.highlight("some code\n", Some("nonexistent_lang_xyz"), "base16-ocean.dark");
        assert!(!job.text.is_empty());
    }

    #[test]
    fn test_highlight_no_language() {
        let mut h = Highlighter::new();
        let job = h.highlight("just text\n", None, "base16-ocean.dark");
        assert_eq!(job.text, "just text\n");
    }

    #[test]
    fn test_highlight_caching() {
        let mut h = Highlighter::new();
        let job1 = h.highlight("let x = 1;\n", Some("rust"), "base16-ocean.dark");
        let job2 = h.highlight("let x = 1;\n", Some("rust"), "base16-ocean.dark");
        assert_eq!(job1.text, job2.text);
        assert_eq!(job1.sections.len(), job2.sections.len());
    }

    #[test]
    fn test_clear_cache() {
        let mut h = Highlighter::new();
        h.highlight("code\n", Some("rust"), "base16-ocean.dark");
        assert!(!h.cache.is_empty());
        h.clear_cache();
        assert!(h.cache.is_empty());
        assert!(h.lru_order.is_empty());
    }

    #[test]
    fn test_cache_is_bounded() {
        let mut h = Highlighter::new();
        for i in 0..(HIGHLIGHT_CACHE_CAP + 10) {
            h.highlight(&format!("block {i}\n"), Some("rust"), "base16-ocean.dark");
        }
        assert_eq!(
            h.cache.len(),
            HIGHLIGHT_CACHE_CAP,
            "cache should evict instead of growing past the cap"
        );
        assert_eq!(h.lru_order.len(), HIGHLIGHT_CACHE_CAP);
    }

    #[test]
    fn test_cache_evicts_least_recently_used() {
        let mut h = Highlighter::new();
        for i in 0..HIGHLIGHT_CACHE_CAP {
            h.highlight(&format!("block {i}\n"), Some("rust"), "base16-ocean.dark");
        }
        // Re-highlight the oldest entry, making it the most recently used.
        h.highlight("block 0\n", Some("rust"), "base16-ocean.dark");
        // One more insert must evict the new oldest ("block 1"), not the
        // freshly touched "block 0".
        h.highlight("new block\n", Some("rust"), "base16-ocean.dark");

        let key_of = |code: &str| {
            (hash_str(code), "rust".to_string(), "base16-ocean.dark".to_string())
        };
        assert!(h.cache.contains_key(&key_of("block 0\n")), "touched entry should survive");
        assert!(
            !h.cache.contains_key(&key_of("block 1\n")),
            "untouched oldest entry should be evicted"
        );
    }

    #[test]
    fn test_invalid_theme_falls_back() {
        let mut h = Highlighter::new();
        let job = h.highlight("code\n", Some("rust"), "nonexistent_theme");
        assert!(!job.text.is_empty());
    }
}
