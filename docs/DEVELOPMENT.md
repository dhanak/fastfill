# FastFill development guide

[← Back to README](../README.md)

The WinUI shell lives in `FastFill.App`. Detection, correction, project I/O,
annotation rendering, and PDF output live in `FastFill.Core`, allowing most
iterations to run in the Linux container without Surface hardware.

## Fast Linux loop

Enable the repository hooks once after cloning:

```sh
git config core.hooksPath .githooks
```

The pre-push hook rejects unsigned commits newly introduced to a remote.

```sh
make check
make preview
make clean
```

`make check` builds the pinned .NET 10 container, runs framework-free checks,
replays the 224-frame camera regression, and compares the rendered page with
`tests/goldens/annotated-page.png`. Preview outputs land in
`artifacts/preview/`.

`make clean` removes generated `bin/` and `obj/` directories below `src/`,
plus disposable check, preview, and scan-analysis outputs below `artifacts/`.
It preserves captures, test assets, documentation, `artifacts/signing/`, and
`artifacts/.gitkeep`.

The check covers document detection, perspective correction, filters,
recorded-sequence auto-capture stability, hit testing, annotation geometry and
ordering, undo/redo, hostile archive rejection, `.docscan` round trips,
annotation rendering, and multi-page PDF export.

Regenerate the printable camera and annotation sample form with:

```sh
make sample-form
```

The PDF is written to `docs/samples/fastfill-camera-sample-form.pdf`.

## Interactive detection labs

Test production smart-snap and auto-framing logic on Linux:

```sh
make lab
```

Open `http://localhost:5077` and choose a lab. Snap Lab tests text-line,
checkbox, and cross-box placement. Frame Lab tests document framing,
stabilization, and auto-capture timing. Repeating one camera frame is useful
for watching consensus and countdown state advance without hardware.

The cached checks image is rebuilt first. Press `Ctrl+C` once to stop and
remove the lab container. PDFs must first be imported and saved as `.docscan`
for Snap Lab because Linux has no native Windows PDF renderer.

Calibrate detection against captured camera samples without changing the test
suite:

```sh
docker run --rm \
  -v "$PWD/captures:/samples:ro" \
  fastfill-checks --detect /samples/frame.jpg
```

## Golden preview

When an intentional renderer change alters the preview:

```sh
make update-goldens
```

Linux cannot execute WinUI's Windows-only XAML compiler. GitHub Actions runs
the same checks on Windows and compiles the complete WinUI application.

## Windows development

Requirements: Windows 10 or 11, Visual Studio with WinUI/C# workloads, and
.NET 10.

```powershell
dotnet restore FastFill.slnx
dotnet run --project src/FastFill.Checks -c Release
dotnet build src/FastFill.App -c Debug -p:Platform=x64
```

Run `FastFill.App` from Visual Studio for camera and interactive UI work. Use
JPEG or PNG import for deterministic editor iteration; it follows the same
review, correction, annotation, project, and export paths as camera capture.

## CI and portable releases

The `Windows` workflow checks and publishes FastFill as an unpackaged,
self-contained x64 folder. Successful `master` builds replace the public
`dev` prerelease and include `FastFill.cer` in the portable archive. Pull
request builds create a private workflow artifact but do not publish a
release. Pushing a version tag such as `v0.2.0` creates a permanent release
with the same name and signed `FastFill-portable.zip` asset.

## Minimal SP6 release gate

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
