# MarkDownViewer (mdview)

A simple, fast and lightweight Markdown viewer written in Rust.

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
- A `SHA256SUMS.txt` file for artifact verification

## License

GPLv3 — see [LICENSE](LICENSE) for details.
