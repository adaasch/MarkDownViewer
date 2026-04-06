use std::path::Path;
use eframe::egui::{self, Color32, FontFamily, FontId, Margin, RichText, Stroke, Ui, Vec2, Frame};
use eframe::egui::text::LayoutJob;
use eframe::egui::TextFormat;
use crate::highlight::Highlighter;
use crate::images::ImageCache;
use crate::links::{classify_link, heading_to_anchor, LinkAction};
use crate::parser::{InlineElement, ListItem, MdElement};
use crate::theme::Theme;

#[derive(Clone, Debug, Default)]
struct InlineStyleState {
    bold: bool,
    italic: bool,
    strikethrough: bool,
    html_code: bool,
    color_override: Option<Color32>,
}

/// Return value from rendering — tells the app what action to take
#[derive(Debug, Clone)]
pub enum RenderAction {
    NavigateMarkdown(std::path::PathBuf),
    NavigateTextFile(std::path::PathBuf),
    OpenExternal(String),
    ScrollToAnchor(String),
}

pub struct MdRenderer {
    pub highlighter: Highlighter,
    pub image_cache: ImageCache,
    pub scroll_target: Option<String>,
}

impl MdRenderer {
    pub fn new() -> Self {
        Self {
            highlighter: Highlighter::new(),
            image_cache: ImageCache::new(),
            scroll_target: None,
        }
    }

    /// Render the parsed markdown elements into the UI.
    pub fn render(
        &mut self,
        ui: &mut Ui,
        elements: &[MdElement],
        theme: &Theme,
        base_dir: &Path,
    ) -> Vec<RenderAction> {
        let mut actions = Vec::new();
        for element in elements {
            self.render_element(ui, element, theme, base_dir, &mut actions);
            ui.add_space(4.0);
        }
        actions
    }

    fn render_element(
        &mut self,
        ui: &mut Ui,
        element: &MdElement,
        theme: &Theme,
        base_dir: &Path,
        actions: &mut Vec<RenderAction>,
    ) {
        match element {
            MdElement::Heading { level, content } => {
                self.render_heading(ui, *level, content, theme, base_dir, actions);
            }
            MdElement::Paragraph(inlines) => {
                self.render_paragraph(ui, inlines, theme, base_dir, actions);
            }
            MdElement::CodeBlock { language, code } => {
                self.render_code_block(ui, language.as_deref(), code, theme);
            }
            MdElement::Table { headers, rows } => {
                self.render_table(ui, headers, rows, theme, base_dir, actions);
            }
            MdElement::List { ordered, start, items } => {
                self.render_list(ui, *ordered, *start, items, theme, base_dir, actions);
            }
            MdElement::ThematicBreak => {
                ui.separator();
            }
            MdElement::BlockQuote(children) => {
                self.render_blockquote(ui, children, theme, base_dir, actions);
            }
            MdElement::HtmlBlock(html) => {
                self.render_html_block(ui, html, theme);
            }
        }
    }

    fn render_heading(
        &mut self,
        ui: &mut Ui,
        level: u8,
        content: &[InlineElement],
        theme: &Theme,
        base_dir: &Path,
        actions: &mut Vec<RenderAction>,
    ) {
        let size = theme.heading_size(level);
        ui.add_space(8.0);
        let anchor = heading_to_anchor(&Self::extract_text(content));
        let response = ui.push_id(&anchor, |ui| {
            self.render_inline_sequence(ui, content, theme, base_dir, actions, Some(size), true);
        });
        // Store the heading rect for scroll-to-anchor
        if let Some(ref target) = self.scroll_target {
            if *target == anchor {
                response.response.scroll_to_me(Some(egui::Align::TOP));
                self.scroll_target = None;
            }
        }
        if level <= 2 {
            ui.separator();
        }
    }

    fn render_paragraph(
        &mut self,
        ui: &mut Ui,
        inlines: &[InlineElement],
        theme: &Theme,
        base_dir: &Path,
        actions: &mut Vec<RenderAction>,
    ) {
        self.render_inline_sequence(ui, inlines, theme, base_dir, actions, None, false);
    }

