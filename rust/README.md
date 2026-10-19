# FastFill Rust prototype

Experimental cross-platform rewrite. The production C#/WinUI application is
unchanged. See [PLAN.md](PLAN.md) for scope, progress, and checkpoint gates.

Install the pinned toolchain and run routine local checks:

```sh
mise install
mise exec -- cargo fmt --check
mise exec -- cargo clippy --all-targets --all-features -- -D warnings
mise exec -- cargo test --all-features --all-targets
mise exec -- cargo check --target wasm32-unknown-unknown
```

Routine tests and CI skip the expensive 224-frame camera replay. The three
real-frame smoke checks, temporal unit tests, and small replay checks still
run normally. Run the full recording explicitly as documented below.

The small browser download adapter also has a dependency-free check
(Node.js 18+; run in CI):

```sh
node web/download.test.mjs
```

The WASM command needs its standard library once per toolchain:

```sh
mise exec -- rustup target add wasm32-unknown-unknown
```

Run the current native prototype:

```sh
mise exec -- cargo run
```

Debug builds lightly optimize FastFill's own pixel-processing loops (level 1)
and optimize dependencies, including image/PDF codecs, at level 2. Debug
symbols and assertions remain enabled. The first build after this setting
changes is slower; subsequent edits reuse that work. Use
`mise exec -- cargo run --release` to assess release performance.

Measure archive loading/saving with a trusted project:

```sh
mise exec -- cargo run --no-default-features \
  --example project_timing -- form.docscan
```

Serve the same prototype to another device on the LAN:

```sh
mise run web
```

Open the printed `http://<laptop-address>:24800/` URL. The device must be able
to reach TCP port 24800. Trunk watches for changes, but browser auto-reload is
disabled so the generated page never assumes the development machine is
`localhost`; refresh the browser after a rebuild.

For browser performance checks, use full release optimization rather than
the lighter debug settings. This takes longer to build:

```sh
mise exec -- trunk serve --release --address 0.0.0.0 --port 24800 \
  --no-autoreload true
```

The browser remains a touch-test harness; responsiveness still needs tuning.

Checkpoint 6A starts the correction pipeline with **an offline preview
command** (interactive controls are described below). From `rust/`, create
`../artifacts/previews/` if needed, or choose another existing output folder:

```sh
mise exec -- cargo run --no-default-features --example correct_page -- \
  ../tests/samples/sample-form.png ../artifacts/previews/corrected-form.png \
  examples/correction.json
```

The supplied settings crop 4% from each edge, then rotate 90° clockwise.
Open the resulting PNG in an image viewer. Change `rotationQuarterTurns`
to 0 to inspect the unrotated crop, or omit the JSON argument entirely for
an exact full-frame copy of the decoded 8-bit RGBA pixels. Preview PNGs use
8-bit channels; original encoding and metadata are not reproduced. Output
files are created atomically and never overwritten, so choose a new filename
for each run. Original images and projects are untouched.

JSON uses the existing normalized `crop` coordinates (0–1), ordered
`topLeft`, `topRight`, `bottomRight`, `bottomLeft`, and clockwise
`rotationQuarterTurns` (negative counts work too). Arbitrary convex crops use
a four-corner projective transform with alpha-aware bilinear interpolation;
they are not four independent corner adjustments. Nonfinite, crossed,
out-of-image, or numerically unusable crops are rejected. Both input and
output are limited to 50 MP; encoded inputs to 100 MiB and JSON to 16 KiB.
Full-frame correction retains every RGBA pixel and the original dimensions,
unlike the old C# processor's one-pixel reduction. The preview command is
native-only; the correction library also builds for WASM. Use `--release`
before `--no-default-features` for performance measurements.

Checkpoint 6B adds **filters to the same offline preview command**. For
example, use `examples/filter.json` instead of the crop/rotation preset.
It applies Enhanced color with contrast +10 and no geometry changes.
The JSON `filter` object uses the existing project schema:

| `mode` | Filter |
| --- | --- |
| 0 | Original |
| 1 | Enhanced color: gray-world balance and local Lab contrast |
| 2 | Grayscale (BT.601) |
| 3 | Adaptive B&W for ink on unevenly lit paper |

`contrast` ranges from -100 to +100, default 0. It uses linear gain about
128. The top-level `brightness` setting has the same range/default and adds
up to 255 channel levels. Use `examples/brightness.json` for brightness +15,
or combine both adjustments:

```json
{"brightness": 15, "filter": {"mode": 1, "contrast": 10}}
```

Both adjustments run **after** the selected filter, in one lookup:
`(channel - 128) * (1 + contrast / 100) + 128 + 255 * brightness / 100`.
Round and clamp to 0–255 only once, after both adjustments. With brightness
0, negative contrast softens B&W into gray; contrast -100 produces mid-gray.
Brightness can also soften pure B&W, so leave it at 0 for strictly black and
white output. Geometry runs before filtering, so cropped backgrounds do not
affect color balance. Filters preserve alpha and leave fully transparent
pixels unchanged. Source images and projects stay intact. Schema-v2 projects
also persist brightness; the page editor exposes these corrections.

Enhanced color retains the old gray-world + Lab CLAHE approach (8x8 tiles,
clip limit 2). Empty/flat tiles use identity mappings to avoid blowing tiny
or uniform images to white; edge tiles use actual image pixels. B&W retains
the 31-pixel Gaussian neighborhood (sigma 5, offset 12), but intentionally
omits blanket median denoising to preserve single-pixel rules, dotted fields,
and fine print. Camera noise can therefore remain, and thresholding can lose
very faint writing; use grayscale/enhanced when needed. The portable
float/byte implementation is not promised to match native OpenCV pixelwise.
See the [CLAHE reference] and [adaptive threshold reference].

