# FastFill

[![Windows build][build-badge]][build]
[![Portable development build][download-badge]][download]
[![Windows 11 x64][windows-badge]][download]
[![MIT license][license-badge]][license]

📄 Scan, correct, annotate, and export documents on Windows 11. FastFill is
designed for touchscreens and initially targets Microsoft Surface Pro 6.

FastFill is under active development. Projects and document images stay on
the local computer unless you explicitly export or share them.

## 🚀 Download and run

**Requirements:** Windows 11 on an x64 computer. No installer, developer
license, .NET runtime, or separate Windows App SDK runtime is required.

1. [Download the latest portable development build][download].
2. Extract the ZIP to any folder.
3. Keep every extracted file together.
4. Run `FastFill\FastFill.exe`.

The development executable uses a persistent self-signed certificate. Trust
it once for each Windows account used to test FastFill:

1. Open `FastFill\FastFill.cer` and select **Install Certificate**.
2. Select **Current User**.
3. Place it in **Trusted Root Certification Authorities**.
4. Finish the import, then run `FastFill.exe`.

Only trust the certificate downloaded from this repository. Self-signing does
not create public SmartScreen reputation. If Windows shows an
unrecognized-app warning, select **More info**, then **Run anyway**.

## Features

- 📷 Front or rear camera capture with stabilized live document framing,
  automatic capture, and a three-second stability countdown. The shutter
  remains available for immediate manual capture.
- Manual four-corner perspective correction, page rotation, contrast,
  enhanced color, grayscale, and black-and-white filters.
- JPEG, PNG, and multi-page PDF import using Windows' built-in PDF renderer.
- Inline editable text with bold, italic, and underline styles; pen,
  highlighter, lines, arrows, boxes, ovals, checkmarks, X marks, and filled
  shapes.
- Move, resize, rotate, recolor, reorder, or delete annotations after placing
  them. Multi-selection, undo, and redo are supported.
- Drag page reordering for projects containing up to five pages.
- 🔍 Full-view, 1:1, and stepped zoom controls. The Navigate tool enables
  pinch zoom and two-finger pan.
- Editable `.docscan` projects with autosave recovery.
- Flattened 300-DPI PDF export to local or USB storage and the Windows share
  sheet for mail and other installed applications.

## Typical workflow

1. Choose **New camera scan**, **Import image / PDF**, **Open project**, or
   **Recover autosave**.
2. Capture or import a page. Review its corners, rotation, color mode, and
   contrast before accepting it.
3. Add annotations. Select an existing object to move, resize, rotate, style,
   reorder, or delete it.
4. Add or import more pages. Drag pages in the left rail to reorder them.
5. Save an editable `.docscan` project or export and share a PDF.

The interface is touch-friendly. A keyboard is only required for text entry
and optional shortcuts.

## Controls and shortcuts

Select **Shortcuts** in FastFill's top bar for the complete reference.

| Shortcut | Function |
| --- | --- |
| **Ctrl+Z** | Undo the last change. |
| **Ctrl+Y** | Redo the last undone change. |
| **Delete** | Delete selected annotations. |
| **Ctrl+click** | Add or remove an object from the selection. |
| **Shift+drag** | Always move the selected object. |
| **Alt+drag** | Resize from the nearest handle. |
| **Ctrl+rotate** | Snap rotation to 45-degree increments. |
| **Ctrl+drag endpoint** | Snap a line or arrow to 45-degree angles. |
| **Ctrl+draw/resize shape** | Constrain a box or oval to a square or circle. |
| **Enter** | Accept text editing. |
| **Shift+Enter** | Insert a line break while editing text. |
| **Escape** | Cancel text editing. |

## 🔒 Local files and privacy

A `.docscan` file is a validated ZIP containing the JSON edit model and the
original JPEG or PNG page assets. Annotations remain editable until PDF
export, which flattens corrected pages and annotations into the final file.

FastFill has no cloud sync or built-in SMTP client. Sharing uses the Windows
share sheet. Projects are not encrypted at rest, so protect sensitive files
with Windows account and disk security.

## Current limits

- Maximum five pages per project.
- No OCR or searchable-PDF generation.
- No cloud synchronization or direct SMTP delivery.
- Development certificate has no public publisher reputation.

---

## Development

The WinUI shell lives in `FastFill.App`. Detection, correction, project I/O,
annotation rendering, and PDF output live in `FastFill.Core`, allowing most
iterations to run in the Linux container without Surface hardware.

### Fast Linux loop

```sh
make check
make preview
```

`make check` builds the pinned .NET 10 container, runs framework-free checks,
and compares the rendered page with `tests/goldens/annotated-page.png`.
Preview outputs land in `artifacts/preview/`.

When an intentional renderer change alters the preview:

```sh
make update-goldens
```

The check covers document detection, perspective correction, filters,
auto-capture stability, hit testing, annotation geometry and ordering,
undo/redo, hostile archive rejection, `.docscan` round trips, annotation
rendering, and multi-page PDF export.

Calibrate detection against captured camera samples without changing the test
suite:

```sh
docker run --rm \
  -v "$PWD/captures:/samples:ro" \
  fastfill-checks --detect /samples/frame.jpg
```

Linux cannot execute WinUI's Windows-only XAML compiler. GitHub Actions runs
the same checks on Windows and compiles the complete WinUI application.

### Windows development

Requirements: Windows 11, Visual Studio with WinUI/C# workloads, and .NET 10.

```powershell
dotnet restore FastFill.slnx
dotnet run --project src/FastFill.Checks -c Release
dotnet build src/FastFill.App -c Debug -p:Platform=x64
```

Run `FastFill.App` from Visual Studio for camera and interactive UI work. Use
JPEG or PNG import for deterministic editor iteration; it follows the same
review, correction, annotation, project, and export paths as camera capture.

### CI and portable releases

The `Windows` workflow checks and publishes FastFill as an unpackaged,
self-contained x64 folder. Successful `master` builds replace the public
`dev` prerelease and include `FastFill.cer` in the portable archive. Pull
request builds create a private workflow artifact but do not publish a
release.

### Minimal SP6 release gate

Use the Surface only before a release candidate. One short pass:

1. Test both cameras, permission denial, rotation, suspend/resume, and
   automatic capture under bright, dim, and glare-heavy light.
2. Drag crop handles and every annotation tool using touch and Surface Pen.
   Verify controls at 150% and 200% display scaling.
3. Import a multi-page PDF, export it, reopen a `.docscan`, share to Mail, and
   save to a USB drive.
4. Check memory, thermal behavior, and capture latency over 20 repeated scans.

Keep calibration thresholds in `AutoCaptureGate` and capture throttling in
`CameraFrameSource`; both require final real-device calibration.

[build]: https://github.com/dhanak/fastfill/actions/workflows/windows.yml
[build-badge]: https://github.com/dhanak/fastfill/actions/workflows/windows.yml/badge.svg
[download]: https://github.com/dhanak/fastfill/releases/download/dev/FastFill-portable.zip
[download-badge]: https://img.shields.io/badge/download-portable_dev-0078D4
[windows-badge]: https://img.shields.io/badge/Windows_11-x64-0078D4
[license]: LICENSE
[license-badge]: https://img.shields.io/badge/license-MIT-green