    fn render_code_block(
        &mut self,
        ui: &mut Ui,
        language: Option<&str>,
        code: &str,
        theme: &Theme,
    ) {
        let bg = theme.code_bg();
        let available_width = ui.available_width();
        Frame::new()
            .fill(bg)
            .inner_margin(Margin::same(8))
            .corner_radius(4.0)
            .show(ui, |ui| {
                ui.set_width(available_width - 16.0);

                // Header row: language label + copy button
                ui.horizontal(|ui| {
                    if let Some(lang) = language {
                        ui.label(RichText::new(lang).small().color(Color32::GRAY));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("📋 Copy").clicked() {
                            ui.ctx().copy_text(code.to_string());
                        }
                    });
                });

                ui.add_space(2.0);

                let job = self.highlighter.highlight(code, language, theme.syntect_theme_name());
                ui.label(job);
            });
    }

    fn render_table(
        &mut self,
        ui: &mut Ui,
        headers: &[Vec<InlineElement>],
        rows: &[Vec<Vec<InlineElement>>],
        theme: &Theme,
        base_dir: &Path,
        actions: &mut Vec<RenderAction>,
    ) {
        let _num_cols = headers.len().max(1);
        let _available = ui.available_width();

        egui::Grid::new(ui.next_auto_id())
            .striped(true)
            .min_col_width(40.0)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                for cell in headers {
                    self.render_inline_sequence(ui, cell, theme, base_dir, actions, None, true);
                }
                ui.end_row();
                for row in rows {
                    for cell in row {
                        self.render_inline_sequence(ui, cell, theme, base_dir, actions, None, false);
                    }
                    ui.end_row();
                }
            });
    }

    fn render_list(
        &mut self,
        ui: &mut Ui,
        ordered: bool,
        start: Option<u64>,
        items: &[ListItem],
        theme: &Theme,
        base_dir: &Path,
        actions: &mut Vec<RenderAction>,
    ) {
        let start_num = start.unwrap_or(1);
        for (i, item) in items.iter().enumerate() {
            let marker = if let Some(checked) = item.checked {
                if checked { "☑".to_string() } else { "☐".to_string() }
            } else if ordered {
                format!("{}.", start_num + i as u64)
            } else {
                "•".to_string()
            };

            // Use left_to_right with top-alignment so content wraps properly
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                ui.add_space(16.0);
                ui.label(&marker);
                ui.vertical(|ui| {
                    for element in &item.content {
                        self.render_element(ui, element, theme, base_dir, actions);
                    }
                });
            });
        }
    }

    fn render_blockquote(
        &mut self,
        ui: &mut Ui,
        children: &[MdElement],
        theme: &Theme,
        base_dir: &Path,
        actions: &mut Vec<RenderAction>,
    ) {
        let border_color = theme.blockquote_border();
        let bg_color = theme.blockquote_bg();

        // Asymmetric margin: extra left padding for border bar + gap
        Frame::new()
            .fill(bg_color)
            .inner_margin(Margin {
                left: 16,
                right: 8,
                top: 8,
                bottom: 8,
            })
            .corner_radius(2.0)
            .show(ui, |ui| {
                // Draw left border bar in the margin gap
                let rect = ui.max_rect();
                let bar_x = rect.left() - 12.0;
                ui.painter().line_segment(
                    [
                        egui::pos2(bar_x, rect.top()),
                        egui::pos2(bar_x, rect.bottom()),
                    ],
                    Stroke::new(3.0, border_color),
                );

                for child in children {
                    self.render_element(ui, child, theme, base_dir, actions);
                }
            });
    }

    fn render_html_block(&self, ui: &mut Ui, html: &str, theme: &Theme) {
        let trimmed = html.trim();
        if is_html_comment(trimmed) {
            return;
        }
        if is_line_break_tag(trimmed) {
            ui.add_space(8.0);
            return;
        }
        if is_horizontal_rule_tag(trimmed) {
            ui.separator();
            return;
        }
        let text = html_to_display_text(trimmed);
        if !text.trim().is_empty() {
            ui.label(RichText::new(text.trim()).color(theme.text_color()));
        }
    }

    // --- Inline rendering using LayoutJob ---

    /// Check recursively if any inline element is interactive (link or image)
    fn has_interactive(inlines: &[InlineElement]) -> bool {
        inlines.iter().any(|i| match i {
            InlineElement::Link { .. } | InlineElement::Image { .. } => true,
            InlineElement::Bold(c) | InlineElement::Italic(c) | InlineElement::Strikethrough(c) => {
                Self::has_interactive(c)
            }
            _ => false,
        })
    }

    /// Main inline rendering entry point.
    /// Uses LayoutJob for text-only sequences, mixed approach for sequences with links/images.
    fn render_inline_sequence(
        &mut self,
        ui: &mut Ui,
        inlines: &[InlineElement],
        theme: &Theme,
        base_dir: &Path,
        actions: &mut Vec<RenderAction>,
        font_size: Option<f32>,
        bold: bool,
    ) {
        if !Self::has_interactive(inlines) {
            // Fast path: pure text, build a single LayoutJob
            let mut job = LayoutJob::default();
            job.wrap.max_width = ui.available_width();
            job.break_on_newline = true;
            let mut style = InlineStyleState {
                bold,
                ..Default::default()
            };
            Self::append_inlines_to_job(&mut job, inlines, theme, font_size, &mut style);
            if !job.text.is_empty() {
                ui.label(job);
            }
        } else {
            // Mixed path: text + interactive elements
            self.render_mixed_inlines(ui, inlines, theme, base_dir, actions, font_size, bold, false, false);
        }
    }

    /// Render inline sequence that contains links or images.
    /// Accumulates text into LayoutJob, flushes before interactive elements.
    fn render_mixed_inlines(
        &mut self,
        ui: &mut Ui,
        inlines: &[InlineElement],
        theme: &Theme,
        base_dir: &Path,
        actions: &mut Vec<RenderAction>,
        font_size: Option<f32>,
        bold: bool,
        italic: bool,
        strikethrough: bool,
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            let mut job = LayoutJob::default();
            job.wrap.max_width = ui.available_width();
            job.break_on_newline = true;

            for inline in inlines {
                self.collect_or_flush(ui, &mut job, inline, theme, base_dir, actions, font_size, bold, italic, strikethrough);
            }

            // Flush remaining text
            if !job.text.is_empty() {
                ui.label(std::mem::take(&mut job));
            }
        });
    }

    /// Either append an inline element to the current LayoutJob, or flush and render interactively.
    fn collect_or_flush(
        &mut self,
        ui: &mut Ui,
        job: &mut LayoutJob,
        inline: &InlineElement,
        theme: &Theme,
        base_dir: &Path,
        actions: &mut Vec<RenderAction>,
        font_size: Option<f32>,
        bold: bool,
        italic: bool,
        strikethrough: bool,
    ) {
        match inline {
            InlineElement::Text(_) | InlineElement::Code(_)
            | InlineElement::SoftBreak | InlineElement::HardBreak
            | InlineElement::Html(_) => {
                let mut style = InlineStyleState {
                    bold,
                    italic,
                    strikethrough,
                    ..Default::default()
                };
                Self::append_inline_to_job(job, inline, theme, font_size, &mut style);
            }
            InlineElement::Bold(children) => {
                for child in children {
                    self.collect_or_flush(ui, job, child, theme, base_dir, actions, font_size, true, italic, strikethrough);
                }
            }
            InlineElement::Italic(children) => {
                for child in children {
                    self.collect_or_flush(ui, job, child, theme, base_dir, actions, font_size, bold, true, strikethrough);
                }
            }
            InlineElement::Strikethrough(children) => {
                for child in children {
                    self.collect_or_flush(ui, job, child, theme, base_dir, actions, font_size, bold, italic, true);
                }
            }
            InlineElement::Link { content, url } => {
                // Flush accumulated text first
                if !job.text.is_empty() {
                    ui.label(std::mem::take(job));
                    job.wrap.max_width = ui.available_width();
                    job.break_on_newline = true;
                }
                // Render link as interactive widget
                let link_text = Self::extract_text(content);
                let mut rt = RichText::new(&link_text).color(theme.link_color());
                if let Some(size) = font_size {
                    rt = rt.size(size);
                }
                if bold { rt = rt.strong(); }
                if italic { rt = rt.italics(); }

                let response = ui.link(rt);
                if response.clicked() {
                    match classify_link(url, base_dir) {
                        LinkAction::NavigateMarkdown(path) => {
                            actions.push(RenderAction::NavigateMarkdown(path));
                        }
                        LinkAction::NavigateTextFile(path) => {
                            actions.push(RenderAction::NavigateTextFile(path));
                        }
                        LinkAction::ScrollToAnchor(anchor) => {
                            actions.push(RenderAction::ScrollToAnchor(anchor));
                        }
                        LinkAction::OpenExternal(url) => {
                            actions.push(RenderAction::OpenExternal(url));
                        }
                    }
                }
                if response.hovered() {
                    response.on_hover_text(url);
                }
            }
            InlineElement::Image { alt, url } => {
                // Flush accumulated text first
                if !job.text.is_empty() {
                    ui.label(std::mem::take(job));
                    job.wrap.max_width = ui.available_width();
                    job.break_on_newline = true;
                }
                // Render image
                let image_path = base_dir.join(url);
                if let Some(texture) = self.image_cache.get_or_load(&image_path, ui.ctx()) {
                    let size = texture.size_vec2();
                    if size.x > 0.0 && size.y > 0.0 {
                        let max_width = ui.available_width();
                        let scale = (max_width / size.x).min(1.0);
                        let display_size = Vec2::new(size.x * scale, size.y * scale);
                        ui.image((texture.id(), display_size));
                    } else {
                        ui.label(RichText::new(format!("[Invalid image: {alt}]")).italics());
                    }
                } else {
                    ui.label(RichText::new(format!("[Image: {alt}]")).italics());
                }
            }
        }
    }

    /// Recursively append inline elements to a LayoutJob (non-interactive only).
    fn append_inlines_to_job(
        job: &mut LayoutJob,
        inlines: &[InlineElement],
        theme: &Theme,
        font_size: Option<f32>,
        style: &mut InlineStyleState,
    ) {
        for inline in inlines {
            Self::append_inline_to_job(job, inline, theme, font_size, style);
        }
    }

    /// Append a single inline element to a LayoutJob.
    fn append_inline_to_job(
        job: &mut LayoutJob,
        inline: &InlineElement,
        theme: &Theme,
        font_size: Option<f32>,
        style: &mut InlineStyleState,
    ) {
        let size = font_size.unwrap_or(14.0);
        let color = style.color_override.unwrap_or_else(|| {
            if style.bold {
                theme.strong_text_color()
            } else {
                theme.text_color()
            }
        });
        let st = if style.strikethrough {
            Stroke::new(1.0, color)
        } else {
            Stroke::NONE
        };

        match inline {
            InlineElement::Text(text) => {
                let family = FontFamily::Proportional;
                let format = TextFormat {
                    font_id: FontId::new(size, family),
                    color,
                    italics: style.italic,
                    strikethrough: st,
                    ..Default::default()
                };
                job.append(text, 0.0, format);
            }
            InlineElement::Bold(children) => {
                let mut nested = style.clone();
                nested.bold = true;
                Self::append_inlines_to_job(job, children, theme, font_size, &mut nested);
            }
            InlineElement::Italic(children) => {
                let mut nested = style.clone();
                nested.italic = true;
                Self::append_inlines_to_job(job, children, theme, font_size, &mut nested);
            }
            InlineElement::Strikethrough(children) => {
                let mut nested = style.clone();
                nested.strikethrough = true;
                Self::append_inlines_to_job(job, children, theme, font_size, &mut nested);
            }
            InlineElement::Code(code) => {
                let format = TextFormat {
                    font_id: FontId::new(size, FontFamily::Monospace),
                    color,
                    background: theme.code_bg(),
                    italics: style.italic,
                    strikethrough: st,
                    ..Default::default()
                };
                job.append(code, 0.0, format);
            }
            InlineElement::SoftBreak => {
                let format = TextFormat {
                    font_id: FontId::new(size, FontFamily::Proportional),
                    color,
                    ..Default::default()
                };
                job.append(" ", 0.0, format);
            }
            InlineElement::HardBreak => {
                let format = TextFormat {
                    font_id: FontId::new(size, FontFamily::Proportional),
                    color,
                    ..Default::default()
                };
                job.append("\n", 0.0, format);
            }
            // Interactive elements are not appended to LayoutJob
            InlineElement::Link { .. } | InlineElement::Image { .. } => {}
            InlineElement::Html(html) => {
                let trimmed = html.trim();
                if is_html_comment(trimmed) {
                    return;
                }
                if is_line_break_tag(trimmed) {
                    let format = TextFormat {
                        font_id: FontId::new(size, FontFamily::Proportional),
                        color,
                        ..Default::default()
                    };
                    job.append("\n", 0.0, format);
                    return;
                }
                if apply_inline_html_style(trimmed, style) {
                    return;
                }

                let text = html_to_display_text(trimmed);
                if !text.is_empty() {
                    let format = TextFormat {
                        font_id: FontId::new(
                            size,
                            if style.html_code {
                                FontFamily::Monospace
                            } else {
                                FontFamily::Proportional
                            },
                        ),
                        color,
                        background: if style.html_code {
                            theme.code_bg()
                        } else {
                            Color32::TRANSPARENT
                        },
                        italics: style.italic,
                        strikethrough: st,
                        ..Default::default()
                    };
                    job.append(&text, 0.0, format);
                }
            }
        }
    }

    fn extract_text(inlines: &[InlineElement]) -> String {
        let mut s = String::new();
        for inline in inlines {
            match inline {
                InlineElement::Text(t) => s.push_str(t),
                InlineElement::Bold(children)
                | InlineElement::Italic(children)
                | InlineElement::Strikethrough(children) => {
                    s.push_str(&Self::extract_text(children));
                }
                InlineElement::Code(c) => s.push_str(c),
                InlineElement::Link { content, .. } => {
                    s.push_str(&Self::extract_text(content));
                }
                InlineElement::SoftBreak => s.push(' '),
                InlineElement::HardBreak => s.push('\n'),
                InlineElement::Image { alt, .. } => s.push_str(alt),
                InlineElement::Html(html) => {
                    let text = html_to_display_text(html);
                    s.push_str(&text);
                }
            }
        }
        s
    }
}

