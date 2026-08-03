//! Android-only document I/O.
//!
//! On desktop every document the app touches — the markdown file, the files it
//! links to, the images it embeds — is reachable through `std::fs`. On Android
//! that is only true for the app's own private directories. Anything the user
//! opened lives behind the Storage Access Framework and can only be read
//! through a `ContentResolver`, addressed either by an explicit `content://`
//! URI or by a path relative to a granted folder tree.
//!
//! This module is the single place that knows how to turn one of the app's
//! paths into bytes, so the markdown reader (`app.rs`) and the image loader
//! (`images.rs`) resolve documents identically. Before it existed the image
//! loader called `std::fs::read` directly, which silently failed for every
//! image inside a granted tree — images simply never appeared.

#![cfg(target_os = "android")]

use std::path::Path;
use std::sync::RwLock;

/// The currently granted folder tree URI, if the user has opened a folder.
///
/// This mirrors `MdViewApp::android_tree_uri`. It lives here as well because
/// the image loader sits behind `MdRenderer` and has no route back to the app
/// struct, and threading a tree URI through every rendering call to reach one
/// leaf would be worse than a single process-wide cell for a single-window app.
static TREE_URI: RwLock<Option<String>> = RwLock::new(None);

/// Record the granted folder tree. Pass `None` to forget it.
pub fn set_tree_uri(uri: Option<String>) {
    if let Ok(mut guard) = TREE_URI.write() {
        *guard = uri;
    }
}

/// The currently granted folder tree URI, if any.
pub fn tree_uri() -> Option<String> {
    TREE_URI.read().ok().and_then(|g| g.clone())
}

/// Read a document addressed the way the rest of the app addresses documents.
///
/// The order of the cases matters:
///
/// 1. An explicit `content://` URI is read straight through the resolver.
/// 2. An absolute path that exists locally is read with `std::fs`. This covers
///    the app's private cache, where the single-file picker parks what it read.
///    It **must** be tried before the tree lookup: a cached absolute path is not
///    a tree-relative path, and running it through the tree resolver fails with
///    a confusing "not found in the opened folder".
/// 3. Anything else, with a folder granted, is a tree-relative path
///    (`sub/other.md`) and is resolved by walking the tree by display name.
///    This is what makes links between sibling markdown files work.
/// 4. Otherwise fall back to `std::fs` so relative paths still work in the
///    app's own working directory.
pub fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let path_str = path.to_string_lossy().to_string();

    if path_str.starts_with("content://") {
        return crate::android_shim::read_uri(&path_str)
            .ok_or_else(|| format!("Could not read content URI: {path_str}"));
    }

    if path.is_absolute() {
        if let Ok(bytes) = std::fs::read(path) {
            return Ok(bytes);
        }
    }

    if let Some(tree) = tree_uri() {
        let rel = crate::android_paths::normalize_rel(&path_str);
        return crate::android_shim::resolve_tree_path(&tree, &rel)
            .and_then(|uri| crate::android_shim::read_uri(&uri))
            .ok_or_else(|| format!("'{rel}' was not found in the opened folder"));
    }

    std::fs::read(path).map_err(|e| e.to_string())
}

/// Read a document as UTF-8 text. See [`read_bytes`] for how paths resolve.
pub fn read_to_string(path: &Path) -> Result<String, String> {
    let bytes = read_bytes(path)?;
    String::from_utf8(bytes).map_err(|e| format!("File is not valid UTF-8: {e}"))
}