[CLAHE reference]:
  https://docs.opencv.org/4.x/d5/daf/tutorial_py_histogram_equalization.html
[adaptive threshold reference]:
  https://docs.opencv.org/4.x/d7/d4d/tutorial_py_thresholding.html

Checkpoint 6C1 enables **editing corrected projects**. Open `.docscan` pages
with crop, quarter-turn rotation, any color mode, contrast, and brightness.
The canvas, thumbnails, pixel-based 1:1 zoom, and PDF export use the corrected
image aspect. Annotations already refer to that corrected page; loading does
not crop or rotate them again. Save/recovery retain original encoded image
bytes and correction settings, so reopening never applies corrections twice.

Schema-v1 still loads and remains the output format when possible. Nonzero
brightness upgrades output to schema-v2, stored as page-level `brightness`
(-100..100). Zero is omitted; an existing v2 project stays v2. The old C# app
need not read v2. Invalid corrections or unsupported annotation layouts fail
the entire load without replacing the current project. Test with copies.

Checkpoint 6C2 adds **Edit page** to the page rail for image pages:

- Drag the four handles on the original image with mouse or touch. The
  corrected preview updates separately; invalid/crossed crops are rejected.
- **Full frame** restores the entire source image without resetting filters
  or rotation. Rotate left/right changes the corrected image by 90°.
- Choose Original, Enhanced, Grayscale, or B&W. Adjust brightness/contrast
  with sliders or the one-step −/+ buttons. Originals stay untouched.
- **Accept** applies a completed preview matching the current settings.
  **Cancel** or Escape discards the pending page edits. Resizing the window
  or switching focus keeps the draft; document commands stay blocked.
- Annotation centers follow their original image locations. Shapes scale
  width/height independently using local image magnification; stroke width
  and object rotation stay fixed. No annotation perspective warp or skew.
  Text scales by vertical magnification with natural glyph proportions,
  not horizontal stretching. Changes exceeding 6–144 pt text are rejected.
  Font-size labels round to at most one decimal place; underlying scaled
  sizes retain their precision for rendering, editing, and saving.
- Partly visible annotations need no extra move. Wholly displaced items move
  just back within reach, with a warning. Undo restores their old positions.
  The live preview shows the corrected image only; annotations update on
  acceptance. Accepted corrections and relocations form one undo/redo step
  and participate in Save, PDF export, dirty tracking, and native recovery.

Native previews run one background job plus the latest queued settings;
stale results cannot replace a newer preview. The browser harness still runs
correction work on its UI thread. Live/temporal framing and camera capture
remain later checkpoints; Auto frame for existing pages is described below.

Checkpoint 6D2 adds **Smart snap** placement:

- The button appears with **Text**, **Check**, and **Cross**, including while
  an object remains selected. It starts off at launch and remembers its state
  across tools, pages, and projects during that run. It is not a document
  setting and does not affect dirty tracking or undo history.
- Enable it and wait for **Ready**. Analysis uses the corrected page image,
  independent of zoom. Native analysis runs in one background worker;
  switching pages queues only the latest request. Live page results are
  cached, and corrections invalidate the affected result automatically.
- Click or tap near a checkbox to center a check/cross in an inset square
  inside it. Dragging instead keeps manual sizing. Color and thickness stay
  as selected; existing-object moves and resizes do not snap again.
- Click near a solid, dashed, or dotted rule to start text at its left end,
  immediately above it. The text's bottom stays anchored while typing or
  changing font size. Table cells use their own left border where detected.
  Text retains its natural width and explicit newlines; no field auto-wrap.
  Clicking elsewhere on a filled rule reopens text already anchored to that
  field, with the caret at the end, instead of creating a duplicate. Direct
  clicks on text still place the caret near the clicked glyph. Editing existing
  text keeps its position rather than snapping it again.
- **Off**, **No image**, **Analyzing…**, or **Unavailable** means ordinary
  placement. Completed analysis never moves annotations already placed or
  an editor already opened. Toggle off/on to retry a failed analysis.
  Snap placement creates the usual single undo step and saves/exports like
  any other annotation. The browser harness still analyzes on its UI thread
  and may pause briefly when enabling snap or visiting an uncached page.

Checkpoint 6D1 also provides **portable form-feature diagnostics**. From
`rust/`, choose an existing output folder and an unused filename:

```sh
mise exec -- cargo run --no-default-features --example form_probe -- \
  ../tests/samples/sample-form.png ../artifacts/previews/form-candidates.png
```

Open the resulting PNG: blue outlines mark checkboxes; orange lines mark
solid/dashed/dotted rules. JSON on stdout lists normalized candidates and
analysis dimensions; redirect it to a file if desired. Repeat with
`../tests/samples/datum-form.png` for the Dátum label and short dotted fields.
Input/output files are never overwritten. Analysis retains the 1600-pixel
edge and short-axis distance calibration; GUI zoom is not an input.

This is not OCR: table borders are valid rule candidates too. Vertical
dividers connecting two horizontal rules add individual cell candidates;
the nearest-line query prefers the cell under the click over the full row
at the same distance. Full row rules remain available, including merged
rows without dividers. A text stem touching just one rule does not split it.
The portable box check uses straight edge bands and rectangular interiors,
avoiding rounded letter counters without OpenCV. Strongly rotated or
perspective-distorted boxes are outside this slice; analyze corrected,
upright pages. Scaling uses the installed image resampler, not OpenCV's exact
pixels.

