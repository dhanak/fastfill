# 📄 FastFill

[![Build status][build-badge]][build]
[![Development downloads][download-badge]][downloads]
[![Windows 10/11 x64][windows-badge]][windows]
[![Linux x64][linux-badge]][linux] [![MIT license][license-badge]][license]

FastFill scans, corrects, annotates, and exports documents. This cross-platform
app supports touch and mouse, keeps editable `.docscan` projects, and runs on
Windows and Linux. A browser build is available for local testing. These are
development builds; the existing C# release remains separate.

[![FastFill editor with a sample form][editor-shot]][editor-shot]

## 🚀 Download and run

No installer, Rust toolchain, or .NET runtime is needed.

| Platform          | Download                  | Run after extraction              |
| ----------------- | ------------------------- | --------------------------------- |
| Windows 10/11 x64 | [Portable ZIP][windows]   | `FastFill-Rust\FastFill-Rust.exe` |
| Linux x64         | [Portable archive][linux] | `FastFill-Rust/FastFill-Rust`     |

Keep the extracted files together. The public Windows build uses the existing
FastFill self-signed certificate; Windows may still warn or block it. Run only
downloads you trust. The Linux build needs a compatible glibc-based system, a
graphical session, and an XDG desktop portal for file dialogs. See the bundled
`README.txt` for platform diagnostics. Both archives and their source revision
appear on the [development release page][downloads].

## ⭐ Features

- Camera capture on supported Windows/Linux devices, with live framing,
  automatic capture, and a manual shutter.
- Perspective framing, rotation, brightness, contrast, and color filters.
- Image and multipage PDF import, plus reorderable page thumbnails.
- Text, pen, highlighter, shapes, arrows, checkmarks, X marks, and smart form
  snapping.
- Multi-selection, resizing, rotation, layers, undo, and redo.
- Touch pan/pinch zoom and mouse navigation.
- Up to 20 pages per editable `.docscan` project and PDF export.

## ⚡ Quick start

1. Choose **Add image**, **Import PDF**, or **Camera** (Windows/Linux desktop).
2. Use **Edit page** to frame and correct the image.
3. Add text, marks, shapes, or ink with the Tools panel.
4. Save an editable `.docscan` project, then export a PDF to share.

See the [complete user manual][manual] for tools, gestures, corrections,
shortcuts, saving, and recovery. Press **F1** in the app for a shortcut list.

## ♿ Accessibility scope

Touch and mouse editing are supported. Standard controls offer keyboard
navigation, but canvas editing still requires a pointing device and is not yet
optimized for screen readers.

## 🧑‍💻 Development

See the [development guide][development] for local builds, tests, browser
preview, CI packages, and diagnostics. The [implementation plan][plan] tracks
completed work and remaining hardware validation.

## ©️ License

FastFill is available under the [MIT License][license].

[build]: https://github.com/dhanak/fastfill/actions/workflows/rust.yml
[build-badge]: https://github.com/dhanak/fastfill/actions/workflows/rust.yml/badge.svg
[development]: docs/DEVELOPMENT.md
[download-badge]: https://img.shields.io/badge/download-development_build-5865F2
[downloads]: https://github.com/dhanak/fastfill/releases/tag/rust-dev
[editor-shot]: docs/screenshots/workspace.png
[license]: ../LICENSE
[license-badge]: https://img.shields.io/badge/license-MIT-green
[linux]: https://github.com/dhanak/fastfill/releases/download/rust-dev/FastFill-Rust-linux-x64.tar.gz
[linux-badge]: https://img.shields.io/badge/Linux-x64-2DA44E
[manual]: docs/MANUAL.md
[plan]: docs/PLAN.md
[windows]: https://github.com/dhanak/fastfill/releases/download/rust-dev/FastFill-Rust-windows-x64.zip
[windows-badge]: https://img.shields.io/badge/Windows_10%2F11-x64-0078D4
