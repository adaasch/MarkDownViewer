use crate::navigation::NavigationHistory;
use crate::parser::{parse_markdown, MdElement};
use crate::renderer::{extract_toc, MdRenderer, RenderAction};
use crate::theme::Theme;
use crate::watcher::FileWatcher;
use eframe::egui;
use std::path::PathBuf;

enum PendingNavigation {
    Back,
    Forward,
    Markdown(PathBuf),
    PlainText(PathBuf),
}

pub struct MdViewApp {
    navigation: NavigationHistory,
    theme: Theme,
    renderer: MdRenderer,
    watcher: Option<FileWatcher>,
    elements: Vec<MdElement>,
    content: String,
    error: Option<String>,
    show_about: bool,
    is_plain_text: bool,
    toc: Vec<(u8, String, String)>,
    icon_texture: Option<egui::TextureHandle>,
}

impl MdViewApp {
    pub fn new(file_path: PathBuf) -> Self {
        let (content, elements, error) = match std::fs::read_to_string(&file_path) {
            Ok(content) => {
                let elements = parse_markdown(&content);
                (content, elements, None)
            }
            Err(e) => (
                String::new(),
                Vec::new(),
                Some(format!("Failed to read file: {}", e)),
            ),
        };

        let toc = extract_toc(&elements);

        let is_plain_text = false;
        let watcher = FileWatcher::new(&file_path)
            .map_err(|e| eprintln!("Warning: Could not watch file: {e}"))
            .ok();

        Self {
            navigation: NavigationHistory::new(file_path, is_plain_text),
            theme: Theme::Dark,
            renderer: MdRenderer::new(),
            watcher,
            elements,
            content,
            error,
            show_about: false,
            is_plain_text,
            toc,
            icon_texture: None,
        }
    }

    fn reload_file(&mut self) {
        let path = self.navigation.current().clone();
        match std::fs::read_to_string(&path) {
            Ok(new_content) => {
                if new_content != self.content {
                    self.content = new_content;
                    let display_content = if self.is_plain_text {
                        // Wrap plain text in a code block for rendering
                        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                        format!(
                            "# {}\n\n```{ext}\n{}\n```",
                            path.file_name().unwrap_or_default().to_string_lossy(),
                            self.content
                        )
                    } else {
                        self.content.clone()
                    };
                    self.elements = parse_markdown(&display_content);
                    self.toc = extract_toc(&self.elements);
                    self.renderer.image_cache.clear();
                    self.renderer.highlighter.clear_cache();
                }
                self.error = None;
            }
            Err(e) => {
                self.error = Some(format!("Error reading file: {}", e));
            }
        }
    }

    fn reload_and_rewatch(&mut self) {
        self.reload_file();
        let current = self.navigation.current().clone();
        self.watcher = FileWatcher::new(&current)
            .map_err(|e| eprintln!("Warning: Could not watch file: {e}"))
            .ok();
    }

    fn navigate_to(&mut self, path: PathBuf, is_plain_text: bool) {
        self.is_plain_text = is_plain_text;
        self.navigation
            .navigate_to(path, egui::Vec2::ZERO, is_plain_text);
        self.reload_and_rewatch();
    }

