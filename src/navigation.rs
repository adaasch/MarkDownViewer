use eframe::egui::Vec2;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
struct NavigationEntry {
    path: PathBuf,
    scroll_offset: Vec2,
    is_plain_text: bool,
}

pub struct NavigationHistory {
    back_stack: Vec<NavigationEntry>,
    forward_stack: Vec<NavigationEntry>,
    current: NavigationEntry,
}

impl NavigationHistory {
    pub fn new(initial: PathBuf, is_plain_text: bool) -> Self {
        Self {
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            current: NavigationEntry {
                path: initial,
                scroll_offset: Vec2::ZERO,
                is_plain_text,
            },
        }
    }

    pub fn current(&self) -> &PathBuf {
        &self.current.path
    }

    pub fn current_scroll_offset(&self) -> Vec2 {
        self.current.scroll_offset
    }

    pub fn update_current_scroll_offset(&mut self, scroll_offset: Vec2) {
        self.current.scroll_offset = scroll_offset;
    }

    #[cfg(test)]
    pub fn current_is_plain_text(&self) -> bool {
        self.current.is_plain_text
    }

    pub fn navigate_to(&mut self, path: PathBuf, scroll_offset: Vec2, is_plain_text: bool) {
        self.back_stack.push(self.current.clone());
        self.current = NavigationEntry {
            path,
            scroll_offset,
            is_plain_text,
        };
        self.forward_stack.clear();
    }

    pub fn go_back(&mut self) -> Option<(&PathBuf, Vec2, bool)> {
        if let Some(prev) = self.back_stack.pop() {
            self.forward_stack.push(self.current.clone());
            self.current = prev;
            Some((
                &self.current.path,
                self.current.scroll_offset,
                self.current.is_plain_text,
            ))
        } else {
            None
        }
    }

    pub fn go_forward(&mut self) -> Option<(&PathBuf, Vec2, bool)> {
        if let Some(next) = self.forward_stack.pop() {
            self.back_stack.push(self.current.clone());
            self.current = next;
            Some((
                &self.current.path,
                self.current.scroll_offset,
                self.current.is_plain_text,
            ))
        } else {
            None
        }
    }

    pub fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::vec2;

    #[test]
    fn test_new_history_has_current() {
        let nav = NavigationHistory::new(PathBuf::from("/a.md"), false);
        assert_eq!(nav.current(), &PathBuf::from("/a.md"));
        assert_eq!(nav.current_scroll_offset(), Vec2::ZERO);
        assert!(!nav.current_is_plain_text());
        assert!(!nav.can_go_back());
        assert!(!nav.can_go_forward());
    }

    #[test]
    fn test_navigate_to_pushes_back() {
        let mut nav = NavigationHistory::new(PathBuf::from("/a.md"), false);
        nav.update_current_scroll_offset(vec2(0.0, 42.0));
        nav.navigate_to(PathBuf::from("/b.md"), Vec2::ZERO, false);
        assert_eq!(nav.current(), &PathBuf::from("/b.md"));
        assert_eq!(nav.current_scroll_offset(), Vec2::ZERO);
        assert!(!nav.current_is_plain_text());
        assert!(nav.can_go_back());
        assert!(!nav.can_go_forward());
    }

    #[test]
    fn test_go_back() {
        let mut nav = NavigationHistory::new(PathBuf::from("/a.md"), false);
        nav.update_current_scroll_offset(vec2(0.0, 75.0));
        nav.navigate_to(PathBuf::from("/b.py"), vec2(0.0, 10.0), true);
        let result = nav.go_back();
        assert_eq!(result, Some((&PathBuf::from("/a.md"), vec2(0.0, 75.0), false)));
        assert_eq!(nav.current(), &PathBuf::from("/a.md"));
        assert_eq!(nav.current_scroll_offset(), vec2(0.0, 75.0));
        assert!(!nav.current_is_plain_text());
        assert!(!nav.can_go_back());
        assert!(nav.can_go_forward());
    }

    #[test]
    fn test_go_forward() {
        let mut nav = NavigationHistory::new(PathBuf::from("/a.md"), false);
        nav.update_current_scroll_offset(vec2(0.0, 20.0));
        nav.navigate_to(PathBuf::from("/b.py"), vec2(0.0, 90.0), true);
        nav.go_back();
        let result = nav.go_forward();
        assert_eq!(result, Some((&PathBuf::from("/b.py"), vec2(0.0, 90.0), true)));
        assert_eq!(nav.current(), &PathBuf::from("/b.py"));
        assert_eq!(nav.current_scroll_offset(), vec2(0.0, 90.0));
        assert!(nav.current_is_plain_text());
        assert!(nav.can_go_back());
        assert!(!nav.can_go_forward());
    }

    #[test]
    fn test_navigate_clears_forward() {
        let mut nav = NavigationHistory::new(PathBuf::from("/a.md"), false);
        nav.navigate_to(PathBuf::from("/b.md"), Vec2::ZERO, false);
        nav.go_back();
        assert!(nav.can_go_forward());
        nav.navigate_to(PathBuf::from("/c.md"), Vec2::ZERO, false);
        assert!(!nav.can_go_forward());
    }

    #[test]
    fn test_go_back_on_empty_returns_none() {
        let mut nav = NavigationHistory::new(PathBuf::from("/a.md"), false);
        assert_eq!(nav.go_back(), None);
    }

    #[test]
    fn test_go_forward_on_empty_returns_none() {
        let mut nav = NavigationHistory::new(PathBuf::from("/a.md"), false);
        assert_eq!(nav.go_forward(), None);
    }

    #[test]
    fn test_multi_step_navigation() {
        let mut nav = NavigationHistory::new(PathBuf::from("/a.md"), false);
        nav.navigate_to(PathBuf::from("/b.md"), Vec2::ZERO, false);
        nav.navigate_to(PathBuf::from("/c.md"), Vec2::ZERO, false);
        nav.navigate_to(PathBuf::from("/d.md"), Vec2::ZERO, false);

        assert_eq!(nav.current(), &PathBuf::from("/d.md"));

        nav.go_back();
        assert_eq!(nav.current(), &PathBuf::from("/c.md"));

        nav.go_back();
        assert_eq!(nav.current(), &PathBuf::from("/b.md"));

        nav.go_forward();
        assert_eq!(nav.current(), &PathBuf::from("/c.md"));

        // Navigate to new page from middle — clears forward
        nav.navigate_to(PathBuf::from("/e.md"), Vec2::ZERO, false);
        assert!(!nav.can_go_forward());
        assert_eq!(nav.current(), &PathBuf::from("/e.md"));
    }
}
