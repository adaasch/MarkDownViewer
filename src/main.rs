// Desktop entry point. All real logic lives in the `mdview` library crate so
// that the Android `cdylib` build (see `src/lib.rs::android_main`) can reuse
// the same renderer, parser, navigation, theme and app code.
//
// On Android this binary is empty (the `cdylib` output is consumed by the
// Gradle project under `android/`).

#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
    mdview::run_desktop()
}

#[cfg(target_os = "android")]
fn main() {}
