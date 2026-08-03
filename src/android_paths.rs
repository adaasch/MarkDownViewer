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

/// Best-effort display name for a `content://` URI, used when the content
/// resolver declines to give us the real one.
///
/// A SAF document URI ends in a percent-encoded document ID, e.g.
/// `content://…/document/primary%3ADocs%2Fnotes.md`. Taking the raw last path
/// segment — which is what the app used to do — puts `primary%3ADocs%2Fnotes.md`
/// in the title bar. Decode the two separators SAF actually uses and return the
/// final component.
pub fn uri_display_name(uri: &str) -> String {
    let decoded = uri
        .split('?')
        .next()
        .unwrap_or(uri)
        .replace("%2F", "/")
        .replace("%2f", "/")
        .replace("%3A", ":")
        .replace("%3a", ":");
    decoded
        .rsplit(['/', ':'])
        .find(|s| !s.is_empty())
        .unwrap_or("(unknown)")
        .to_string()
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
    fn uri_display_name_decodes_saf_document_id() {
        assert_eq!(
            uri_display_name(
                "content://com.android.externalstorage.documents/document/primary%3ADocs%2Fnotes.md"
            ),
            "notes.md"
        );
        assert_eq!(
            uri_display_name(
                "content://com.android.externalstorage.documents/document/primary%3Areadme.md"
            ),
            "readme.md"
        );
    }

    #[test]
    fn uri_display_name_handles_plain_and_lowercase_encoding() {
        assert_eq!(uri_display_name("file:///sdcard/Docs/a.md"), "a.md");
        assert_eq!(uri_display_name("content://x/document/p%3aa%2fb.md"), "b.md");
        // A query string is not part of the name.
        assert_eq!(uri_display_name("content://x/doc/a.md?v=2"), "a.md");
    }

    #[test]
    fn uri_display_name_falls_back_when_empty() {
        assert_eq!(uri_display_name(""), "(unknown)");
        assert_eq!(uri_display_name("content://x/dir/"), "dir");
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
