use eframe::egui::{Color32, Visuals};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Theme {
    Light,
    Dark,
}

#[allow(dead_code)]
impl Theme {
    pub fn toggle(&self) -> Self {
        match self {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        }
    }

    pub fn visuals(&self) -> Visuals {
        match self {
            Theme::Light => Visuals::light(),
            Theme::Dark => Visuals::dark(),
        }
    }

    pub fn syntect_theme_name(&self) -> &'static str {
        match self {
            Theme::Light => "InspiredGitHub",
            Theme::Dark => "base16-ocean.dark",
        }
    }

    pub fn code_bg(&self) -> Color32 {
        match self {
            Theme::Light => Color32::from_rgb(246, 248, 250),
            Theme::Dark => Color32::from_rgb(30, 30, 46),
        }
    }

    pub fn link_color(&self) -> Color32 {
        match self {
            Theme::Light => Color32::from_rgb(9, 105, 218),
            Theme::Dark => Color32::from_rgb(88, 166, 255),
        }
    }

    pub fn blockquote_border(&self) -> Color32 {
        match self {
            Theme::Light => Color32::from_rgb(208, 215, 222),
            Theme::Dark => Color32::from_rgb(48, 54, 61),
        }
    }

    pub fn blockquote_bg(&self) -> Color32 {
        match self {
            Theme::Light => Color32::from_rgb(246, 248, 250),
            Theme::Dark => Color32::from_rgb(22, 27, 34),
        }
    }

    pub fn heading_size(&self, level: u8) -> f32 {
        match level {
            1 => 32.0,
            2 => 26.0,
            3 => 22.0,
            4 => 18.0,
            5 => 16.0,
            _ => 14.0,
        }
    }

    pub fn icon_label(&self) -> &'static str {
        match self {
            Theme::Light => "🌙 Dark",
            Theme::Dark => "☀ Light",
        }
    }

    pub fn text_color(&self) -> Color32 {
        match self {
            Theme::Light => Color32::from_gray(60),
            Theme::Dark => Color32::from_gray(190),
        }
    }

    pub fn strong_text_color(&self) -> Color32 {
        match self {
            Theme::Light => Color32::BLACK,
            Theme::Dark => Color32::WHITE,
        }
    }

    pub fn table_border(&self) -> Color32 {
        match self {
            Theme::Light => Color32::from_gray(200),
            Theme::Dark => Color32::from_gray(60),
        }
    }

    pub fn table_stripe_bg(&self) -> Color32 {
        match self {
            Theme::Light => Color32::from_rgb(236, 240, 244),
            Theme::Dark => Color32::from_rgb(38, 44, 54),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Dark
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toggle() {
        assert_eq!(Theme::Light.toggle(), Theme::Dark);
        assert_eq!(Theme::Dark.toggle(), Theme::Light);
    }

    #[test]
    fn test_default_is_dark() {
        assert_eq!(Theme::default(), Theme::Dark);
    }

    #[test]
    fn test_heading_sizes_decrease() {
        let theme = Theme::Light;
        assert!(theme.heading_size(1) > theme.heading_size(2));
        assert!(theme.heading_size(2) > theme.heading_size(3));
        assert!(theme.heading_size(3) > theme.heading_size(4));
    }

    #[test]
    fn test_syntect_theme_names() {
        assert_eq!(Theme::Light.syntect_theme_name(), "InspiredGitHub");
        assert_eq!(Theme::Dark.syntect_theme_name(), "base16-ocean.dark");
    }

    #[test]
    fn test_code_bg_returns_color() {
        let light_bg = Theme::Light.code_bg();
        let dark_bg = Theme::Dark.code_bg();
        assert!(light_bg.a() > 0, "Light code_bg should be non-transparent");
        assert!(dark_bg.a() > 0, "Dark code_bg should be non-transparent");
    }

    #[test]
    fn test_link_color_returns_color() {
        let light = Theme::Light.link_color();
        let dark = Theme::Dark.link_color();
        assert!(light.a() > 0, "Light link_color should be non-transparent");
        assert!(dark.a() > 0, "Dark link_color should be non-transparent");
    }

    #[test]
    fn test_blockquote_border_returns_color() {
        let light = Theme::Light.blockquote_border();
        let dark = Theme::Dark.blockquote_border();
        assert!(light.a() > 0, "Light blockquote_border should be non-transparent");
        assert!(dark.a() > 0, "Dark blockquote_border should be non-transparent");
    }

    #[test]
    fn test_blockquote_bg_returns_color() {
        let light = Theme::Light.blockquote_bg();
        let dark = Theme::Dark.blockquote_bg();
        assert!(light.a() > 0, "Light blockquote_bg should be non-transparent");
        assert!(dark.a() > 0, "Dark blockquote_bg should be non-transparent");
    }

    #[test]
    fn test_text_color_light() {
        let color = Theme::Light.text_color();
        assert!(color.r() < 128, "Light theme text should have dark R component");
        assert!(color.g() < 128, "Light theme text should have dark G component");
        assert!(color.b() < 128, "Light theme text should have dark B component");
    }

    #[test]
    fn test_text_color_dark() {
        let color = Theme::Dark.text_color();
        assert!(color.r() > 128, "Dark theme text should have light R component");
        assert!(color.g() > 128, "Dark theme text should have light G component");
        assert!(color.b() > 128, "Dark theme text should have light B component");
    }

    #[test]
    fn test_strong_text_color_differs_from_text() {
        for theme in [Theme::Light, Theme::Dark] {
            let text = theme.text_color();
            let strong = theme.strong_text_color();
            assert_ne!(text, strong, "strong_text_color should differ from text_color for {:?}", theme);
        }
    }

    #[test]
    fn test_table_border_returns_color() {
        let light = Theme::Light.table_border();
        let dark = Theme::Dark.table_border();
        assert!(light.a() > 0, "Light table_border should be non-transparent");
        assert!(dark.a() > 0, "Dark table_border should be non-transparent");
    }

    #[test]
    fn test_icon_label_dark() {
        let label = Theme::Dark.icon_label();
        assert!(label.contains('☀'), "Dark theme icon_label should contain sun symbol");
    }

    #[test]
    fn test_icon_label_light() {
        let label = Theme::Light.icon_label();
        assert!(label.contains("🌙"), "Light theme icon_label should contain moon symbol");
    }

    #[test]
    fn test_visuals_dark() {
        let visuals = Theme::Dark.visuals();
        let bg = visuals.window_fill;
        assert!(bg.r() < 100 && bg.g() < 100 && bg.b() < 100,
            "Dark theme visuals should have a dark background");
    }

    #[test]
    fn test_visuals_light() {
        let visuals = Theme::Light.visuals();
        let bg = visuals.window_fill;
        assert!(bg.r() > 200 && bg.g() > 200 && bg.b() > 200,
            "Light theme visuals should have a light background");
    }

    #[test]
    fn test_heading_size_h1_is_largest() {
        let theme = Theme::Dark;
        assert!(theme.heading_size(1) >= theme.heading_size(2));
        assert!(theme.heading_size(2) >= theme.heading_size(3));
        assert!(theme.heading_size(3) >= theme.heading_size(4));
        assert!(theme.heading_size(4) >= theme.heading_size(5));
        assert!(theme.heading_size(5) >= theme.heading_size(6));
    }

    #[test]
    fn test_heading_size_boundaries() {
        let theme = Theme::Light;
        let size_6 = theme.heading_size(6);
        assert_eq!(theme.heading_size(7), size_6, "Level 7 should return same as level 6");
        assert_eq!(theme.heading_size(10), size_6, "Level 10 should return same as level 6");
        assert_eq!(theme.heading_size(0), size_6, "Level 0 should return same as level 6 (fallback)");
    }

    #[test]
    fn test_syntect_theme_dark() {
        assert_eq!(Theme::Dark.syntect_theme_name(), "base16-ocean.dark");
    }

    #[test]
    fn test_syntect_theme_light() {
        assert_eq!(Theme::Light.syntect_theme_name(), "InspiredGitHub");
    }
}
