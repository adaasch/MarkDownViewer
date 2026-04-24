use std::path::Path;
use eframe::egui::{self, Color32, FontFamily, FontId, Margin, RichText, Stroke, Ui, Vec2, Frame};
use eframe::egui::text::LayoutJob;
use eframe::egui::TextFormat;
use crate::highlight::Highlighter;
use crate::images::ImageCache;
use crate::links::{classify_link, heading_to_anchor, LinkAction};
use crate::parser::{InlineElement, ListItem, MdElement};
use crate::theme::Theme;

const MIN_TABLE_COL_PX: f32 = 40.0;
const TABLE_CELL_PADDING_PX: f32 = 6.0;

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
        let num_cols = headers.len().max(1);
        let spacing_x = 8.0f32;
        let row_pad_y = 3.0f32;
        // Frame inner_margin (left + right) + safety buffer so the table never
        // pushes the page past ui.available_width() — otherwise the outer
        // ScrollArea adds a horizontal scrollbar for a couple of stray pixels.
        const TABLE_WIDTH_SAFETY_PX: f32 = 8.0;

        let total_spacing = (num_cols.saturating_sub(1) as f32) * spacing_x;
        let available = (ui.available_width() - total_spacing - TABLE_WIDTH_SAFETY_PX).max(0.0);

        // Measure text widths using the actual font so min/max widths match
        // what will be rendered — char-count × glyph_width('x') underestimates
        // wider letters and causes single-word headers to wrap.
        let font_id = FontId::new(14.0, FontFamily::Proportional);
        let col_widths = ui.fonts_mut(|fonts| {
            let measure = |s: &str| -> f32 {
                if s.is_empty() {
                    0.0
                } else {
                    fonts
                        .layout_no_wrap(s.to_string(), font_id.clone(), Color32::WHITE)
                        .size()
                        .x
                }
            };
            Self::compute_column_widths(headers, rows, available, measure)
        });

        let stripe_color = theme.table_stripe_bg();
        let border_color = theme.table_border();

        // Header row — no stripe, bold text
        self.render_table_row(
            ui,
            headers,
            &col_widths,
            spacing_x,
            row_pad_y,
            Color32::TRANSPARENT,
            true,
            theme,
            base_dir,
            actions,
        );

        // Separator line under header
        let sep_stroke = Stroke::new(1.0, border_color);
        let sep_rect = ui.available_rect_before_wrap();
        let table_width: f32 = col_widths.iter().sum::<f32>() + total_spacing;
        ui.painter().hline(
            sep_rect.min.x..=sep_rect.min.x + table_width,
            sep_rect.min.y,
            sep_stroke,
        );
        ui.add_space(1.0);

        // Body rows — alternating stripe background
        for (row_idx, row) in rows.iter().enumerate() {
            let fill = if row_idx % 2 == 0 {
                stripe_color
            } else {
                Color32::TRANSPARENT
            };
            self.render_table_row(
                ui,
                row,
                &col_widths,
                spacing_x,
                row_pad_y,
                fill,
                false,
                theme,
                base_dir,
                actions,
            );
        }
    }

    /// Render one table row with the given column widths and background fill.
    /// Cells are top-aligned so the row's height equals the tallest cell and
    /// the background covers the full row.
    #[allow(clippy::too_many_arguments)]
    fn render_table_row(
        &mut self,
        ui: &mut Ui,
        cells: &[Vec<InlineElement>],
        col_widths: &[f32],
        spacing_x: f32,
        row_pad_y: f32,
        fill: Color32,
        bold: bool,
        theme: &Theme,
        base_dir: &Path,
        actions: &mut Vec<RenderAction>,
    ) {
        Frame::new()
            .fill(fill)
            .inner_margin(Margin {
                left: 2,
                right: 2,
                top: row_pad_y as i8,
                bottom: row_pad_y as i8,
            })
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = spacing_x;
                    for (i, cell) in cells.iter().enumerate() {
                        let width = col_widths.get(i).copied().unwrap_or(MIN_TABLE_COL_PX);
                        ui.allocate_ui_with_layout(
                            Vec2::new(width, 0.0),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                ui.set_max_width(width);
                                ui.set_min_width(width);
                                self.render_inline_sequence_wrapped(
                                    ui, cell, theme, base_dir, actions, None, bold,
                                    Some(width),
                                );
                            },
                        );
                    }
                });
            });
    }

    /// Compute per-column widths using a CSS `table-layout: auto`-style algorithm.
    ///
    /// For each column we estimate a minimum width (longest unbreakable word)
    /// and a preferred width (widest full line). If the preferred widths sum
    /// to at most `available`, they are used as-is. Otherwise the available
    /// space is distributed between min and preferred in proportion to each
    /// column's slack (max − min). If even the minimums don't fit, columns
    /// fall back to their minimum widths — the table is allowed to exceed the
    /// available width rather than squash words mid-letter.
    fn compute_column_widths<M: FnMut(&str) -> f32>(
        headers: &[Vec<InlineElement>],
        rows: &[Vec<Vec<InlineElement>>],
        available: f32,
        mut measure: M,
    ) -> Vec<f32> {
        let num_cols = headers.len().max(1);

        let mut min_w = vec![0.0f32; num_cols];
        let mut max_w = vec![0.0f32; num_cols];

        let mut measure_cell = |cell: &[InlineElement]| -> (f32, f32) {
            let text = Self::extract_text(cell);
            // max: widest single line (assuming no wrap)
            let mut max_px = 0.0f32;
            for line in text.lines() {
                max_px = max_px.max(measure(line));
            }
            // min: widest unbreakable word
            let mut min_px = 0.0f32;
            for word in text.split_whitespace() {
                min_px = min_px.max(measure(word));
            }
            max_px += TABLE_CELL_PADDING_PX;
            min_px += TABLE_CELL_PADDING_PX;
            (
                min_px.max(MIN_TABLE_COL_PX),
                max_px.max(min_px).max(MIN_TABLE_COL_PX),
            )
        };

        for (i, h) in headers.iter().enumerate().take(num_cols) {
            let (mn, mx) = measure_cell(h);
            min_w[i] = min_w[i].max(mn);
            max_w[i] = max_w[i].max(mx);
        }
        for row in rows {
            for (i, cell) in row.iter().enumerate().take(num_cols) {
                let (mn, mx) = measure_cell(cell);
                min_w[i] = min_w[i].max(mn);
                max_w[i] = max_w[i].max(mx);
            }
        }
        for i in 0..num_cols {
            if min_w[i] < MIN_TABLE_COL_PX {
                min_w[i] = MIN_TABLE_COL_PX;
            }
            if max_w[i] < min_w[i] {
                max_w[i] = min_w[i];
            }
        }

        let sum_max: f32 = max_w.iter().sum();
        let sum_min: f32 = min_w.iter().sum();

        if sum_max <= available {
            max_w
        } else if sum_min < available {
            let extra = available - sum_min;
            let diff_total = sum_max - sum_min;
            if diff_total <= f32::EPSILON {
                min_w
            } else {
                min_w
                    .iter()
                    .zip(max_w.iter())
                    .map(|(&mn, &mx)| mn + extra * (mx - mn) / diff_total)
                    .collect()
            }
        } else {
            min_w
        }
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
        self.render_inline_sequence_wrapped(
            ui,
            inlines,
            theme,
            base_dir,
            actions,
            font_size,
            bold,
            None,
        );
    }

    /// Like `render_inline_sequence` but with an explicit wrap width. When
    /// `wrap_width` is `Some(w)` the text layout wraps at exactly `w` pixels
    /// regardless of `ui.available_width()`; this is how table cells force a
    /// column-specific wrap boundary. Note that `ui.label(job)` cannot be used
    /// here because `Label` unconditionally overwrites `job.wrap.max_width`
    /// with `ui.available_width()`; we lay out the galley ourselves and paint
    /// it to preserve the explicit wrap width.
    #[allow(clippy::too_many_arguments)]
    fn render_inline_sequence_wrapped(
        &mut self,
        ui: &mut Ui,
        inlines: &[InlineElement],
        theme: &Theme,
        base_dir: &Path,
        actions: &mut Vec<RenderAction>,
        font_size: Option<f32>,
        bold: bool,
        wrap_width: Option<f32>,
    ) {
        if !Self::has_interactive(inlines) {
            // Fast path: pure text, build a single LayoutJob
            let mut job = LayoutJob::default();
            job.wrap.max_width = wrap_width.unwrap_or_else(|| ui.available_width());
            job.break_on_newline = true;
            let mut style = InlineStyleState {
                bold,
                ..Default::default()
            };
            Self::append_inlines_to_job(&mut job, inlines, theme, font_size, &mut style);
            if job.text.is_empty() {
                return;
            }
            if wrap_width.is_some() {
                // Paint galley directly so our explicit wrap width is honored.
                let galley = ui.fonts_mut(|f| f.layout_job(job));
                let (rect, _resp) =
                    ui.allocate_exact_size(galley.size(), egui::Sense::hover());
                ui.painter()
                    .galley(rect.min, galley, theme.text_color());
            } else {
                ui.label(job);
            }
        } else {
            // Mixed path: text + interactive elements
            self.render_mixed_inlines(
                ui, inlines, theme, base_dir, actions, font_size, bold, false, false,
                wrap_width,
            );
        }
    }

    /// Render inline sequence that contains links or images.
    /// Accumulates text into LayoutJob, flushes before interactive elements.
    #[allow(clippy::too_many_arguments)]
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
        wrap_width: Option<f32>,
    ) {
        let explicit_width = wrap_width;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            if let Some(w) = explicit_width {
                ui.set_max_width(w);
            }

            let mut job = LayoutJob::default();
            job.wrap.max_width = explicit_width.unwrap_or_else(|| ui.available_width());
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

    // --- compute_column_widths tests ---

    fn txt(s: &str) -> Vec<InlineElement> {
        vec![InlineElement::Text(s.to_string())]
    }

    /// Deterministic test measure function: 7px per char. Matches the old
    /// behavior so the numeric expectations in existing tests still hold.
    fn test_measure(s: &str) -> f32 {
        s.chars().count() as f32 * 7.0
    }

    #[test]
    fn test_compute_column_widths_fits_uses_preferred() {
        let headers = vec![txt("A"), txt("B")];
        let rows = vec![vec![txt("x"), txt("y")]];
        let widths = MdRenderer::compute_column_widths(&headers, &rows, 1000.0, test_measure);
        assert_eq!(widths.len(), 2);
        // Each column fits naturally — widths bounded by min col px (40) at minimum
        for w in &widths {
            assert!(*w >= MIN_TABLE_COL_PX - 0.01);
        }
        // Total well below available
        let total: f32 = widths.iter().sum();
        assert!(total < 1000.0);
    }

    #[test]
    fn test_compute_column_widths_squeezed_interpolates() {
        // Column 2 has much longer content than column 1
        let headers = vec![txt("Short"), txt("Header")];
        let long_line = "word ".repeat(40); // ~200 chars, widest line
        let rows = vec![vec![txt("a"), txt(&long_line)]];
        let available = 400.0;
        let widths = MdRenderer::compute_column_widths(&headers, &rows, available, test_measure);
        assert_eq!(widths.len(), 2);
        let total: f32 = widths.iter().sum();
        // Should fit within available (minimums fit since "word" is only 4 chars)
        assert!(total <= available + 0.5, "total {} should be ≤ available {}", total, available);
        // The long column should be wider than the short one
        assert!(widths[1] > widths[0]);
    }

    #[test]
    fn test_compute_column_widths_overflow_on_huge_unbreakable_word() {
        // A column contains a single word longer than the available width
        let huge = "a".repeat(200);
        let headers = vec![txt(&huge)];
        let rows: Vec<Vec<Vec<InlineElement>>> = vec![];
        let available = 100.0;
        let widths = MdRenderer::compute_column_widths(&headers, &rows, available, test_measure);
        assert_eq!(widths.len(), 1);
        // Min width is the longest word ≈ 200 * 7 = 1400px, which exceeds 100.
        // Expected: table overflows; column uses min width.
        assert!(widths[0] > available, "expected overflow: width {} should exceed available {}", widths[0], available);
    }

    #[test]
    fn test_compute_column_widths_empty_cells_get_minimum() {
        let headers = vec![txt(""), txt("")];
        let rows: Vec<Vec<Vec<InlineElement>>> = vec![];
        let widths = MdRenderer::compute_column_widths(&headers, &rows, 1000.0, test_measure);
        assert_eq!(widths.len(), 2);
        for w in &widths {
            assert!(*w >= MIN_TABLE_COL_PX - 0.01);
        }
    }

    #[test]
    fn test_compute_column_widths_preserves_column_count() {
        let headers = vec![txt("A"), txt("B"), txt("C")];
        let rows = vec![
            vec![txt("1"), txt("2"), txt("3")],
            vec![txt("4"), txt("5"), txt("6")],
        ];
        let widths = MdRenderer::compute_column_widths(&headers, &rows, 500.0, test_measure);
        assert_eq!(widths.len(), 3);
    }

    #[test]
    fn test_compute_column_widths_handles_single_column() {
        let headers = vec![txt("Only")];
        let rows = vec![vec![txt("cell")]];
        let widths = MdRenderer::compute_column_widths(&headers, &rows, 300.0, test_measure);
        assert_eq!(widths.len(), 1);
        assert!(widths[0] >= MIN_TABLE_COL_PX);
    }

    #[test]
    fn test_compute_column_widths_interpolation_respects_minimum() {
        // When squeezed, a column should never get less than its longest-word width.
        let headers = vec![txt("supercalifragilistic"), txt("A")];
        let rows = vec![vec![txt("short"), txt("x".repeat(500).as_str())]];
        let available = 300.0;
        let widths = MdRenderer::compute_column_widths(&headers, &rows, available, test_measure);
        // Column 0's longest word is "supercalifragilistic" = 20 chars * 7 + padding ≈ 146
        let col0_min = 20.0 * 7.0;
        assert!(widths[0] >= col0_min - 10.0, "col0 width {} should be >= {}", widths[0], col0_min);
    }
}
