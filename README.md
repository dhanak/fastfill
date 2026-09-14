# FastFill

Touch-first Windows 11 document capture, correction, annotation, and PDF
export. Initial hardware target: Microsoft Surface Pro 6.

FastFill keeps projects local. A `.docscan` file is a validated ZIP containing
the JSON edit model and original JPEG/PNG page assets. PDF export flattens the
corrected pages and annotations at 300 DPI.

## Current scope

- Front/back camera capture. Manual capture is default; stable-document auto
  capture is optional.
- OpenCV corner detection, manual four-corner correction, rotation, enhanced
  color, grayscale, black-and-white, and contrast.
- Text, pen, highlighter, lines, arrows, boxes, circles, and checkmarks.
- Selection, move, rotate, scale, recolor, fill, thickness, delete, undo, redo.
- Five-page projects, autosave recovery, editable `.docscan` files.
- Flattened PDF, file picker export to local/USB storage, Windows share sheet.

No OCR, PDF import, cloud sync, SMTP client, or at-rest encryption in v1.

## Fast Linux loop

Only the WinUI shell is Windows-specific. Detection, correction, project I/O,
annotation rendering, and PDF output live in `FastFill.Core` and run in the
Linux container.

```sh
make check
make preview
```

`make check` builds the pinned .NET 10 container, runs framework-free checks,
and compares the rendered page with
`tests/goldens/annotated-page.png`. Preview outputs land in
`artifacts/preview/`.

When an intentional renderer change alters the preview:

```sh
make update-goldens
```

This loop covers document detection, perspective correction, filters,
auto-capture stability, hit testing, undo/redo, hostile archive rejection,
`.docscan` round trips, annotation rendering, and multipage PDF export.

Linux cannot execute WinUI's Windows-only XAML compiler. Every push therefore
runs the same checks on Windows and compiles the WinUI app in GitHub Actions.
Most work should need no SP6 access.

## Windows development

Requirements: Windows 11, Visual Studio with WinUI/C# workloads, and .NET 10.

```powershell
dotnet restore FastFill.slnx
dotnet run --project src/FastFill.Checks -c Release
dotnet build src/FastFill.App -c Debug -p:Platform=x64
```

Run `FastFill.App` from Visual Studio for camera and interactive UI work. Use
JPEG/PNG import for deterministic editor iteration; it exercises the same
review, correction, annotation, project, and export paths as camera capture.

## Signed MSIX

The package identity expects certificate subject
`CN=FastFill Development`. Configure repository secrets:

- `FASTFILL_CERTIFICATE_BASE64`: base64-encoded PFX.
- `FASTFILL_CERTIFICATE_PASSWORD`: PFX password.

The Windows workflow always compiles unsigned. With both secrets present it
also uploads a signed sideload MSIX artifact. Replace the development
publisher and certificate before production distribution.

## Minimal SP6 gate

Use the device only before a release candidate. One short pass:

1. Test both cameras, permission denial, rotation, suspend/resume, and auto
   capture under bright, dim, and glare-heavy light.
2. Drag all crop handles and annotation tools with finger and Surface Pen.
   Verify controls remain usable at 150% and 200% display scaling.
3. Export a five-page PDF, reopen a `.docscan`, share to Mail, and save to a
   USB drive.
4. Check memory, thermal behavior, and capture latency over 20 repeated scans.

Keep calibration thresholds in `AutoCaptureGate` and capture throttling in
`CameraFrameSource`; those are expected to need real-device tuning.
