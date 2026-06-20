//! Pure path helpers for the Android folder-tree navigation.
//!
//! These functions resolve links between markdown files that were opened from a
//! granted folder tree (Storage Access Framework). They contain no Android or
//! JNI dependencies, so they compile and are unit-tested on every platform even
//! though they are only *used* by the Android build (`src/app.rs`).

/// Normalize a tree-relative path: drop empty and `.` segments and resolve `..`
/// against the preceding segment.
///
/// Examples:
/// - `sub/../other.md` → `other.md`
/// - `./a/b.md` → `a/b.md`
/// - `/notes/x.md` → `notes/x.md` (a leading `/` becomes tree-root-relative)
/// - `a/b/../../c.md` → `c.md`
pub fn normalize_rel(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    out.join("/")
}

/// Whether a path looks like a markdown file (vs. plain text) by extension.
pub fn is_markdown_path(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(ext.as_str(), "md" | "markdown" | "mdown" | "mkd" | "mkdn")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_current_dir() {
        assert_eq!(normalize_rel("./a/b.md"), "a/b.md");
        assert_eq!(normalize_rel("a/./b.md"), "a/b.md");
    }

    #[test]
    fn normalize_resolves_parent() {
        assert_eq!(normalize_rel("sub/../other.md"), "other.md");
        assert_eq!(normalize_rel("a/b/../../c.md"), "c.md");
        assert_eq!(normalize_rel("a/b/../c.md"), "a/c.md");
    }

    #[test]
    fn normalize_strips_leading_slash() {
        assert_eq!(normalize_rel("/notes/x.md"), "notes/x.md");
    }

    #[test]
    fn normalize_collapses_empty_segments() {
        assert_eq!(normalize_rel("a//b.md"), "a/b.md");
        assert_eq!(normalize_rel(""), "");
        assert_eq!(normalize_rel("readme.md"), "readme.md");
    }

    #[test]
    fn normalize_parent_above_root_is_dropped() {
        // `..` with nothing to pop is simply ignored.
        assert_eq!(normalize_rel("../x.md"), "x.md");
        assert_eq!(normalize_rel("../../x.md"), "x.md");
    }

    #[test]
    fn markdown_extensions_recognized() {
        for p in ["a.md", "A.MARKDOWN", "dir/b.mdown", "c.mkd", "d.mkdn"] {
            assert!(is_markdown_path(p), "expected markdown: {p}");
        }
    }

    #[test]
    fn non_markdown_extensions_rejected() {
        for p in ["a.txt", "b.rs", "c.json", "noext", "d.png"] {
            assert!(!is_markdown_path(p), "expected non-markdown: {p}");
        }
    }
}
