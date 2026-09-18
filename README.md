# FastFill

[![Windows build][build-badge]][build]
[![Portable development build][download-badge]][download]
[![Windows 10/11 x64][windows-badge]][release]
[![MIT license][license-badge]][license]

📄 FastFill scans, corrects, annotates, and exports documents on Windows 10
and 11. It works well with touchscreens and mice. Files stay local unless you
explicitly export or share them.

[![FastFill editor][shot-editor]][shot-editor]

## 🚀 Download and run

FastFill requires Windows 10 version 1809 or later on an x64 computer. It
needs no installer, developer license, .NET runtime, or separate Windows App
SDK runtime.

1. [Download the latest portable release][release].
2. Extract the ZIP anywhere and keep all extracted files together.
3. Run `FastFill\FastFill.exe`.

Windows may show an unrecognized-app warning because development releases use
a self-signed certificate. Select **More info**, then **Run anyway**, or follow
the trust instructions in the [user manual][manual].

## Features

- 📷 Front or rear camera capture with stabilized live framing, automatic
  capture, and a visible countdown. Manual capture remains available.
- Perspective correction, corner snapping, rotation, contrast, enhanced
  color, grayscale, and black-and-white filters.
- JPEG, PNG, and multi-page PDF import.
- Inline text, pen, highlighter, lines, arrows, boxes, ovals, checkmarks,
  X marks, filled shapes, and smart form snapping.
- Editable annotations with multi-selection, resizing, rotation, colors,
  layers, undo, and redo.
- Touch pan, pinch zoom, mouse navigation, and full-view or 1:1 controls.
- Up to 20 reorderable pages in an editable `.docscan` project.
- Flattened 300-DPI PDF export and the Windows share sheet.

## Quick start

1. Choose **New camera scan** or **Import image / PDF**.
2. Correct page boundaries, rotation, color, and contrast.
3. Add text, marks, shapes, or ink.
4. Save the editable `.docscan` project.
5. Export a PDF or send it through the Windows share sheet.

See the [complete user manual][manual] for every screen, tool, gesture,
modifier, project action, file format, and privacy detail.

## Development

Core document processing and framework-free checks run on Linux; the WinUI
shell builds on Windows and in GitHub Actions. See the
[development guide][development] for setup, checks, interactive detection
labs, CI releases, and the Surface Pro 6 release gate.

## License

FastFill is available under the [MIT License][license].

[build]: https://github.com/dhanak/fastfill/actions/workflows/windows.yml
[build-badge]: https://github.com/dhanak/fastfill/actions/workflows/windows.yml/badge.svg
[development]: docs/DEVELOPMENT.md
[download]: https://github.com/dhanak/fastfill/releases/download/dev/FastFill-portable.zip
[download-badge]: https://img.shields.io/badge/download-portable_dev-0078D4
[license]: LICENSE
[license-badge]: https://img.shields.io/badge/license-MIT-green
[manual]: docs/USER_GUIDE.md
[release]: https://github.com/dhanak/fastfill/releases/latest/download/FastFill-portable.zip
[shot-editor]: docs/screenshots/editor-shapes.png
[windows-badge]: https://img.shields.io/badge/Windows_10%2F11-x64-0078D4
