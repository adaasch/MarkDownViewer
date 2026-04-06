use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum LinkAction {
    /// Navigate to another markdown file internally
    NavigateMarkdown(PathBuf),
    /// Navigate to a plain text file
    NavigateTextFile(PathBuf),
    /// Scroll to a heading anchor in the current document
    ScrollToAnchor(String),
    /// Open in system browser (http/https URLs, non-md files)
    OpenExternal(String),
}

const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown", "mdown", "mkd", "mkdn"];

const TEXT_EXTENSIONS: &[&str] = &["txt", "text", "log", "cfg", "conf", "ini", "toml", "yaml", "yml", "json", "xml", "csv", "sh", "bash", "zsh", "fish", "py", "rs", "js", "ts", "c", "h", "cpp", "hpp", "java", "go", "rb", "pl"];

#[allow(dead_code)]
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "svg", "webp", "bmp", "ico"];

/// Check if a URL points to an image file.
#[allow(dead_code)]
pub fn is_image_url(url: &str) -> bool {
    let path = url.split('?').next().unwrap_or(url);
    let lower = path.to_lowercase();
    IMAGE_EXTENSIONS.iter().any(|ext| lower.ends_with(&format!(".{ext}")))
}

/// Classify a link URL and determine what action to take.
/// `base_dir` is the directory of the current markdown file (for resolving relative paths).
pub fn classify_link(url: &str, base_dir: &Path) -> LinkAction {
    // Fragment-only link: scroll to anchor
    if let Some(anchor) = url.strip_prefix('#') {
        return LinkAction::ScrollToAnchor(anchor.to_string());
    }

    // External URLs go to the browser
    if url.starts_with("http://") || url.starts_with("https://") {
        return LinkAction::OpenExternal(url.to_string());
    }

    // mailto: and other schemes go to the browser
    if url.contains("://") || url.starts_with("mailto:") {
        return LinkAction::OpenExternal(url.to_string());
    }

    // Local path — strip fragment
    let (path_part, _fragment) = match url.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (url, None),
    };

    let resolved = if Path::new(path_part).is_absolute() {
        PathBuf::from(path_part)
    } else {
        base_dir.join(path_part)
    };

    // Check if it's a markdown file
    if let Some(ext) = resolved.extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        if MARKDOWN_EXTENSIONS.iter().any(|e| *e == ext_lower) {
            return LinkAction::NavigateMarkdown(resolved);
        }
        if TEXT_EXTENSIONS.iter().any(|e| *e == ext_lower) {
            return LinkAction::NavigateTextFile(resolved);
        }
    }

    // Everything else (including local images, PDFs, etc.) → open externally
    LinkAction::OpenExternal(url.to_string())
}

/// Generate an anchor ID from heading text (GitHub-style).
/// Lowercase, spaces→hyphens, strip non-alphanumeric (except hyphens).
pub fn heading_to_anchor(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c == ' ' || c == '-' {
                '-'
            } else {
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fragment_link() {
        let action = classify_link("#my-heading", Path::new("/docs"));
        assert_eq!(action, LinkAction::ScrollToAnchor("my-heading".to_string()));
    }

    #[test]
    fn test_http_link() {
        let action = classify_link("https://example.com", Path::new("/docs"));
        assert_eq!(action, LinkAction::OpenExternal("https://example.com".to_string()));
    }

    #[test]
    fn test_http_md_link_still_external() {
        let action = classify_link("https://github.com/README.md", Path::new("/docs"));
        assert_eq!(
            action,
            LinkAction::OpenExternal("https://github.com/README.md".to_string())
        );
    }

    #[test]
    fn test_relative_md_link() {
        let action = classify_link("other.md", Path::new("/docs"));
        assert_eq!(action, LinkAction::NavigateMarkdown(PathBuf::from("/docs/other.md")));
    }

    #[test]
    fn test_relative_md_link_in_subdir() {
        let action = classify_link("subdir/notes.md", Path::new("/docs"));
        assert_eq!(
            action,
            LinkAction::NavigateMarkdown(PathBuf::from("/docs/subdir/notes.md"))
        );
    }

    #[test]
    fn test_absolute_md_link() {
        let action = classify_link("/home/user/file.md", Path::new("/docs"));
        assert_eq!(
            action,
            LinkAction::NavigateMarkdown(PathBuf::from("/home/user/file.md"))
        );
    }

    #[test]
    fn test_markdown_extension_variants() {
        for ext in &["md", "markdown", "mdown", "mkd", "mkdn"] {
            let url = format!("file.{ext}");
            let action = classify_link(&url, Path::new("/docs"));
            assert!(
                matches!(action, LinkAction::NavigateMarkdown(_)),
                "Expected NavigateMarkdown for .{ext}"
            );
        }
    }

    #[test]
    fn test_text_extension_variants() {
        for ext in &["txt", "text", "log", "py", "rs", "js", "json", "toml"] {
            let url = format!("file.{ext}");
            let action = classify_link(&url, Path::new("/docs"));
            assert!(
                matches!(action, LinkAction::NavigateTextFile(_)),
                "Expected NavigateTextFile for .{ext}"
            );
        }
    }

    #[test]
    fn test_non_md_local_file_opens_external() {
        let action = classify_link("document.pdf", Path::new("/docs"));
        assert_eq!(action, LinkAction::OpenExternal("document.pdf".to_string()));
    }

    #[test]
    fn test_mailto_link() {
        let action = classify_link("mailto:user@example.com", Path::new("/docs"));
        assert_eq!(
            action,
            LinkAction::OpenExternal("mailto:user@example.com".to_string())
        );
    }

    #[test]
    fn test_is_image_url() {
        assert!(is_image_url("photo.png"));
        assert!(is_image_url("photo.JPG"));
        assert!(is_image_url("dir/image.webp"));
        assert!(is_image_url("https://example.com/img.gif?w=200"));
        assert!(!is_image_url("document.pdf"));
        assert!(!is_image_url("readme.md"));
    }

    #[test]
    fn test_heading_to_anchor() {
        assert_eq!(heading_to_anchor("Hello World"), "hello-world");
        assert_eq!(heading_to_anchor("My (Great) Heading!"), "my-great-heading");
        assert_eq!(heading_to_anchor("version 2.0"), "version-20");
        assert_eq!(heading_to_anchor("  spaces  "), "spaces");
    }
}
