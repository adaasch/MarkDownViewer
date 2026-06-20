# MarkDownViewer (mdview)

A simple, fast and lightweight standalone Markdown Viewer written in Rust.

## Features

- GitHub Flavored Markdown rendering (tables, task lists, strikethrough)
- Syntax highlighting for 50+ programming languages
- Smart link handling (`.md` files open internally, others open in browser)
- Back/forward navigation between markdown files
- Live reload on file changes
- Light and dark themes
- Inline image rendering

## Usage

```bash
mdview <FILE.md>
```

## Keyboard Shortcuts

| Shortcut     | Action           |
|--------------|------------------|
| Alt+Left     | Navigate back    |
| Alt+Right    | Navigate forward |
| Ctrl+Q       | Quit             |
| F5           | Manual refresh   |
| Ctrl+T       | Toggle theme     |

## Releases

GitHub releases are built by [`.github/workflows/release.yml`](.github/workflows/release.yml).

Create and push a version tag like `v0.1.0` to trigger the pipeline automatically:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow publishes:

- Windows `zip` archives
- macOS `tar.gz` archives for Intel and Apple Silicon
- Linux `tar.gz` archives
- Linux `.deb` packages for Debian and Ubuntu based distros
- Linux `.rpm` packages for Fedora, RHEL, Rocky, AlmaLinux, and openSUSE style distros
- Android `app-release.apk` (universal APK supporting arm64-v8a, armeabi-v7a, x86_64, x86)
- A `SHA256SUMS.txt` file for artifact verification

## Building for Android

The Android port lives in [`android/`](android). To build the APK locally:

```bash
# 1. Install Rust targets
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android

# 2. Install cargo-ndk
cargo install cargo-ndk

# 3. Install Android SDK + NDK
sdkmanager "platform-tools" "platforms;android-34" "build-tools;34.0.0" "ndk;26.3.11579264"

# 4. Build the native libraries
cargo ndk \
    -t arm64-v8a \
    -t armeabi-v7a \
    -t x86_64 \
    -t x86 \
    -P 24 \
    -o android/app/build/intermediates/cargo-ndk-out \
    build --release

# 5. Build the APK
cd android
ANDROID_HOME=/opt/android-sdk JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 ./gradlew :app:assembleRelease

# 6. Install on a device
adb install -r app/build/outputs/apk/release/app-release.apk
adb shell am start -n com.adaasch.mdview/.MainActivity
```

The app uses `androidx.games:games-activity:4.4.0` to host the eframe/egui
rendering surface, and the `jni` crate for the small set of JNI calls
needed to bridge between Rust and Kotlin (file picker, intent data, etc.).

## License

GPLv3 — see [LICENSE](LICENSE) for details.