fn is_html_comment(html: &str) -> bool {
    html.starts_with("<!--") && html.ends_with("-->")
}

fn is_line_break_tag(html: &str) -> bool {
    matches!(html, "<br>" | "<br/>" | "<br />")
}

fn is_horizontal_rule_tag(html: &str) -> bool {
    matches!(html, "<hr>" | "<hr/>" | "<hr />")
}

fn parse_named_color(name: &str) -> Option<Color32> {
    match name.trim().to_ascii_lowercase().as_str() {
        "red" => Some(Color32::from_rgb(220, 60, 60)),
        "green" => Some(Color32::from_rgb(46, 160, 67)),
        "blue" => Some(Color32::from_rgb(66, 133, 244)),
        "orange" => Some(Color32::from_rgb(230, 126, 34)),
        "yellow" => Some(Color32::from_rgb(241, 196, 15)),
        "gray" | "grey" => Some(Color32::from_gray(140)),
        _ => None,
    }
}

fn parse_html_color(tag: &str) -> Option<Color32> {
    let lower = tag.to_ascii_lowercase();
    let style_pos = lower.find("style=")?;
    let style = &tag[style_pos..];
    let color_pos = style.to_ascii_lowercase().find("color")?;
    let color_slice = &style[color_pos..];
    let colon_pos = color_slice.find(':')?;
    let value = color_slice[colon_pos + 1..]
        .trim_start()
        .trim_matches('"')
        .trim_matches('\'');
    let end = value
        .find([';', '"', '\'', '>'])
        .unwrap_or(value.len());
    parse_named_color(&value[..end])
}

