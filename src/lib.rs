//! mdview — A simple, fast and lightweight standalone Markdown Viewer.
//!
//! This crate provides the renderer, parser, and app logic used by both the
//! desktop binary (`src/main.rs`) and the Android `cdylib` entry point
//! (`#[no_mangle] fn android_main` in this file).

pub mod navigation;
pub mod links;
pub mod theme;
pub mod highlight;
pub mod parser;
pub mod watcher;
pub mod images;
pub mod renderer;
pub mod app;
pub mod android_paths;

pub use app::MdViewApp;

#[cfg(target_os = "android")]
pub mod android_shim;

#[cfg(target_os = "android")]
pub mod android_io;

#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

pub fn create_app_icon() -> eframe::egui::IconData {
    const APP_ICON: &[u8] = include_bytes!("../assets/app-icon.png");

    if let Ok(image) = image::load_from_memory(APP_ICON) {
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        return eframe::egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        };
    }

    let size = 32;
    let mut rgba = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let idx = (y * size + x) * 4;
            let cx = x as f32 - size as f32 / 2.0;
            let cy = y as f32 - size as f32 / 2.0;
            let r = (cx * cx + cy * cy).sqrt();
            if r < size as f32 / 2.0 - 1.0 {
                rgba[idx] = 0xDE;
                rgba[idx + 1] = 0x56;
                rgba[idx + 2] = 0x16;
                rgba[idx + 3] = 0xFF;
            }
        }
    }
    eframe::egui::IconData {
        rgba,
        width: size as u32,
        height: size as u32,
    }
}

#[cfg(not(target_os = "android"))]
pub fn run_desktop() -> eframe::Result<()> {
    use std::path::PathBuf;
    use clap::Parser;

    #[derive(Parser)]
    #[command(name = "mdview", version, about = "A simple, fast and lightweight standalone Markdown Viewer")]
    struct Cli {
        /// Markdown file to open (opens file picker if not provided)
        file: Option<PathBuf>,
    }

    let cli = Cli::parse();

    let file_arg = match cli.file {
        Some(f) => f,
        None => {
            // Try to show file picker; catch panics from missing display server
            let pick_result = std::panic::catch_unwind(|| {
                rfd::FileDialog::new()
                    .set_title("Open Markdown File")
                    .add_filter("Markdown", &["md", "markdown", "mdown", "mkd", "mkdn"])
                    .add_filter("Text", &["txt", "text"])
                    .add_filter("All Files", &["*"])
                    .pick_file()
            });
            match pick_result {
                Ok(Some(path)) => path,
                Ok(None) => {
                    eprintln!("No file selected.");
                    std::process::exit(0);
                }
                Err(_) => {
                    eprintln!("Error: Could not open file picker dialog.");
                    eprintln!("Usage: mdview <FILE>");
                    eprintln!("  Provide a markdown file path as argument.");
                    std::process::exit(1);
                }
            }
        }
    };

    let file_path = file_arg.canonicalize().unwrap_or_else(|e| {
        eprintln!("Error: Cannot open '{}': {e}", file_arg.display());
        std::process::exit(1);
    });

    let title = format!("mdview - {}", file_path.file_name().unwrap_or_default().to_string_lossy());

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([400.0, 300.0])
            .with_icon(std::sync::Arc::new(create_app_icon())),
        ..Default::default()
    };

    eframe::run_native(
        &title,
        options,
        Box::new(move |cc| {
            configure_fonts(&cc.egui_ctx);
            Ok(Box::new(MdViewApp::new(file_path)))
        }),
    )
}

#[cfg(not(target_os = "android"))]
fn configure_fonts(ctx: &eframe::egui::Context) {
    let mut fonts = eframe::egui::FontDefinitions::default();

    let extra_fonts = [
        (
            "Symbola",
            "/usr/share/fonts/truetype/ancient-scripts/Symbola_hint.ttf",
        ),
        (
            "NotoSansSymbols2",
            "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
        ),
        (
            "NotoSansSymbols",
            "/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf",
        ),
    ];

    for (name, path) in extra_fonts {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                name.to_owned(),
                std::sync::Arc::new(eframe::egui::FontData::from_owned(bytes)),
            );
        }
    }

    for family in [
        eframe::egui::FontFamily::Proportional,
        eframe::egui::FontFamily::Monospace,
    ] {
        if let Some(font_family) = fonts.families.get_mut(&family) {
            for name in ["Symbola", "NotoSansSymbols2", "NotoSansSymbols"] {
                if fonts.font_data.contains_key(name) && !font_family.iter().any(|font| font == name)
                {
                    font_family.push(name.to_owned());
                }
            }

            for name in ["NotoEmoji-Regular", "emoji-icon-font"] {
                if !font_family.iter().any(|font| font == name) {
                    font_family.push(name.to_owned());
                }
            }
        }
    }

    ctx.set_fonts(fonts);
    ctx.options_mut(|options| {
        options.input_options.line_scroll_speed = 72.0;
    });
}

// ---------------------------------------------------------------------------
// Android entry point
// ---------------------------------------------------------------------------

/// Android `cdylib` entry point.
///
/// The Java side (`MainActivity` in `android/app/src/main/java/...`) extends
/// `androidx.gamesdk.gameactivity.GameActivity` which loads `libmdview.so` via
/// `System.loadLibrary("mdview")` and calls this function on a dedicated thread
/// spawned by the GameActivity glue.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(android_app: AndroidApp) {
    // Initialize Android-specific shim that lets the renderer call back into
    // Java to open files via the system file picker, open external URLs and
    // read content URIs.
    android_shim::init(android_app.clone());

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([400.0, 800.0])
            .with_min_inner_size([320.0, 480.0])
            .with_icon(std::sync::Arc::new(create_app_icon())),
        android_app: Some(android_app),
        ..Default::default()
    };

    let title = "mdview";
    if let Err(e) = eframe::run_native(
        title,
        options,
        Box::new(move |cc| {
            configure_fonts_android(&cc.egui_ctx);
            // Read the launching intent's URI (if any) before the renderer
            // spins up so the file is on screen from the first frame.
            //
            // `MainActivity.onCreate` deliberately does *not* also queue this
            // URI for `consume_intent_data`. It used to, which meant the
            // launching document was read, cached and navigated to twice — once
            // here and again on frame 1 — leaving a bogus second entry in the
            // back history.
            let initial_uri = android_shim::intent_data_string();
            let initial_bytes = initial_uri
                .as_ref()
                .and_then(|uri| android_shim::read_uri(uri));
            Ok(Box::new(MdViewApp::android_new(
                cc.egui_ctx.clone(),
                initial_uri,
                initial_bytes,
            )))
        }),
    ) {
        log::error!("eframe::run_native failed: {e:?}");
    }
}

#[cfg(target_os = "android")]
fn configure_fonts_android(ctx: &eframe::egui::Context) {
    // On Android we rely on the system's bundled NotoSans* fonts which are
    // available out of the box; egui's `default_fonts` feature (enabled in
    // Cargo.toml for Android) already wires those in.
    ctx.options_mut(|options| {
        options.input_options.line_scroll_speed = 72.0;
    });
}
