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

/// State for the Android-specific file picker. The picker is asynchronous: we
/// launch it via JNI, the user picks a file, and the bridge returns a
/// `content://` URI on a later frame. Until then, the user sees a "Picking
/// file…" placeholder.
#[cfg(target_os = "android")]
#[derive(Default)]
struct PendingFilePick {
    /// True while a file picker is in flight (the user hasn't picked yet).
    in_flight: bool,
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
    /// On Android we may have been launched with a `content://` URI from an
    /// intent. We cache the raw bytes in the app's `getFilesDir()` so the
    /// rest of the renderer (which expects a `PathBuf`) can read them.
    #[cfg(target_os = "android")]
    cached_uri_path: Option<PathBuf>,
    #[cfg(target_os = "android")]
    pending_pick: PendingFilePick,
    /// The granted folder tree URI (from `ACTION_OPEN_DOCUMENT_TREE`). When set,
    /// documents are addressed by their path relative to this tree, which is
    /// what makes links between sibling markdown files resolvable.
    #[cfg(target_os = "android")]
    android_tree_uri: Option<String>,
    /// Markdown/text files discovered in the granted tree (tree-relative
    /// paths), shown in the in-app file browser.
    #[cfg(target_os = "android")]
    android_browser: Vec<String>,
    /// Whether the in-app folder browser is currently shown.
    #[cfg(target_os = "android")]
    show_browser: bool,
    /// True while a folder picker is in flight.
    #[cfg(target_os = "android")]
    pending_folder: bool,
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
            #[cfg(target_os = "android")]
            cached_uri_path: None,
            #[cfg(target_os = "android")]
            pending_pick: PendingFilePick::default(),
            #[cfg(target_os = "android")]
            android_tree_uri: None,
            #[cfg(target_os = "android")]
            android_browser: Vec::new(),
            #[cfg(target_os = "android")]
            show_browser: false,
            #[cfg(target_os = "android")]
            pending_folder: false,
        }
    }

    /// Android-only constructor used by `android_main`. Reads the launching
    /// intent's `data` URI (if any) and shows a "no file" placeholder
    /// otherwise. The user can tap the "📁 Open" button in the toolbar to
    /// launch the system file picker.
    #[cfg(target_os = "android")]
    pub fn android_new(
        _ctx: egui::Context,
        initial_uri: Option<String>,
        initial_bytes: Option<Vec<u8>>,
    ) -> Self {
        // Resolve the initial file. We support three paths:
        //   1. The activity was launched with an intent whose data is a
        //      `content://` URI (file manager, "Open with…", etc.).
        //   2. The activity was launched with an intent whose data is a
        //      `file://` URI (legacy).
        //   3. The activity was launched without any data — show a welcome
        //      placeholder and let the user tap "Open".
        let mut content = String::new();
        let mut elements = Vec::new();
        let mut error: Option<String> = None;
        let navigation: NavigationHistory;
        let is_plain_text = false;

        let placeholder_path = PathBuf::from("(no file)");

        if let Some(uri) = initial_uri {
            // Try the bytes the bridge already read for us, falling back to
            // reading the URI ourselves (file:// URIs work that way).
            let bytes = initial_bytes.or_else(|| {
                crate::android_shim::read_uri(&uri)
            });

            if let Some(bytes) = bytes {
                let display_name = crate::android_shim::display_name(&uri)
                    .unwrap_or_else(|| crate::android_paths::uri_display_name(&uri));
                let fake_path = PathBuf::from(display_name);
                match String::from_utf8(bytes) {
                    Ok(text) => {
                        content = text;
                        elements = parse_markdown(&content);
                    }
                    Err(e) => {
                        error = Some(format!("File is not valid UTF-8: {e}"));
                    }
                }
                navigation = NavigationHistory::new(fake_path, false);
            } else {
                error = Some(format!(
                    "Could not read file from URI: {uri}. Use the Open button to pick a file."
                ));
                navigation = NavigationHistory::new(placeholder_path, false);
            }
        } else {
            navigation = NavigationHistory::new(placeholder_path, false);
        }

        let toc = extract_toc(&elements);
        Self {
            navigation,
            theme: Theme::Dark,
            renderer: MdRenderer::new(),
            watcher: None, // FileWatcher doesn't work on Android
            elements,
            content,
            error,
            show_about: false,
            is_plain_text,
            toc,
            icon_texture: None,
            cached_uri_path: None,
            pending_pick: PendingFilePick::default(),
            android_tree_uri: None,
            android_browser: Vec::new(),
            show_browser: false,
            pending_folder: false,
        }
    }

    fn reload_file(&mut self) {
        let path = self.navigation.current().clone();
        let content_to_load = self.read_path(&path);
        match content_to_load {
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

    /// Read a file's contents as a UTF-8 string.
    ///
    /// On Android this goes through [`crate::android_io`], which knows how to
    /// resolve `content://` URIs, cached absolute paths and folder-tree
    /// relative paths. The image loader uses the same resolver, so a document
    /// and the images it embeds are always addressed the same way.
    #[cfg(target_os = "android")]
    fn read_path(&self, path: &std::path::Path) -> Result<String, String> {
        crate::android_io::read_to_string(path)
    }

    #[cfg(not(target_os = "android"))]
    fn read_path(&self, path: &std::path::Path) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("{e}"))
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

    /// Navigate to a `content://` URI returned by the SAF file picker.
    /// We cache the bytes to the app's private `getFilesDir()` and navigate
    /// to the cached file so the rest of the renderer (which expects a path)
    /// works unchanged.
    #[cfg(target_os = "android")]
    fn navigate_to_uri(&mut self, uri: String) {
        let bytes = match crate::android_shim::read_uri(&uri) {
            Some(b) => b,
            None => {
                self.error = Some(format!("Could not read content URI: {uri}"));
                return;
            }
        };

        let display_name = crate::android_shim::display_name(&uri)
            .unwrap_or_else(|| crate::android_paths::uri_display_name(&uri));
        let safe_name: String = display_name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let cache_name = format!("mdview-{safe_name}");
        let cache_dir = crate::android_shim::files_dir()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("/data/data/eu.io_com.mdview/files"));
        let _ = std::fs::create_dir_all(&cache_dir);
        let cache_path = cache_dir.join(&cache_name);
        if let Err(e) = std::fs::write(&cache_path, &bytes) {
            self.error = Some(format!("Could not cache file: {e}"));
            return;
        }
        self.cached_uri_path = Some(cache_path.clone());
        self.is_plain_text = false;
        self.navigation
            .navigate_to(cache_path, egui::Vec2::ZERO, false);
        self.reload_and_rewatch();
    }

    fn base_dir(&self) -> PathBuf {
        self.navigation
            .current()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Launch the system file picker (Android only). The result is delivered
    /// asynchronously; the next frame will pick it up via
    /// [`Self::drain_android_picker`].
    #[cfg(target_os = "android")]
    fn launch_android_picker(&mut self) {
        if self.pending_pick.in_flight {
            return;
        }
        self.pending_pick.in_flight = true;
        // The Java side launches the picker asynchronously. We poll
        // `pick_file_result` for the result. The poll itself is fast
        // (just a JNI call), so we run it on the render thread via the
        // `update` loop's drain step.
        crate::android_shim::pick_file();
    }

    /// Launch the system folder picker (Android only). Granting a folder lets
    /// the app follow links between sibling markdown files. The result is
    /// drained on a later frame by [`Self::drain_android_folder`].
    #[cfg(target_os = "android")]
    fn launch_android_folder(&mut self) {
        if self.pending_folder {
            return;
        }
        self.pending_folder = true;
        crate::android_shim::pick_folder();
    }

    /// Drain any pending folder-picker result. On success we remember the tree
    /// URI, list the markdown files it contains, and open the in-app browser.
    ///
    /// The bridge reports a cancelled picker as an empty string (see
    /// [`crate::android_shim::PICKER_CANCELLED`]); without that signal a user
    /// who backs out of the picker would leave `pending_folder` stuck at `true`
    /// forever, permanently disabling the button.
    #[cfg(target_os = "android")]
    fn drain_android_folder(&mut self) {
        let Some(tree) = crate::android_shim::pick_folder_result() else {
            return;
        };
        self.pending_folder = false;
        if tree == crate::android_shim::PICKER_CANCELLED {
            return;
        }
        self.android_browser = crate::android_shim::list_tree_markdown(&tree);
        crate::android_io::set_tree_uri(Some(tree.clone()));
        self.android_tree_uri = Some(tree);
        self.show_browser = true;
        self.error = None;
    }

    /// Open a tree-relative file from the in-app folder browser.
    #[cfg(target_os = "android")]
    fn open_tree_file(&mut self, rel: String) {
        let is_plain_text = !crate::android_paths::is_markdown_path(&rel);
        self.show_browser = false;
        self.navigate_to(PathBuf::from(rel), is_plain_text);
    }

    /// Drain any pending file picker result. Called at the top of every
    /// `update` so the user sees the new file as soon as it's picked.
    ///
    /// A cancelled picker comes back as [`crate::android_shim::PICKER_CANCELLED`]
    /// rather than as "no result yet". The two have to be distinguishable: if
    /// cancelling looked like "still waiting", `in_flight` would never clear,
    /// the toolbar would show ⏳ forever and the 200 ms repaint timer below
    /// would keep the GPU awake for the rest of the session.
    #[cfg(target_os = "android")]
    fn drain_android_picker(&mut self) {
        let Some(uri) = crate::android_shim::pick_file_result() else {
            return;
        };
        self.pending_pick.in_flight = false;
        if uri == crate::android_shim::PICKER_CANCELLED {
            return;
        }
        self.navigate_to_uri(uri);
    }

    /// Drain any pending intent data URI (delivered via "open with" intents).
    /// Called at the top of every `update` so the user sees the new file as
    /// soon as the intent is delivered.
    #[cfg(target_os = "android")]
    fn drain_android_intent_data(&mut self) {
        if let Some(uri) = crate::android_shim::consume_intent_data() {
            self.navigate_to_uri(uri);
        }
    }
}

