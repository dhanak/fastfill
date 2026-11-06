# 📄 FastFill

[![Build status][build-badge]][build]
[![Development downloads][download-badge]][downloads]
[![Windows 10/11 x64][windows-badge]][windows]
[![Linux x64][linux-badge]][linux] [![macOS universal][macos-badge]][macos]
[![MIT license][license-badge]][license]

FastFill scans, corrects, annotates and exports documents. It supports touch and
mouse, keeps editable `.docscan` projects, and runs on Windows, Linux, macOS and
in a browser. Documents stay local unless you explicitly share them.

**[Try FastFill in your browser][browser-demo]** without installing anything.

[![FastFill editor with a sample form][editor-shot]][editor-shot]

## 🚀 Download and run

No installer, compiler or managed runtime is needed.

| Platform                    | Download        | Run                            |
| --------------------------- | --------------- | ------------------------------ |
| Windows 10/11 x64           | [ZIP][windows]  | `FastFill\FastFill.exe`        |
| Linux x64                   | [tar.gz][linux] | `FastFill/fastfill`            |
| macOS Intel / Apple Silicon | [ZIP][macos]    | `FastFill/FastFill.app`        |
| Browser                     | [tar.gz][web]   | Extract into an empty web root |

The table links to the latest successful `master` build on the
[development release][downloads]. [Tagged releases][releases] provide fixed
versions; alpha/beta versions are marked as prereleases.

Keep extracted files together. Linux needs a compatible glibc-based system, a
graphical session and an XDG desktop portal for file dialogs. The macOS app is
ad-hoc signed, not notarized; Gatekeeper may require approval in System Settings
→ Privacy & Security. Camera capture is available on Windows, Linux and in the
browser; macOS camera support is pending. See each package's `README.txt` for
platform details and the [browser deployment guide][web-guide] for static
hosting.

Linux packages include `fastfill.desktop` and an icon for optional
[application-menu installation](docs/LINUX.txt).

## 🔏 Windows signing

Public Windows packages are signed with FastFill's persistent self-signed
certificate and include `FastFill.cer`. To trust it for your Windows account:

1. Open the bundled `FastFill.cer` and choose **Install Certificate**.
2. Select **Current User**.
3. Choose **Trusted Root Certification Authorities** as the destination.
4. Finish the import, then run `FastFill.exe`.

Only trust a certificate from a download you trust. Certificate trust does not
create public SmartScreen reputation or guarantee Smart App Control approval.
Existing installations of the same certificate remain valid. Pull-request and
other-branch CI packages are unsigned.

## ⭐ Features

- Camera capture with live framing, automatic capture and a manual shutter.
- Perspective correction, rotation, brightness, contrast and color filters.
- JPEG, PNG and multipage PDF import, with reorderable annotated thumbnails.
- Text, pen, highlighter, shapes, arrows, checkmarks, X marks and smart
  snapping.
- Multi-selection, resizing, rotation, layers, undo and redo.
- Touch pan/pinch zoom, mouse navigation and a searchable command palette.
- Up to 20 pages per editable `.docscan` project, legacy-project loading and PDF
  export. Native desktop builds also provide automatic recovery.

## ⚡ Quick start

1. Choose **Add image**, **Import PDF** or **Camera** where supported.
2. Use **Edit page** to frame the page and adjust its appearance.
3. Add text, marks, shapes or ink with the Tools panel.
4. Save an editable `.docscan` project, then export a PDF to share.

The [complete manual][manual] covers tools, gestures, shortcuts, saving and
recovery. Press **F1** for shortcuts or **Ctrl+.** (Cmd+. on macOS) to find
commands. Print the [sample form][sample] at actual size to try camera framing
and form snapping.

## ♿ Accessibility scope

Touch and mouse editing are supported. Standard controls offer keyboard
navigation, but canvas editing requires a pointing device and is not yet
optimized for screen readers.

## 🧑‍💻 Development

See the [development guide][development] for builds, tests, diagnostics, signing
and releases. The [project plan][plan] records remaining Mac and Surface Pro 6
validation, including display scaling and physical sleep tests.

## ©️ License

FastFill uses the [MIT License][license]. Bundled icons and browser fonts retain
their own notices; see [font licensing][fonts].

[browser-demo]: https://dev.vidga.hu/fastfill/
[build]: https://github.com/dhanak/fastfill/actions/workflows/build.yml
[build-badge]: https://github.com/dhanak/fastfill/actions/workflows/build.yml/badge.svg
[development]: docs/DEVELOPMENT.md
[download-badge]: https://img.shields.io/badge/download-development_build-5865F2
[downloads]: https://github.com/dhanak/fastfill/releases/tag/dev
[editor-shot]: docs/screenshots/workspace.png
[fonts]: docs/FONTS.md
[license]: LICENSE
[license-badge]: https://img.shields.io/badge/license-MIT-green
[linux]: https://github.com/dhanak/fastfill/releases/download/dev/FastFill-linux-x64.tar.gz
[linux-badge]: https://img.shields.io/badge/Linux-x64-2DA44E
[macos]: https://github.com/dhanak/fastfill/releases/download/dev/FastFill-macos-universal.zip
[macos-badge]: https://img.shields.io/badge/macOS-universal-6E5494
[manual]: docs/MANUAL.md
[plan]: docs/PLAN.md
[releases]: https://github.com/dhanak/fastfill/releases
[sample]: docs/samples/fastfill-camera-sample-form.pdf
[web]: https://github.com/dhanak/fastfill/releases/download/dev/FastFill-web.tar.gz
[web-guide]: docs/WEB.md
[windows]: https://github.com/dhanak/fastfill/releases/download/dev/FastFill-windows-x64.zip
[windows-badge]: https://img.shields.io/badge/Windows_10%2F11-x64-0078D4