Checkpoint 6D3 adds **Ctrl-assisted crop corners** in **Edit page**:

- Drag freely as before. Hold **Ctrl** while dragging a corner to snap to
  nearby image corners. Pressing/releasing Ctrl updates the target without
  moving the pointer; releasing it restores the unsnapped pointer position.
- Hints use original image pixels, regardless of crop, rotation, or filters.
  Analysis runs once in the existing native preview worker and is reused
  for the rest of that editing session. The browser uses its UI thread.
- This is independent of the annotation Smart snap button. No nearby hint,
  failed analysis, or a hint that would cross/collapse the crop leaves normal
  manual dragging available. Cancel/Escape and Accept/Undo work as before.

These are local corner hints, not automatic document framing. The portable
Shi–Tomasi detector retains the C# 1600-pixel analysis edge, 300-feature cap,
0.02 quality threshold, 8-pixel spacing, and 5-pixel window. Resampling and
border handling need not match OpenCV pixel-for-pixel.

Checkpoint 6E1 adds an **offline document-framing diagnostic**. From `rust/`,
choose a photographed PNG/JPEG and a new output filename:

```sh
mise exec -- cargo run --no-default-features --example frame_probe -- \
  photo.jpg framed.png
```

Green shows an accepted document; amber shows the best rejected candidate,
not an automatic crop. JSON on stdout reports normalized ordered corners,
confidence, area ratio, Laplacian sharpness, and analysis dimensions. The
score is a heuristic, not a probability. Input is bounded to 100 MiB/50 MP;
analysis to a 1280-pixel edge. Sources and existing outputs are never replaced.

Related outline and line-pair candidates use the original paper-mask,
edge, geometry, boundary, and continuing-side calibrations. Hough sampling
uses a fixed seed; no native OpenCV library is required. See the
[upstream Hough reference].
Portable resampling, contours, and numeric details can still differ. The
current port uses connected contours, hulls, and enclosed edge regions.
Diagonal edge suppression follows the [native strict-maximum rule].
Low-contrast booklet photos remain rejected; do not lower the threshold to
accept them.
The gallery's minor framing errors and background-related misses are
accepted limitations. No live camera/temporal wiring is present yet.
Background edges can also outrank the correct outline: a speaker or cable
beside a card can pull its right corners outward, even at a high score.
Check every suggestion and adjust corners manually; Ctrl snapping can help.

[upstream Hough reference]:
  https://github.com/opencv/opencv/blob/4.x/modules/imgproc/src/hough.cpp
[native strict-maximum rule]:
  https://github.com/opencv/opencv/blob/4.11.0/modules/imgproc/src/canny.cpp#L585

Checkpoint 6E2 adds **Auto frame** beside **Full frame** in **Edit page**:

- Opening the editor never changes the crop automatically. Press Auto frame
  to propose four corners on the original image and update the corrected
  preview. Rotation, filters, brightness, and contrast stay as selected.
- Native analysis runs in the existing background preview worker and caches
  its result for that editor session, independent of correction settings.
  Auto frame is disabled while another preview/analysis job is running.
  The browser harness still runs this work on its UI thread and may pause.
- Manual review retains the C# 0.45 candidate cutoff, below the detector's
  0.72 automatic acceptance threshold. Weaker suggestions carry an explicit
  **Low-confidence** notice. Always check the frame: sharp, high-contrast
  backgrounds can mislead it. No usable result or an analysis failure leaves
  the current crop intact and manual corner dragging available.
- Manual changes made while analysis runs take priority over its result,
  including Full frame when already full. Once the worker finishes, Auto
  frame can be pressed again to reuse the cached suggestion. Cancel/Escape
  remains available; a late result cannot affect a different editing session.
- Nothing is committed until Accept. It waits for the current completed
  preview. Accepted crop/corrections and image-anchored annotations remain
  one Undo/Redo step and survive Save, reopening, recovery, and PDF export.

Checkpoint 6F1 adds **offline temporal replay**, not a live camera screen:

```sh
mise exec -- cargo run --no-default-features --example frame_replay -- \
  ../artifacts/reviews/cam/frames ../artifacts/previews/my-frame-replay
```

Choose a new output folder whose parent exists. Up to 300 regular PNG/JPEG
files are replayed in filename order, using virtual 33 ms steps (an optional
third argument changes the interval to 25–5000 ms). Use zero-padded names.
The command streams source images, preserves their encoded bytes, refuses
existing destinations, and leaves any partial diagnostics recoverable on
error. Resolution changes reset tracking. Open the generated `index.html`
directly; no server, camera permission, or network connection is needed.

Play/pause, scrub, step frames, toggle raw/stable/prior-hint outlines, inspect
metadata, and jump to the first capture decision if one exists. The circular
countdown and decisions use recorded timestamps, independent of analysis or
playback speed. `replay.json` retains all results for programmatic inspection.
Tracking uses the C# four-frame/two-vote consensus and a **one-second**
countdown, shortened following handheld-use feedback. Confidence, area,
sharpness, and movement limits stay unchanged.
Live adapters must reset both tracker and gate on pause, camera/orientation
changes, and input failures; they must not reuse stale camera results.

