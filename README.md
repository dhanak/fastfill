# FastFill

Touch-first Windows 11 document capture, correction, annotation, and PDF
export. Initial hardware target: Microsoft Surface Pro 6.

FastFill keeps projects local. A `.docscan` file is a validated ZIP containing
the JSON edit model and original JPEG/PNG page assets. PDF export flattens the
corrected pages and annotations at 300 DPI.

## Current scope

- Windows camera capture with its built-in crop workflow and camera controls.
- Manual four-corner perspective correction, rotation, enhanced color,
  grayscale, black-and-white, and contrast.
- Inline editable text, pen, highlighter, lines, arrows, boxes, ovals,
  checkmarks, and X marks.
- Selection, drag move/resize, rotate, recolor, fill, thickness, delete,
  undo, redo, drag page reorder, pinch zoom, and two-finger pan.
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

This loop covers perspective correction, filters, hit testing, undo/redo,
hostile archive rejection, `.docscan` round trips, annotation rendering, and
multipage PDF export.

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
review, correction, annotation, project, and export paths as Windows camera
capture.

## Portable Windows build

The Windows workflow publishes FastFill as an unpackaged, self-contained x64
folder. It needs no application installation, developer license, .NET runtime,
or Windows App SDK runtime. Extract the ZIP anywhere and run
`FastFill\FastFill.exe`. Keep the other extracted files beside the executable.

Master builds are Authenticode-signed with FastFill's persistent self-signed
development certificate. Trust it once on each test account:

1. Open `FastFill\FastFill.cer` and select **Install Certificate**.
2. Select **Current User**, then place it in **Trusted Root Certification
   Authorities**.
3. Finish the import and run `FastFill.exe`.

Only trust the certificate downloaded from this repository. Self-signing does
not create public SmartScreen reputation, so Windows may still show an
unrecognized-app warning for a downloaded build. If it does, use **More info**,
then **Run anyway**.

Successful `master` builds replace the public `dev` prerelease. Download the
current portable ZIP without signing in:

<https://github.com/dhanak/fastfill/releases/download/dev/FastFill-portable.zip>

## Minimal SP6 gate

Use the device only before a release candidate. One short pass:

1. Test Windows camera capture, camera switching, cancellation, cropping, and
   return to FastFill.
2. Drag all crop handles and annotation tools with finger and Surface Pen.
   Verify controls remain usable at 150% and 200% display scaling.
3. Export a five-page PDF, reopen a `.docscan`, share to Mail, and save to a
   USB drive.
4. Check memory, thermal behavior, and capture latency over 20 repeated scans.
