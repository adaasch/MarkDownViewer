//! Live-reload watcher for the open document.
//!
//! Android has no implementation: `notify` watches via inotify, which does not
//! work on the SAF-backed storage every Android document lives on, and the app
//! addresses documents by content URI rather than by a watchable path. Rather
//! than compile `notify` into the APK to have it fail at runtime, the Android
//! build gets a stub with the same signature whose constructor always fails —
//! callers already treat that as "no live reload", so nothing else changes.

#[cfg(not(target_os = "android"))]
use notify_debouncer_mini::notify::{self, RecommendedWatcher};
#[cfg(not(target_os = "android"))]
use notify_debouncer_mini::{DebounceEventResult, DebouncedEventKind, new_debouncer};
use std::path::{Path, PathBuf};
#[cfg(not(target_os = "android"))]
use std::sync::mpsc;
#[cfg(not(target_os = "android"))]
use std::time::Duration;

#[cfg(not(target_os = "android"))]
pub struct FileWatcher {
    _debouncer: notify_debouncer_mini::Debouncer<RecommendedWatcher>,
    receiver: mpsc::Receiver<PathBuf>,
}

#[cfg(not(target_os = "android"))]
impl FileWatcher {
    pub fn new(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let (tx, rx) = mpsc::channel();
        let sender = tx.clone();

        let mut debouncer = new_debouncer(
            Duration::from_millis(200),
            move |res: DebounceEventResult| {
                if let Ok(events) = res {
                    for event in events {
                        if event.kind == DebouncedEventKind::Any {
                            let _ = sender.send(event.path);
                        }
                    }
                }
            },
        )?;

        // IMPORTANT: Must `use notify::Watcher;` for the `.watch()` trait method to be available
        #[allow(unused_imports)]
        use notify::Watcher;
        debouncer
            .watcher()
            .watch(path, notify::RecursiveMode::NonRecursive)?;

        Ok(Self {
            _debouncer: debouncer,
            receiver: rx,
        })
    }

    pub fn try_recv(&self) -> Option<PathBuf> {
        self.receiver.try_recv().ok()
    }
}

/// Android stub — see the module docs. Construction always fails, so callers
/// end up with `None` and simply never get change notifications.
#[cfg(target_os = "android")]
pub struct FileWatcher {
    _private: (),
}

#[cfg(target_os = "android")]
impl FileWatcher {
    pub fn new(_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        Err("file watching is not supported on Android".into())
    }

    pub fn try_recv(&self) -> Option<PathBuf> {
        None
    }
}

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;
    use tempfile::NamedTempFile;

    #[test]
    fn test_watcher_creation() {
        let file = NamedTempFile::new().unwrap();
        let watcher = FileWatcher::new(file.path());
        assert!(watcher.is_ok());
    }

    #[test]
    fn test_watcher_detects_change() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();
        let watcher = FileWatcher::new(&path).unwrap();
        
        thread::sleep(Duration::from_millis(100));
        fs::write(&path, "modified content").unwrap();
        thread::sleep(Duration::from_millis(500));
        
        let changed = watcher.try_recv();
        assert!(changed.is_some());
    }
}