The full 224-frame regression retains the original C# geometry and quality
gates, with the user-requested shorter wait. It can take several minutes and
is marked `#[ignore]`, so ordinary tests and CI do not run it. The three
original unhinted smoke checks and fast temporal/capture checks still run
normally. Run the complete sequence explicitly from `rust/`:

```sh
mise exec -- cargo test --no-default-features --test document_detection \
  recorded_laptop_sequence -- --ignored --nocapture
```

Viewfinder screenshots are diagnostic-only: app toolbars/status bars are not
part of live camera frames. Keep genuine camera images as the acceptance
fixtures. Offline replay and repeated-still demos do not validate physical
camera access, permissions, orientation, or capture latency.

Checkpoint 6G1 adds an **opt-in native camera probe**, not GUI capture:

```sh
mise exec -- cargo run --no-default-features --features native-camera \
  --example camera_probe -- list
mise exec -- cargo run --no-default-features --features native-camera \
  --example camera_probe -- formats 0
mise exec -- cargo run --no-default-features --features native-camera \
  --example camera_probe -- capture 0 camera-test.png
```

Run from `rust/`, with other camera apps closed. Use the numeric device ID
printed by `list`; Linux IDs refer to `/dev/videoN`, not list positions.
Some Linux video nodes contain metadata rather than images and will reject
capture. `formats` lists supported modes before starting a stream. Existing
output files are refused, never replaced. No project or annotation is changed.

The default request is exactly **640×480, 30 FPS, MJPG**. There is no silent
resolution/format fallback. If unavailable, use a mode reported by `formats`:

```sh
mise exec -- cargo run --no-default-features --features native-camera \
  --example camera_probe -- capture 0 camera-test-2.png 1280 720 30 YUYV
```

Supported input tokens: `MJPG`, `YUYV`, `NV12`, `RGB3`, `BGR3`, `GREY`.
The probe and GUI use these same tokens on Linux and Windows. Requests are
bounded to 50 MP, 1–120 FPS and 100 MiB decoded RGBA per frame. YUYV needs
even width; NV12 needs even width/height. Unexpected padding/stride is
rejected rather than guessed. After receiving the first frame, the probe
drains frames for at least two seconds, giving auto-exposure time to adjust.
Slow device opening does not count toward this interval. It discards at
least three initial buffers, then reads a fresh frame, stops the stream,
saves PNG, and exits. This is a timed allowance, not stability detection.
It does not crop, rotate, mirror, detect documents, or run auto-capture.
Check brightness, orientation, and colors.

Each operation runs in a child process with a 20-second timeout, so a
normally stalled driver cannot indefinitely block the parent. Ctrl+C stops
the console probe. Missing/busy cameras, denied permissions, unsupported
modes, and decoding errors are failures, not blank placeholder images.
Check OS camera privacy permissions for desktop apps and device access.

