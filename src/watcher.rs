use notify_debouncer_mini::notify::{self, RecommendedWatcher};
use notify_debouncer_mini::{DebounceEventResult, DebouncedEventKind, new_debouncer};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

pub struct FileWatcher {
    _debouncer: notify_debouncer_mini::Debouncer<RecommendedWatcher>,
    receiver: mpsc::Receiver<PathBuf>,
}

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

#[cfg(test)]
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