#[cfg(target_os = "android")]
fn open_external(url: &str) {
    // Best-effort: ignore failures (e.g. no browser installed).
    let _ = crate::android_shim::open_external(url);
}

#[cfg(not(target_os = "android"))]
fn open_external(url: &str) {
    let _ = open::that(url);
}

impl eframe::App for MdViewApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut pending_navigation = None;
        // A file chosen in the Android folder browser this frame, opened after
        // the panel closure releases its borrow of `self`.
        #[cfg(target_os = "android")]
        let mut browser_pick: Option<String> = None;

        // Drain any pending Android file picker result
        #[cfg(target_os = "android")]
        self.drain_android_picker();

        // Drain any pending Android folder picker result
        #[cfg(target_os = "android")]
        self.drain_android_folder();

        // Drain any pending intent data URI (delivered via "open with" intents)
        #[cfg(target_os = "android")]
        self.drain_android_intent_data();

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
                // TOC dropdown. On Android the bar is space-constrained, so the
                // button is icon-only and the file name lives inside the
                // dropdown as a header. On desktop the original wider label is
                // kept and the file name stays in the path display on the right.
                if !self.toc.is_empty() {
                    let mut selected_anchor = String::new();

                    #[cfg(target_os = "android")]
                    let toc_label = "📑";
                    #[cfg(not(target_os = "android"))]
                    let toc_label = "📑 Contents";

                    let combo = egui::ComboBox::from_id_salt("toc").selected_text(toc_label);
                    #[cfg(not(target_os = "android"))]
                    let combo = combo.width(200.0);

                    combo.show_ui(ui, |ui| {
                        #[cfg(target_os = "android")]
                        {
                            let file_name = self
                                .navigation
                                .current()
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| {
                                    self.navigation.current().display().to_string()
                                });
                            ui.strong(&file_name);
                            ui.separator();
                        }
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

                // Android-only "Open folder" button. Granting a folder (rather
                // than a single file) is what lets links between markdown files
                // resolve, so it's the primary entry point.
                #[cfg(target_os = "android")]
                {
                    let label = if self.pending_folder { "⏳" } else { "📁" };
                    if ui
                        .button(label)
                        .on_hover_text("Open a folder so links between files work")
                        .clicked()
                    {
                        self.launch_android_folder();
                    }

                    // Once a folder is open, allow reopening the file browser.
                    if self.android_tree_uri.is_some()
                        && ui
                            .button("📂")
                            .on_hover_text("Show files in the opened folder")
                            .clicked()
                    {
                        self.show_browser = true;
                    }

                    // Secondary: open a single file directly. Quick, but links
                    // to other files won't resolve without a granted folder.
                    let file_label = if self.pending_pick.in_flight { "⏳" } else { "📄" };
                    if ui
                        .button(file_label)
                        .on_hover_text("Open a single file (links to other files won't work)")
                        .clicked()
                    {
                        self.launch_android_picker();
                    }
                }

                ui.separator();

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // About button
                    if ui.button("ℹ️").on_hover_text("About").clicked() {
                        self.show_about = !self.show_about;
                    }

                    // Theme toggle. Icon-only on Android to save bar space;
                    // labelled on desktop.
                    #[cfg(target_os = "android")]
                    let theme_label = if matches!(self.theme, Theme::Dark) { "☀" } else { "🌙" };
                    #[cfg(not(target_os = "android"))]
                    let theme_label = self.theme.icon_label();
                    if ui
                        .button(theme_label)
                        .on_hover_text("Toggle theme (Ctrl+T)")
                        .clicked()
                    {
                        self.theme = self.theme.toggle();
                        self.renderer.highlighter.clear_cache();
                    }

                    // File path display (desktop only — on Android the file
                    // name is shown in the TOC dropdown header instead, and the
                    // bar has no room for a full path).
                    #[cfg(not(target_os = "android"))]
                    {
                        ui.separator();

                        let path_str = self.navigation.current().display().to_string();
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.add_sized(
                                    [ui.available_width(), ui.spacing().interact_size.y],
                                    egui::Label::new(egui::RichText::new(&path_str)).truncate(),
                                );
                            },
                        );
                    }
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
                            open_external("https://github.com/adaasch/MarkDownViewer");
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
                #[cfg(target_os = "android")]
                if self.android_tree_uri.is_some()
                    && ui.button("📄 Back to file list").clicked()
                {
                    self.show_browser = true;
                    self.error = None;
                }
                return;
            }

            // Android-only in-app folder browser: lists the markdown files in
            // the granted tree so the user can pick one to open.
            #[cfg(target_os = "android")]
            if self.show_browser {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.heading("📂 Files");
                    ui.label("Pick a file. Links to other files in this folder will work.");
                    ui.add_space(8.0);
                    if self.android_browser.is_empty() {
                        ui.label("No markdown or text files found in this folder.");
                    }
                    for rel in &self.android_browser {
                        if ui.button(rel).clicked() {
                            browser_pick = Some(rel.clone());
                        }
                    }
                });
                return;
            }

            // Android-only empty state: show a friendly welcome with an
            // "Open folder" hint.
            #[cfg(target_os = "android")]
            if self.navigation.current().to_string_lossy() == "(no file)" {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.vertical_centered(|ui| {
                        ui.heading("📄 mdview");
                        ui.add_space(8.0);
                        ui.label("No file open. Tap 📁 Open folder in the toolbar, then pick a markdown file. Links between files in the folder will work.");
                    });
                });
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
                                open_external(&url);
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

        // Open a file chosen in the Android folder browser this frame.
        #[cfg(target_os = "android")]
        if let Some(rel) = browser_pick {
            self.open_tree_file(rel);
        }

        // Request repaint periodically for file watcher (only if watcher exists)
        if self.watcher.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }

        // On Android we also need periodic repaints to drain the file picker.
        #[cfg(target_os = "android")]
        if self.pending_pick.in_flight {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}