fn apply_inline_html_style(tag: &str, style: &mut InlineStyleState) -> bool {
    let lower = tag.trim().to_ascii_lowercase();
    match lower.as_str() {
        "<b>" | "<strong>" => {
            style.bold = true;
            true
        }
        "</b>" | "</strong>" => {
            style.bold = false;
            true
        }
        "<i>" | "<em>" => {
            style.italic = true;
            true
        }
        "</i>" | "</em>" => {
            style.italic = false;
            true
        }
        "<s>" | "<strike>" | "<del>" => {
            style.strikethrough = true;
            true
        }
        "</s>" | "</strike>" | "</del>" => {
            style.strikethrough = false;
            true
        }
        "<code>" => {
            style.html_code = true;
            true
        }
        "</code>" => {
            style.html_code = false;
            true
        }
        "</span>" => {
            style.color_override = None;
            true
        }
        _ if lower.starts_with("<span") => {
            style.color_override = parse_html_color(tag);
            true
        }
        _ => false,
    }
}

/// Strip HTML tags, returning only the text content between tags.
#[allow(dead_code)]
pub fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    result
}

fn decode_html_entities(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn html_to_display_text(html: &str) -> String {
    let mut result = String::new();
    let mut chars = html.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            let mut tag = String::new();
            while let Some(next) = chars.next() {
                if next == '>' {
                    break;
                }
                tag.push(next);
            }
            let normalized = tag.trim().to_ascii_lowercase();
            if normalized.starts_with("!--") {
                continue;
            }
            if normalized == "br" || normalized == "br/" {
                result.push('\n');
            } else if normalized == "hr" || normalized == "hr/" {
                if !result.ends_with('\n') {
                    result.push('\n');
                }
            } else if normalized == "li" {
                if !result.is_empty() && !result.ends_with('\n') {
                    result.push('\n');
                }
                result.push_str("• ");
            } else if normalized.starts_with("/p")
                || normalized.starts_with("/div")
                || normalized.starts_with("/li")
                || normalized.starts_with("/ul")
                || normalized.starts_with("/ol")
                || normalized.starts_with("/blockquote")
                || normalized.starts_with("/h1")
                || normalized.starts_with("/h2")
                || normalized.starts_with("/h3")
                || normalized.starts_with("/h4")
                || normalized.starts_with("/h5")
                || normalized.starts_with("/h6")
            {
                if !result.ends_with('\n') {
                    result.push('\n');
                }
            }
        } else {
            result.push(ch);
        }
    }

    decode_html_entities(result.trim_matches('\n')).trim().to_string()
}