    fn base_dir(&self) -> PathBuf {
        self.navigation
            .current()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

impl eframe::App for MdViewApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut pending_navigation = None;

        // Set theme visuals at the start of each frame
        let mut visuals = self.theme.visuals();
        visuals.resize_corner_size = 12.0;
        visuals.faint_bg_color = self.theme.table_stripe_bg();
        ctx.set_visuals(visuals);

        // Increase resize grab radius for easier window resizing
        let mut style = (*ctx.style()).clone();
        style.interaction.resize_grab_radius_side = 8.0;
        style.interaction.resize_grab_radius_corner = 12.0;
        ctx.set_style(style);

        // Check file watcher for changes - drain all pending events
        if let Some(ref watcher) = self.watcher {
            let mut changed = false;
            while watcher.try_recv().is_some() {
                changed = true;
            }
            if changed {
                self.reload_file();
            }
        }

        // Handle keyboard shortcuts
        ctx.input(|i| {
            if i.key_pressed(egui::Key::F5) {
                self.reload_file();
            }
            if i.modifiers.alt
                && i.key_pressed(egui::Key::ArrowLeft)
                && self.navigation.can_go_back()
            {
                pending_navigation = Some(PendingNavigation::Back);
            }
            if i.modifiers.alt
                && i.key_pressed(egui::Key::ArrowRight)
                && self.navigation.can_go_forward()
            {
                pending_navigation = Some(PendingNavigation::Forward);
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::T) {
                self.theme = self.theme.toggle();
                self.renderer.highlighter.clear_cache();
            }
            if i.modifiers.ctrl && i.key_pressed(egui::Key::Q) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });

        // Top panel with toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // TOC dropdown
                if !self.toc.is_empty() {
                    let mut selected_anchor = String::new();
                    egui::ComboBox::from_id_salt("toc")
                        .selected_text("📑 Contents")
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            for (level, text, anchor) in &self.toc {
                                let indent = "  ".repeat((*level as usize).saturating_sub(1));
                                let label = format!("{indent}{text}");
                                if ui.selectable_label(false, &label).clicked() {
                                    selected_anchor = anchor.clone();
                                }
                            }
                        });
                    if !selected_anchor.is_empty() {
                        self.renderer.scroll_target = Some(selected_anchor);
                    }
                }

                ui.separator();

                // Back button
                let back_enabled = self.navigation.can_go_back();
                if ui
                    .add_enabled(back_enabled, egui::Button::new("⬅"))
                    .on_hover_text("Back (Alt+Left)")
                    .clicked()
                {
                    pending_navigation = Some(PendingNavigation::Back);
                }

                // Forward button
                let fwd_enabled = self.navigation.can_go_forward();
                if ui
                    .add_enabled(fwd_enabled, egui::Button::new("➡"))
                    .on_hover_text("Forward (Alt+Right)")
                    .clicked()
                {
                    pending_navigation = Some(PendingNavigation::Forward);
                }

                // Reload button
                if ui.button("🔄").on_hover_text("Reload (F5)").clicked() {
                    self.reload_file();
                }

                ui.separator();

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // About button
                    if ui.button("ℹ️").on_hover_text("About").clicked() {
                        self.show_about = !self.show_about;
                    }

                    // Theme toggle
                    if ui
                        .button(self.theme.icon_label())
                        .on_hover_text("Toggle theme (Ctrl+T)")
                        .clicked()
                    {
                        self.theme = self.theme.toggle();
                        self.renderer.highlighter.clear_cache();
                    }

                    ui.separator();

                    // File path display
                    let path_str = self.navigation.current().display().to_string();
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.add_sized(
                                [ui.available_width(), ui.spacing().interact_size.y],
                                egui::Label::new(egui::RichText::new(&path_str))
                                    .truncate(),
                            );
                        },
                    );
                });
            });
        });

        // About dialog
        if self.show_about {
            // Create icon texture if not yet loaded
            if self.icon_texture.is_none() {
                let icon = crate::create_app_icon();
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [icon.width as usize, icon.height as usize],
                    &icon.rgba,
                );
                self.icon_texture =
                    Some(ctx.load_texture("app-icon", color_image, egui::TextureOptions::LINEAR));
            }

            egui::Window::new("About mdview")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        // Show logo
                        if let Some(ref texture) = self.icon_texture {
                            let size = egui::Vec2::new(64.0, 64.0);
                            ui.image((texture.id(), size));
                            ui.add_space(4.0);
                        }
                        ui.heading("mdview");
                        ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                        ui.add_space(8.0);
                        ui.label("A simple, fast and lightweight standalone Markdown Viewer");
                        ui.label("© 2026 Andreas Daasch");
                        if ui.link("https://github.com/adaasch/MarkDownViewer").clicked() {
                            let _ = open::that("https://github.com/adaasch/MarkDownViewer");
                        }
                        ui.label("License: GPL-3.0");
                        ui.add_space(8.0);
                        ui.label("Keyboard Shortcuts:");
                        egui::Grid::new("shortcuts").show(ui, |ui| {
                            ui.label("F5");
                            ui.label("Reload");
                            ui.end_row();
                            ui.label("Ctrl+T");
                            ui.label("Toggle theme");
                            ui.end_row();
                            ui.label("Ctrl+Q");
                            ui.label("Quit");
                            ui.end_row();
                            ui.label("Alt+←");
                            ui.label("Back");
                            ui.end_row();
                            ui.label("Alt+→");
                            ui.label("Forward");
                            ui.end_row();
                        });
                        ui.add_space(8.0);
                        if ui.button("Close").clicked() {
                            self.show_about = false;
                        }
                    });
                });
        }

        // Central panel with markdown content
        egui::CentralPanel::default().show(ctx, |ui| {
            // Display error if present
            if let Some(ref error) = self.error {
                ui.colored_label(egui::Color32::RED, format!("Error: {}", error));
                return;
            }

            let scroll_output = egui::ScrollArea::both()
                .id_salt(("document-scroll", self.navigation.current()))
                .scroll_offset(self.navigation.current_scroll_offset())
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    // Add some margin
                    ui.add_space(8.0);
                    let base_dir = self.base_dir();
                    let mut actions = self
                        .renderer
                        .render(ui, &self.elements, &self.theme, &base_dir);
                    for action in actions.drain(..) {
                        match action {
                            RenderAction::NavigateMarkdown(path) => {
                                pending_navigation = Some(PendingNavigation::Markdown(path));
                            }
                            RenderAction::NavigateTextFile(path) => {
                                pending_navigation = Some(PendingNavigation::PlainText(path));
                            }
                            RenderAction::OpenExternal(url) => {
                                if let Err(e) = open::that(&url) {
                                    eprintln!("Failed to open URL: {e}");
                                }
                            }
                            RenderAction::ScrollToAnchor(anchor) => {
                                self.renderer.scroll_target = Some(anchor);
                            }
                        }
                    }
                    ui.add_space(16.0);
                });
            self.navigation
                .update_current_scroll_offset(scroll_output.state.offset);
        });

        if let Some(nav) = pending_navigation {
            match nav {
                PendingNavigation::Back => {
                    if let Some((_, _, is_plain_text)) = self.navigation.go_back() {
                        self.is_plain_text = is_plain_text;
                        self.reload_and_rewatch();
                    }
                }
                PendingNavigation::Forward => {
                    if let Some((_, _, is_plain_text)) = self.navigation.go_forward() {
                        self.is_plain_text = is_plain_text;
                        self.reload_and_rewatch();
                    }
                }
                PendingNavigation::Markdown(path) => {
                    self.navigate_to(path, false);
                }
                PendingNavigation::PlainText(path) => {
                    self.navigate_to(path, true);
                }
            }
        }

        // Request repaint periodically for file watcher (only if watcher exists)
        if self.watcher.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }
}