Linux uses [rscam's direct V4L2 mode][rscam-native]; Windows uses
[Nokhwa's Media Foundation backend][nokhwa-native]. Linux needs neither
libv4l2 nor Clang; JPEG decoding reuses the existing image library rather
than adding mozjpeg/OpenCV/GStreamer. The native GUI enables `native-camera`
by default; dependency-light commands can still disable default features.
macOS currently returns Unsupported; the browser keeps its existing harness
without native camera dependencies. These adapters remain a hardware spike
until checked on the laptop, then SP6. No camera opens in automated tests.

Manual checkpoint: list devices/formats, capture twice to different files,
verify image dimensions/orientation/colors, then open another camera app to
check release. Retry an existing output path: it must fail without replacing
the file. The GUI checkpoint below can also verify exposure and release.

Checkpoint 6G2 adds **native live preview and manual capture**. Run the app
normally, then choose **Camera** in the page rail (Linux/Windows):

1. Live preview appears as soon as the first frame arrives; startup frames
   may be dark while exposure adjusts. Capture is available immediately;
   there is no forced warm-up. Camera and resolution selection follow the
   6G3b controls below; the original 6G2 checkpoint used 640×480 MJPG.
2. Press **Capture** to freeze the displayed frame. **Retake** restarts the
   preview. **Use photo** adds a page and opens the existing crop, rotation,
   brightness, contrast and color editor. Existing annotations/pages remain.
   Cancel in that editor discards corrections, not the added photo; use
   **Delete page** afterward to remove the photo itself.
3. **Cancel** or Escape discards the camera draft. No page is added before
   Use photo. Moving/resizing the window or switching focus keeps preview
   running. Cancel, Capture and closing the app release the camera.
4. After a camera error, choose another **Camera** or supported mode,
   press **Start preview**, or **Refresh cameras** to discover devices again.

Camera reads run in a child process; a bounded frame queue prevents rendering
backlogs. A 20-second no-frame watchdog stops stalled workers. Capture,
cancellation and app closure terminate the stream; PNG preparation happens
off the UI thread. Driver errors appear on the capture screen, not as blank
successful photos. No camera is opened by automated tests.

macOS camera capture remains unsupported; the Camera entry is absent from
the browser harness. Check brightness, orientation, colors, responsiveness,
retake/cancel/release and a project save/reopen before the next slice.

Checkpoint 6G3a adds **live auto-framing and a one-second auto-capture**:

- A translucent green quadrangle shows the stabilized document frame.
  Keep the whole sheet visible against a contrasting background. The ring
  around Capture fills while the document is steady and sufficiently sharp.
  Manual Capture remains available immediately, without a warm-up delay.
- Auto-capture freezes the actual analyzed image with its matching corners,
  not a newer preview frame. The final crop uses that image's unsmoothed
  corners after the agreement check, avoiding averaging lag during gentle
  movement. Live preview and countdown still use stabilized corners.
  Use photo opens pending corrections with that crop suggested. Check all
  four corners before Accept; Full frame removes clipping. Cancel still
  keeps the newly added, uncorrected photo.
- Retake starts a fresh framing/countdown cycle. Moving the window or
  switching focus does not stop preview. Camera errors, large analysis gaps
  and changed dimensions reset tracking; stale work cannot complete capture.
- Preview and document analysis run independently with one analysis job at
  a time. Extra frames replace the pending preview instead of building a
  CPU queue. Slow/failed framing leaves manual capture available.
- Live analysis uses at most a 640-pixel longest edge at every capture
  resolution. Pixel-sized edge filters and blur checks therefore stay
  comparable when switching to 720p or higher, with less analysis work.
  Movement tolerance remains normalized, not measured in source pixels.
  Captured photos keep their original resolution; the page editor's separate
  Auto frame and offline replay retain the detailed 1280-pixel detector.

Manual captures retain the displayed full image; use Auto frame in the page
editor for a fresh suggestion if needed. Automatic capture is enabled by
default. This slice keeps the existing detector thresholds and replay
calibration; device/mode and lifecycle controls are below.

Checkpoint 6G3b adds **camera selection and higher-resolution capture**:

- **Camera** lists OS-reported device names and indices. Selecting another
  camera stops the old preview, clears tracking and lists its supported modes.
  **Refresh cameras** retries discovery after connecting a device or fixing
  permissions. Linux metadata-only nodes may appear but cannot capture.
- **Preview and photo resolution** lists dimensions, FPS and encoding.
  FastFill initially prefers the largest mode up to full HD (1920×1080),
  at up to 30 FPS, preferring MJPG at that size. Higher advertised resolutions
  remain selectable within the image limits. If full HD is unavailable, the
  displayed selection shows the chosen supported mode; capture failures never
  silently switch to a different resolution or encoding.
- Preview and captured source use the **same selected resolution**. Only
  on-screen display is scaled; the source photo keeps all pixels. This avoids
  camera reopening, exposure delay and crop mismatch for a separate still.
  Changing modes restarts preview and countdown. Retake retains the selection;
  mode/camera choices are locked while a frozen photo awaits Use photo.
- New frames replace the visible image only after their renderer upload is
  ready. The previous frame remains visible meanwhile, preventing black-frame
  flicker at full HD. Keep one frame in flight through upload; Capture freezes
  the displayed pixels, not an incoming image that has not appeared yet.
- Discovery runs off the UI thread in timed driver helpers. Failed, missing
  or busy devices show recoverable errors; canceled/superseded results cannot
  overwrite the current selection. Switching or closing the camera screen
  terminates pending discovery as well as preview. Existing auto-framing and
  manual capture stay available, with no forced exposure wait.
- Discrete Linux modes are listed directly. Continuous size ranges offer
  their valid endpoints and common sizes, including full HD, rather than
  expanding an enormous grid. Only exact integer rates from 1–120 FPS are
  offered; fractional-only rates remain unsupported in this checkpoint.

Manual check: switch cameras if available; capture at 640×480 and full HD
(or the highest supported resolution), check aspect/colors/crop, then Retake.
Use photo, choose Full frame and Accept; save/reopen and verify the retained
source dimensions. Try a busy/unavailable camera, then retry or refresh.
Repeat front/rear camera and full-HD checks on SP6 when available.

Checkpoint 6G3c adds **orientation and explicit pause/resume**:

- **Rotate 90°** cycles through four orientations. Rotation is lossless and
  shared by preview, detection, captured PNG and suggested crop; quarter
  turns swap photo width/height. Window resizing or tablet display rotation
  alone does not change the photo: rotate if the image appears sideways.
- **Mirror preview** flips the camera image and its framing overlay, to
  make positioning paper in a front camera more intuitive. The captured
  review stays mirrored until **Use photo** opens the unmirrored page editor.
  Detection coordinates and saved/exported documents remain **unmirrored**,
  so printed text remains readable. Explicit Front labels in device names
  enable this by default; generic/unknown cameras do not. The adapters expose
  no facing metadata. The checkbox provides a manual override.
- Orientation changes restart live preview and reset the countdown. Retake,
  resolution changes and Resume keep the orientation; selecting a different
  camera resets it. Controls lock while a captured photo awaits Use photo.
  Settings belong to this camera session, not persistent device preferences.
- One shared button shows **Pause** while running, **Resume preview** while
  paused, **Start preview** after an error, and **Retake** after capture.
  Pause releases the camera and clears the old image and countdown.
  Resume explicitly opens a fresh stream. Camera/orientation/mode
  changes while paused never resume it automatically. Refresh cameras can
  discover devices while paused; press Resume afterward.
- The independent driver watchdog detects polling gaps of three seconds or
  more, including system sleep, and clock rollback. It terminates the helper
  on wake and leaves preview paused until Resume. This is an interruption
  fallback, not an OS pre-suspend notification. Brief sleep below that limit
  is not guaranteed to pause. Stale analysis cannot finish the countdown;
  fresh frames are required. Moving/resizing the window or changing focus
  does not itself pause preview. Already captured photos remain available.
- Camera errors include platform guidance. On Windows, check camera access
  and desktop-app camera access in privacy settings. On Linux, check desktop
  privacy controls and your session's access to `/dev/videoN`. Close competing
  camera apps, then retry or Refresh cameras. FastFill does not change device
  permissions; native capture needs no developer license or package identity.

Manual check: rotate a printed form; toggle Mirror preview and check the
live quad follows the mirrored paper. Capture must keep the same mirrored
orientation. Use photo must open an unmirrored page; check Full frame / Accept
and save/reopen. Check all four orientations, Retake and another resolution.
Pause, change settings, and check
that only Resume restarts the stream. Test busy/denied access and recovery.
Sleep testing is deferred: when convenient, sleep with live preview open for
at least five seconds. After wake, stay paused with no old countdown; Resume,
capture, then Cancel/reopen. Repeat front/rear, portrait/landscape and sleep
checks on SP6; laptop/container checks cannot establish its calibration.

[rscam-native]: https://docs.rs/rscam/0.5.5/rscam/
[nokhwa-native]: https://docs.rs/nokhwa/0.10.11/nokhwa/

Checkpoint 5C adds multiple image pages:

- **Add image:** choose a PNG or JPEG. The first image uses the starter blank
  page and asks before replacing its annotations. Subsequent images are
  inserted after the active page. Limit: 20 pages. Native Add/Replace dialogs
  remember the last successful import folder during the current run.
- **Switch page:** click a thumbnail or choose a page number. Pending text is
  accepted, unfinished shape edits are canceled, and the new page fits the
  view. Each page keeps its annotations, selection, and undo/redo history.
  The active tool and its default settings carry across pages.
- **Reorder:** use ↑/↓ or drag any part of a thumbnail. Thumbnails rearrange
  to preview the new order, with a blue frame and separated drop marker.
  Release commits; dropping in empty space below the last page moves it to
  the end. Escape, lost touch, or dropping outside the rail cancels. Scroll
  with the mouse wheel or drag the scrollbar at the rail's right edge.
- **Replace image:** replace the current page image. Annotated pages require
  confirmation. Canceling the picker or choosing an invalid image preserves
  the current page and its history.
- **Delete page:** confirm removal of the current page and its annotations.
  Deleting the last page leaves one blank editable page. Undo/Redo currently
  applies to active-page annotations; page operations cannot yet be undone.

Checkpoint 5D2 connects project files:

- **New / Open:** start a blank project or choose a `.docscan`. Unsaved
  changes prompt Save, Discard, or Cancel. Failed/canceled opens keep the
  current pages and history; successful opens start fresh annotation history.
- **Save / Save as:** on desktop, first save chooses a destination; later Save
  writes that file atomically. Native Save as chooses another destination.
  The button is hidden in WASM, where Save always downloads a copy. Use
  `.docscan` filenames. Original PNG/JPEG bytes remain intact; blank pages save
  as white PNGs. Save accepts pending text and cancels unfinished shape drags.
- **Dirty marker:** `*` beside the filename means unsaved content. Page
  imports, replacement, deletion, reorder, and annotation changes count.
  Selection, navigation, and tool defaults do not. Undo back to the saved
  revision clears the marker; redo restores it.
- **Close:** the native window's close button also guards unsaved changes.
  Save failures/cancellation never continue New, Open, or Close. Escape
  cancels the confirmation; changing focus does not discard it.
- **Shortcuts:** Ctrl+N New, Ctrl+O Open, Ctrl+S Save, Ctrl+Shift+S Save as.
  Use Command instead of Ctrl on macOS. In WASM, Ctrl+Shift+S remains an alias
  for Save/download.

Checkpoint 5E adds **Import PDF** in the page rail:

- Import all pages after the active page, in their original order. An empty
  starter page is replaced; an annotated starter stays. Existing annotations
  and history are preserved. The result becomes an unsaved project change.
- Pages become 300-DPI PNG backgrounds with white paper, respecting PDF crop
  boxes and page rotation. Printed text, PDF annotations, and form fields are
  flattened, not editable PDF objects. Add FastFill annotations normally, then
  save/reopen as `.docscan`. The original PDF is not retained or modified.
- Imports are all-or-nothing. Invalid/unsupported pages, limits, errors, or
  cancellation leave existing pages intact. Limits: 100 MiB input, 20 project
  pages, 50 megapixels/page, 200 megapixels/batch, and 600 MiB encoded output.
- Native rendering runs on a worker thread with parsing, interpreting,
  rendering, and compression stages plus a progress bar. The bar counts work
  stages, not elapsed time. Cancel stops at stage boundaries and discards the
  entire pending batch; it cannot interrupt a stage already running.
  Late results are ignored.
- Image/PDF pickers share their last successful native folder during the run.
  Browser PDF uploads work, but rasterization blocks the browser event thread;
  progress/cancellation cannot respond while rendering. Test large PDFs in the
  native app instead.

The portable PDF backend is [Hayro](https://github.com/LaurenzV/hayro), with
embedded substitute fonts and CMaps, no external renderer installation. It is
experimental: check imported pages visually, particularly complex fonts,
transparency, and PDF form appearances. Renderer-reported content failures
reject the import; not every unsupported PDF construct produces a warning.
Use trusted PDFs: input/output limits do not sandbox internal decompression,
memory use, or CPU time. Password-protected PDFs have no password prompt.

Checkpoint 5F adds **Export PDF** in the top row:

- Export every page in rail order, including the current inline text edit.
  Selection frames, handles, unfinished drawings, and zoom/pan are excluded.
- Pages use the editor's drawing and text engine, flattened at 300 DPI with
  lossless compression and white paper. Text formatting, rotation, layers,
  transparency, and page-edge clipping are retained. PDF text is **not
  searchable or editable**; keep the `.docscan` for later changes.
- PDF page sizes follow the project's 612-point-wide coordinate convention,
  preserving aspect ratio. This does not retain an imported PDF's original
  physical paper size. Use your PDF viewer's fit-to-paper option when printing.
- Native export runs in a worker with page progress and a cancel button.
  Cancel is checked between pages and before file replacement, not within a
  page render/compression. Wait for acknowledgment; a completed file commit
  cannot be undone by a late cancellation. The native save dialog itself must
  be dismissed normally.
- Native destinations must end in `.pdf`. An existing file is replaced only
  after all pages render and temporary-file bytes pass read-back verification.
  Errors or cancellation before commit leave the old file intact. Export
  remembers its own last successful folder during the run.
  Write errors name the chosen destination, not the internal temporary file.
- Export never saves the editable project, clears its unsaved marker, changes
  its `.docscan` filename, or resets page selection/history/navigation.
- Limits: 20 pages, 50 megapixels/page, 200 megapixels/document at 300 DPI,
  and 100 MiB PDF output. Browser export requests a download directly, without
  FastFill's extra "click to download" dialog, with the same event-thread
  blocking limitation as import. The browser may ask where to save or block a
  download according to its settings. A request is not proof of a disk write.

Checkpoint 5G adds **automatic recovery on native desktop**:

- Changed, unsaved projects get a separate atomic `.docscan` snapshot about
  every 10 seconds while no modal file operation is active. Snapshot encoding
  and disk work run in a background worker. Unfinished drawings and open text
  edits postpone snapshots; they are never accepted automatically. Recent
  work since the last completed
  snapshot can still be lost. Keep saving projects explicitly.
- After an abnormal exit, startup offers **Restore**, **Discard**, or
  **Not now**. Restore opens an unsaved copy named `Recovered project.docscan`,
  without the original project's path. Save chooses a destination. Not now
  keeps recovery for a later launch; Discard deletes that offered snapshot.
  Multiple abandoned snapshots are offered newest first; after restoring one,
  the others remain available on a later launch.
- Successful Save, project replacement, and normal close remove the current
  session's obsolete recovery. Canceled/failed file dialogs keep it. Recovery
  never overwrites the explicit project file or clears its unsaved marker.
  File locks isolate running instances and pending recovery choices.
- Locations: `%LOCALAPPDATA%/FastFill/Recovery` on Windows;
  `$XDG_STATE_HOME/fastfill/recovery` on Linux, falling back to
  `$HOME/.local/state/fastfill/recovery`; and
  `$HOME/Library/Application Support/FastFill/Recovery` on macOS.
  Set `FASTFILL_RECOVERY_DIR` to an absolute directory to isolate testing.
  These files contain unencrypted document images and annotations. Newly
  created Unix recovery directories are private to the user. Existing
  directory permissions remain unchanged. C# workspaces are untouched.
- Failures show a persistent recovery warning without blocking editing or
  explicit saves. Failed snapshots keep the preceding valid recovery and
  retry on the next tick. A startup/worker failure disables recovery for that
  run. Cleanup failure before normal close keeps the window open with a
  warning. Invalid recovery archives stay intact unless explicitly discarded.
- Browser recovery is not implemented in this checkpoint. Local browser
  storage is feasible; an IndexedDB follow-up would suit binary project/image
  data without synchronous `localStorage` writes. Browser Save still requests
  a project download; do not rely on closing/reloading the tab to save work.

Project dialogs remember their last successful folder during the run. File
operations block editing until finished. Page geometry, fonts, and strokes use
point units; 1:1 still matches the imported image's original pixels. Saving
does not resize or recompress imported images.

Opening reuses fully validated image pixels for display, keeping the original
encoded bytes for saving. PDF import likewise retains its raster rather than
decoding the new PNG again. Saving fully validates source images once; native
temporary-file read-back verifies exact bytes and ZIP CRC before atomic commit.
No archive thumbnail/schema change is required for these optimizations.

In browsers, Open uploads a file and Save downloads a `.docscan`; no
original path can be reused. Save requests the download immediately,
using the project filename; the browser handles destination/renaming. There
is no extra application download dialog. Completion means FastFill submitted
the download request, not verified disk persistence. Closing/reloading the
browser tab is not guarded; save first. Large archive/image processing can
still pause the browser. These are harness limits, not native behavior.

Editable conversion accepts corrected pages, rigid annotation
rotation/translation, and three font categories. C#'s Segoe UI, Arial, and
Calibri map to Sans; Times New Roman maps to Serif, and Consolas to Monospace.
An imported family name survives ordinary edits until its category changes.
It still rejects affine scaling/shear and unmapped fonts instead of silently
altering them. These are temporary limits, not a permanent compatibility
policy.

Legacy text loads even when its old `maximumWidth` fitting hint is not 1
or is missing (the C# default was 0.4). That value does not prove the text
actually wrapped. Import retains content, explicit newlines, bounds, and
transforms; unchanged annotations keep their legacy metadata when saved.
Rust text does not automatically wrap words: genuinely wrapped legacy text
may flow differently until given explicit newlines. Editing it uses Rust's
normal natural-size text layout. Generic font rendering and inclusive crop
dimensions are not promised to match C# pixelwise. Test projects with copies.

Available annotation tools:

The tool row scrolls horizontally in narrower windows.

- **Rectangle:** drag with the left mouse button or one finger. The preview
  uses the chosen style immediately. Hold Ctrl to draw a square.
- **Oval:** drag like a rectangle. Hold Ctrl to draw a circle.
- **Line:** drag between endpoints. Hold Ctrl to snap to 45° increments. Drag
  the line to move it or either endpoint handle to change that endpoint.
- **Arrow:** behaves like a line, with a stable-size head at its ending
  endpoint. Its head does not distort or grow with line thickness.
- **Check/Cross:** click or tap the intended center to place a small mark. Drag
  outward for a larger mark. Both remain square when resized.
- **Pen:** draw an opaque freehand path. Fill closes and fills the path while
  retaining its outline.
- **Highlighter:** draw a translucent freehand path. Its color and thickness
  are adjustable, but it remains unfilled.
- **Text:** click or tap where typing should start and type directly over the
  page. The editor grows with its content and uses only explicit line breaks;
  it does not wrap at the paper edge. Shift+Enter inserts a line break; Enter
  or clicking elsewhere accepts non-empty text. Escape cancels pending
  changes. Click accepted text with Text to reopen it with the caret nearest
  the clicked glyph (including rotated text). Select clicks select
  text without opening the editor; drag to move it or use its rotation handle.
  Choose Sans, Serif, or Monospace; size; bold; italic; underline; and color.
  Editor changes remain pending until acceptance; changes to inactive selected
  text apply immediately and support undo. Scroll over the size selector to
  step through sizes.
- **Styles:** choose black, white, red, blue, green, or yellow. Use −/+ for
  stroke widths from 1–20 points. Rectangle, Oval, and Pen offer Fill.
  Each tool remembers its own settings; Select changes the chosen object
  with undo.
- **Select:** click/tap an unselected object to select it on release. Drag
  from an unselected object or empty page space to start a marquee instead
  of moving/resizing the object under the initial press.
  A marquee must intersect a line/arrow/ink stroke or a filled pen area,
  not just empty space inside its outer bounds. Bounded shapes and text
  use their actual rotated frames, matching their click-selection behavior.
  Plain empty-space clicks clear selection; a marquee needs at least 4 screen
  pixels of mouse movement or 8 for touch, independent of zoom. A click
  inside overlapping unselected line frames does not select a group.
  Ctrl-click toggles individual members on release. Ctrl+drag forces a fresh
  marquee, replacing selection even when starting over selected objects or
  handles. Drag an already-selected member to move the whole selection;
  a selected member can still move from inside its frame.
  Both single and group selections show a move cursor over movable members.
  Faint member frames identify membership inside the strong group boundary.
  Small objects keep their selection frame but hide resize/rotation handles
  when those would obscure the object or overlap. Hidden handles are inactive;
  drag to move, or zoom in to restore editing controls. Lines/arrows use their
  endpoint distance, so long horizontal/vertical lines remain resizable.
  Shift/Alt force move/resize when controls are available, including with Ctrl.
  In Select mode, press Ctrl after starting a handle drag to constrain its
  resizing, endpoint angle, or rotation; holding Ctrl before the press starts
  a marquee. Escape/focus loss cancels pending selection without an edit.
  With no active edit/gesture, Escape clears selection without changing the
  current tool or document. Style changes then affect the next annotation.
  During text editing/dragging, the first Escape reverts that pending change;
  press Escape again to clear the retained selection. Resizing the window or
  losing focus does not clear an idle selection.
- **Bulk styles:** color applies to every selected object. Thickness applies
  to shapes/ink; fill applies only to rectangles, ovals, and pen paths.
  Selections containing text also show font, size, and B/I/U controls; these
  change only selected text, preserving its other attributes and measuring
  each box separately. Each action is one undo step. Mixed fonts/sizes show
  "Mixed"; B/I/U enable all unless every selected text already has that
  attribute, in which case they disable all. Shape thickness/fill remain
  available beside text controls; scroll the options row in narrow windows.
- **Layers:** To back and To front move selected text or shapes behind or
  ahead of unselected objects while preserving relative order.
- **Rotate:** drag the round handle above selected text, rectangle, oval, mark,
  or ink path. Hold Ctrl to snap to 45° increments. A rotated text editor stays
  horizontal and uses the object's unrotated measured size. Lines and arrows
  remain endpoint-edited. Rotation 0° resets the selected object and supports
  undo.
- Drawing modes can edit the selected object from its body or handles.
  Starting elsewhere draws a new object of the active type.
- **Delete:** remove the selection with the button or Delete key. Delete all
  requires confirmation and can be undone.
- **Undo/Redo:** use the buttons, Ctrl+Z, Ctrl+Y, or Ctrl+Shift+Z. On macOS,
  use Command instead of Ctrl. Canceled and unchanged edits are not recorded.
- **Escape:** cancel an unfinished draw or edit. Switching tools, changing the
  view, losing focus, or losing touch also restores the pending edit.
- **Navigate:** drag or pan/pinch with touch. Right-drag pans in either tool.
  The mouse wheel zooms in every tool while idle; it is ignored during a draw,
  edit, or navigation gesture. Fit, 1:1, and zoom buttons work in every tool.
- Shapes stay attached to the page while zooming, panning, or resizing the
  window. Parts outside the page are clipped, while a small reachable part is
  kept on-page.

Save projects explicitly; native recovery is a safety net, not a saved file.
Schema-v1 archive load/save
APIs are covered by automated compatibility tests. Minimal Linux or WASM
environments without requested generic or italic fonts fall back to bundled
Fira Sans.
Android browser rendering is currently slow; native touchscreen performance
still needs hardware testing.