/// Extract TOC entries (level, text, anchor) from parsed elements.
pub fn extract_toc(elements: &[MdElement]) -> Vec<(u8, String, String)> {
    let mut toc = Vec::new();
    for element in elements {
        if let MdElement::Heading { level, content } = element {
            let text = MdRenderer::extract_text(content);
            let anchor = heading_to_anchor(&text);
            toc.push((*level, text, anchor));
        }
    }
    toc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::InlineElement;

    #[test]
    fn test_renderer_creation() {
        let renderer = MdRenderer::new();
        let _ = renderer;
    }

    #[test]
    fn test_extract_text_simple() {
        let inlines = vec![InlineElement::Text("Hello World".to_string())];
        assert_eq!(MdRenderer::extract_text(&inlines), "Hello World");
    }

    #[test]
    fn test_extract_text_nested() {
        let inlines = vec![
            InlineElement::Text("Hello ".to_string()),
            InlineElement::Bold(vec![InlineElement::Text("bold".to_string())]),
            InlineElement::Text(" world".to_string()),
        ];
        assert_eq!(MdRenderer::extract_text(&inlines), "Hello bold world");
    }

    #[test]
    fn test_has_interactive() {
        let no_interactive = vec![
            InlineElement::Text("hello".to_string()),
            InlineElement::Bold(vec![InlineElement::Text("bold".to_string())]),
        ];
        assert!(!MdRenderer::has_interactive(&no_interactive));

        let with_link = vec![
            InlineElement::Text("see ".to_string()),
            InlineElement::Link {
                content: vec![InlineElement::Text("here".to_string())],
                url: "http://example.com".to_string(),
            },
        ];
        assert!(MdRenderer::has_interactive(&with_link));

        let nested_link = vec![
            InlineElement::Bold(vec![
                InlineElement::Link {
                    content: vec![InlineElement::Text("link".to_string())],
                    url: "http://example.com".to_string(),
                },
            ]),
        ];
        assert!(MdRenderer::has_interactive(&nested_link));
    }

    // --- strip_html_tags tests ---

    #[test]
    fn test_strip_html_tags_simple() {
        assert_eq!(strip_html_tags("<b>bold</b>"), "bold");
    }

    #[test]
    fn test_strip_html_tags_nested() {
        assert_eq!(strip_html_tags("<div><p>text</p></div>"), "text");
    }

    #[test]
    fn test_strip_html_tags_no_tags() {
        assert_eq!(strip_html_tags("plain text"), "plain text");
    }

    #[test]
    fn test_strip_html_tags_empty() {
        assert_eq!(strip_html_tags(""), "");
    }

    #[test]
    fn test_strip_html_tags_self_closing() {
        assert_eq!(strip_html_tags("before<br/>after"), "beforeafter");
    }

    #[test]
    fn test_strip_html_tags_attributes() {
        assert_eq!(strip_html_tags("<a href=\"url\">link</a>"), "link");
    }

    #[test]
    fn test_strip_html_tags_mixed() {
        assert_eq!(strip_html_tags("hello <em>world</em> end"), "hello world end");
    }

    #[test]
    fn test_strip_html_tags_only_tags() {
        assert_eq!(strip_html_tags("<br><hr>"), "");
    }

    #[test]
    fn test_html_to_display_text_preserves_basic_block_structure() {
        let html = "<h1>Welcome</h1><p>This is a test.</p><ul><li>One</li><li>Two</li></ul>";
        assert_eq!(html_to_display_text(html), "Welcome\nThis is a test.\n• One\n• Two");
    }

    #[test]
    fn test_html_to_display_text_decodes_entities_and_ignores_comments() {
        let html = "<!-- comment --><p>&lt;tag&gt; &amp; text</p>";
        assert_eq!(html_to_display_text(html), "<tag> & text");
    }

    // --- extract_toc tests ---

    #[test]
    fn test_extract_toc_single_heading() {
        let elements = vec![MdElement::Heading {
            level: 1,
            content: vec![InlineElement::Text("Title".to_string())],
        }];
        let toc = extract_toc(&elements);
        assert_eq!(toc, vec![(1, "Title".to_string(), "title".to_string())]);
    }

    #[test]
    fn test_extract_toc_multiple_headings() {
        let elements = vec![
            MdElement::Heading {
                level: 1,
                content: vec![InlineElement::Text("First".to_string())],
            },
            MdElement::Heading {
                level: 2,
                content: vec![InlineElement::Text("Second".to_string())],
            },
            MdElement::Heading {
                level: 3,
                content: vec![InlineElement::Text("Third".to_string())],
            },
        ];
        let toc = extract_toc(&elements);
        assert_eq!(toc.len(), 3);
        assert_eq!(toc[0], (1, "First".to_string(), "first".to_string()));
        assert_eq!(toc[1], (2, "Second".to_string(), "second".to_string()));
        assert_eq!(toc[2], (3, "Third".to_string(), "third".to_string()));
    }

    #[test]
    fn test_extract_toc_empty() {
        let elements: Vec<MdElement> = vec![];
        let toc = extract_toc(&elements);
        assert!(toc.is_empty());
    }

    #[test]
    fn test_extract_toc_no_headings() {
        let elements = vec![
            MdElement::Paragraph(vec![InlineElement::Text("Just text".to_string())]),
            MdElement::ThematicBreak,
        ];
        let toc = extract_toc(&elements);
        assert!(toc.is_empty());
    }

    #[test]
    fn test_extract_toc_heading_with_special_chars() {
        let elements = vec![MdElement::Heading {
            level: 1,
            content: vec![InlineElement::Text("Hello World!".to_string())],
        }];
        let toc = extract_toc(&elements);
        assert_eq!(toc, vec![(1, "Hello World!".to_string(), "hello-world".to_string())]);
    }

    #[test]
    fn test_extract_toc_heading_with_inline_formatting() {
        let elements = vec![MdElement::Heading {
            level: 2,
            content: vec![
                InlineElement::Bold(vec![InlineElement::Text("Bold".to_string())]),
                InlineElement::Text(" and ".to_string()),
                InlineElement::Italic(vec![InlineElement::Text("Italic".to_string())]),
            ],
        }];
        let toc = extract_toc(&elements);
        assert_eq!(toc.len(), 1);
        assert_eq!(toc[0].0, 2);
        assert_eq!(toc[0].1, "Bold and Italic");
    }

    // --- extract_text tests ---

    #[test]
    fn test_extract_text_with_code() {
        let inlines = vec![InlineElement::Code("code".to_string())];
        assert_eq!(MdRenderer::extract_text(&inlines), "code");
    }

    #[test]
    fn test_extract_text_with_html() {
        // extract_text strips HTML tags via strip_html_tags
        let inlines = vec![InlineElement::Html("<b>text</b>".to_string())];
        assert_eq!(MdRenderer::extract_text(&inlines), "text");
    }

    #[test]
    fn test_extract_text_with_link() {
        let inlines = vec![InlineElement::Link {
            content: vec![InlineElement::Text("click here".to_string())],
            url: "http://example.com".to_string(),
        }];
        assert_eq!(MdRenderer::extract_text(&inlines), "click here");
    }

    #[test]
    fn test_extract_text_empty() {
        let inlines: Vec<InlineElement> = vec![];
        assert_eq!(MdRenderer::extract_text(&inlines), "");
    }

    // --- has_interactive tests ---

    #[test]
    fn test_has_interactive_with_link() {
        let inlines = vec![InlineElement::Link {
            content: vec![InlineElement::Text("link".to_string())],
            url: "http://example.com".to_string(),
        }];
        assert!(MdRenderer::has_interactive(&inlines));
    }

    #[test]
    fn test_has_interactive_with_image() {
        let inlines = vec![InlineElement::Image {
            alt: "img".to_string(),
            url: "http://example.com/img.png".to_string(),
        }];
        assert!(MdRenderer::has_interactive(&inlines));
    }

    #[test]
    fn test_has_interactive_text_only() {
        let inlines = vec![InlineElement::Text("just text".to_string())];
        assert!(!MdRenderer::has_interactive(&inlines));
    }

    #[test]
    fn test_has_interactive_nested_link_in_bold() {
        let inlines = vec![InlineElement::Bold(vec![InlineElement::Link {
            content: vec![InlineElement::Text("link".to_string())],
            url: "http://example.com".to_string(),
        }])];
        assert!(MdRenderer::has_interactive(&inlines));
    }

    #[test]
    fn test_has_interactive_with_html() {
        let inlines = vec![InlineElement::Html("<b>text</b>".to_string())];
        assert!(!MdRenderer::has_interactive(&inlines));
    }
}
