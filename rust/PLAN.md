# FastFill Rust prototype plan

## Scope and working decisions

- Keep the C# application, root build targets, and release workflow unchanged.
- Work under `rust/`. This is a prototype, not a replacement release.
- Target Windows and Linux first. Keep macOS and WebAssembly buildable where
  practical; do not promise camera or filesystem parity in a browser.
- Start with iced 0.14.0. Reconsider it at the interaction/text checkpoint,
  before investing in camera integration or a complete UI.
- Keep geometry and document/edit state independent of iced and native APIs.
  Start with one package containing a library and an optional GUI binary.
- Use explicit interaction states and separate pending edits from committed
  document state. Preserve Escape, undo, and save semantics.
- Keep schema-v1 `.docscan` readable. Schema-v2 output is allowed when it
  adds meaningful capabilities or precision; the old C# app need not read it.
  Until conversion and atomic-save tests pass, do not overwrite projects.
- Reuse existing images, forms, camera sequences, and behavioral checks as
  acceptance fixtures. Do not blindly port the old controller or its bugs.
- Add dependencies only when their checkpoint needs them. Defer OpenCV,
  camera backends, PDF import/output, and platform integration decisions.

## Iteration protocol

1. Pick one small, visible behavior and specify acceptance criteria.
2. Write failing Rust tests first. Record the observed failure when runnable.
3. Implement the minimum behavior; run tests, formatting, and Clippy.
4. Automate platform/build checks and regressions as capabilities arrive.
5. Update this file with changes, test results, limitations, and next step.
6. Hand off a reproducible manual check and stop at each checkpoint. Do not
   silently proceed through the whole rewrite.

Use Rust's built-in test harness; no extra test framework initially. Unit
tests cover deterministic geometry and state changes. Later add file-format
round trips, malformed-input cases, recorded camera replays, and renderer
checks. Automated checks cannot replace touch/DPI/camera calibration.

## Checkpoints

### 1. Isolated scaffold and blank-page viewer — accepted

- [x] Inspect repository layout and available tools.
- [x] Write this plan before implementation.
- [x] Pin a Rust toolchain with mise, scoped to `rust/`.
- [x] Write tests for aspect-correct page fitting and coordinate round trips.
- [x] Implement the tested geometry with no GUI/native dependencies.
- [x] Add a minimal iced window displaying a blank page; no editing yet.
- [x] Add independent Linux/Windows CI and macOS/WASM compile checks.
- [x] Run automated checks and document exact commands/results.
- [x] User checkpoint: launch, resize, and inspect page proportions and text.
      High-DPI behavior remains untested by user request; assume iced's
      logical-pixel scaling is correct until suitable hardware is available.

Completed after user confirmed launch, resize, proportions, and normal-DPI
text. No importing, saving, camera, or annotations were included.

### 2. Image page and navigation — accepted

#### 2A. Image and mouse navigation — accepted

- [x] Import one bounded, validated PNG/JPEG through a cross-platform picker.
- [x] Display the image with Fit, 1:1, zoom buttons, pointer-anchored wheel
      zoom, and left/right-button panning in the navigation tool.
- [x] Test transforms, zoom limits, cursor anchoring, reset, and 1:1 scale.
- [x] User checkpoint: real form, mouse navigation, resizing, and errors.
      Import errors worked but were easy to overlook in the normal status line;
      make errors visually distinct in 2B.

#### 2B. Touch navigation and input isolation — accepted

- [x] Add one-finger pan while the navigation tool is active.
- [x] Add two-finger pan and pinch zoom with stable gesture transitions.
- [x] Prove navigation input is ignored while a drawing tool owns input.
- [x] User checkpoint: gestures through the Android WASM harness. Native
      Windows touch remains deferred until better touchscreen hardware is
      available.

### 3. Editable annotations and history — in progress

- Split into small slices: rectangle; select/move/resize; undo/redo; other
  shapes and endpoint editing; then multi-selection and layers.
- Keep strokes constant when objects resize. Support partial off-page
  placement without making wholly inaccessible objects uneditable.
- Test state transitions, modifiers, hit regions, rotation, and history.
- User checkpoint after each slice, not after all annotation tools.

#### 3A. Draw rectangles — accepted

- [x] Add a tested rectangle draft separate from committed annotations.
- [x] Draw with mouse or one finger, with live preview and Ctrl-to-square.
- [x] Cancel drafts on Escape, lost input, tool changes, or navigation changes.
- [x] Keep geometry attached to the page through zoom, pan, and window resize.
- [x] Clip partial off-page objects when rendering; retain their geometry.
- [x] Confirm replacement of a page containing annotations.
- [x] User checkpoint: drawing, constraints, cancellation, and navigation.

#### 3B. Select, move, resize, and delete — accepted

- [x] Select the topmost rectangle by clicking anywhere inside its box.
- [x] Move by dragging its body; resize freely using four corner handles.
- [x] Keep outlines unchanged when resizing; Ctrl constrains resizing to square.
- [x] Keep touch targets usable at different zooms and for small objects.
- [x] Use Shift to force moving and Alt to force resizing when ambiguous.
- [x] Allow temporary editing of the selected rectangle in Rectangle mode.
- [x] Keep partial off-page placement; prevent wholly inaccessible edits.
- [x] Commit edits on release; cancel on Escape, lost input, or tool/view
      change.
- [x] Delete the selection with a button or Delete key.
- [x] User checkpoint: selection, drag edits, modifiers, and deletion.

#### 3C. Undo and redo — accepted

- [x] Record rectangle creation, movement, resizing, and deletion.
- [x] Keep drafts pending and omit canceled or unchanged edits from history.
- [x] Restore object geometry and selection through undo and redo.
- [x] Clear redo when a new committed change follows undo.
- [x] Add buttons, Ctrl+Z, Ctrl+Y, and Ctrl+Shift+Z.
- [x] Preserve history when page replacement is canceled; clear it after a
      successful replacement.
- [x] User checkpoint: mixed edit sequence, cancellation, and shortcuts.

#### 3D. Oval annotations — accepted

- [x] Generalize rectangle storage to bounded shapes with explicit kinds.
- [x] Draw ovals with mouse or touch; Ctrl constrains them to circles.
- [x] Reuse selection, movement, free resize, deletion, and history behavior.
- [x] Render smooth unfilled oval outlines and clip them at page edges.
- [x] Keep wheel zoom and right-button pan available in Oval mode.
- [x] Split launch, application, page canvas, and picker responsibilities.
- [x] User checkpoint: oval drawing, editing, clipping, and history.

#### 3E. Straight-line annotations — accepted

- [x] Store explicit endpoints instead of deriving lines from a box.
- [x] Draw horizontal, vertical, diagonal, and reverse-direction lines.
- [x] Hold Ctrl while drawing or editing an endpoint to snap to 45°.
- [x] Select and move a line; edit either endpoint without changing its stroke.
- [x] Reuse cancellation, deletion, history, reachability, and navigation.
- [x] Clip rendered lines at page edges.
- [x] User checkpoint: line drawing, endpoint editing, snapping, and history.

#### 3F. Arrow annotations — accepted

- [x] Reuse explicit endpoints and straight-line editing for arrows.
- [x] Keep the arrowhead angle and side lengths independent of box aspect.
- [x] Preserve arrow type through movement, endpoint edits, and history.
- [x] Clip the shaft and arrowhead at page edges.
- [x] Keep zoom, pan, snapping, cancellation, and deletion behavior.
- [x] User checkpoint: arrow directions, arrowhead shape, editing, and history.

#### 3G. Checkmark and cross stamps — accepted

- [x] Place small default marks centered on a click or tap.
- [x] Drag outward from the center to create a larger square mark.
- [x] Keep marks square while resizing, without requiring Ctrl.
- [x] Render checkmark and cross strokes without a surrounding box.
- [x] Reuse movement, history, deletion, clipping, and navigation behavior.
- [x] User checkpoint: placement, sizing, editing, and rendering.

#### 3H. Annotation styles and filled shapes — accepted

- [x] Store color, stroke thickness, and compatible fill state per object.
- [x] Remember independent style defaults for each drawing tool.
- [x] Offer black, white, red, blue, green, and yellow controls.
- [x] Apply 1–20 thickness to outlines, marks, lines, and arrows.
- [x] Fill rectangles and ovals without painting outside their frames.
- [x] Apply compatible style changes to the selected object with undo/redo.
- [x] User checkpoint: style memory, selection updates, fill, and rendering.

#### 3I. Freehand pen and highlighter — accepted

- [x] Capture pointer samples in page coordinates for mouse and touch.
- [x] Add an opaque pen and translucent highlighter with constant thickness.
- [x] Let Fill close and fill pen paths; keep highlighter unfilled.
- [x] Move and resize paths without scaling stroke thickness.
- [x] Reuse clipping, selection, cancellation, deletion, and history.
- [x] User checkpoint: drawing quality, touch input, editing, and fill.

#### 3J. Rotation — accepted

- [x] Add a separate rotation handle for supported bounded objects and ink.
- [x] Keep lines and arrows endpoint-edited without rotation.
- [x] Hold Ctrl to snap rotation to 45-degree increments.
- [x] Add reset-to-zero rotation and preserve rotation through history.
- [x] Keep painted geometry inside its rotated selection frame.
- [x] User checkpoint: handle use, snapping, reset, and rendering.

#### 3K. Multi-selection, layers, and bulk deletion — accepted

- [x] Ctrl-click to toggle objects and drag an empty-area selection rectangle.
- [x] Move selections together and apply compatible style changes in bulk.
- [x] Delete selected objects and delete all with confirmation.
- [x] Send selected objects to back or bring them to front.
- [x] Preserve selection and layer order through undo/redo.
- [x] User checkpoint: mixed-object selection, layers, and bulk operations.

#### 3L. Architecture review before text — accepted

- [x] Audit recent commits for unavoidable cross-layer work versus accidental
      file coupling.
- [x] Separate annotation model, tool catalog, editor operations, and styles.
- [x] Separate application state, update/controller logic, and iced views.
- [x] Separate canvas input, annotation rendering, clipping, and selection UI.
- [x] Drive toolbar order, labels, cursor mode, and style defaults from one
      authoritative tool catalog.
- [x] Keep the platform-independent annotation editor free of iced/native
      dependencies.
- [x] Fix the macOS shortcut regression test to use iced's platform command
      modifier instead of hard-coded Ctrl.
- [x] User checkpoint: smoke-test existing tools before phase 4 starts.

### 4. Text and toolkit decision gate — accepted

#### 4A. Plain inline text lifecycle — accepted

- [x] Place a plain text draft left-aligned at a click or tap.
- [x] Edit directly over the page with a translucent framed input.
- [x] Accept non-empty text with Enter or a click elsewhere.
- [x] Cancel with Escape; keep drafts out of history until accepted.
- [x] Include accepted text in undo, redo, page replacement, and rendering.
- [x] User checkpoint: placement, focus, acceptance, cancellation, and undo.

#### 4B. Multiline editing and measured layout — accepted

- [x] Add Shift+Enter line breaks and content-sized editor bounds.
- [x] Keep the editor stable as text changes and preserve click placement.
- [x] Reopen existing text with Text; select and move it with Select.
- [x] Test focus transitions, measurement, hit testing, and movement.
- [x] User checkpoint: multiline entry, reopening, movement, and layout.

#### 4C. Formatting and rotation — accepted

- [x] Add typeface, size, bold, italic, underline, color, and rotation.
- [x] Keep formatting pending until accepted; Escape restores all changes.
- [x] Size the horizontal editor from unrotated text bounds.
- [x] Expose formatting for inactive selected text and wheel-step font size.
- [x] Include text in shared visual layer order and layer commands.
- [x] User checkpoint: rotated/bold text, touch, and keyboard behavior.

#### 4D. Shared rendering and toolkit decision — accepted

- [x] Extract backend-neutral paths and text layout in page coordinates; make
      the iced preview consume it without changing output.
- [x] User checkpoint: existing annotation preview remains visually unchanged.
- [x] Feed the same layout into a narrow PDF-output probe. Flag text or image
      mismatches rather than hiding them.
- [x] Decide whether iced remains suitable before proceeding.

### 5. Projects, pages, imports, and PDF — accepted

- Incremental slices: safe `.docscan` loading; compatible atomic saving;
  multiple pages/reorder; PDF import; matching PDF export; recovery.
- Keep the current 20-page limit and trust-boundary protections.
- Test C#/Rust cross-reading, archive limits, hostile input, interrupted
  saves, dirty revisions, cancellation, and exported page geometry.
- User checkpoint with copies of existing projects, never originals first.

#### 5A. Safe schema-v1 project loading — accepted

- [x] Mirror the C# manifest without converting or dropping edit data.
- [x] Bound ZIP entries, manifest size, expanded size, images, pages,
      annotations, ink points, and text length before use.
- [x] Reject unsafe paths, duplicate entries/IDs, unknown JSON fields,
      malformed geometry, missing assets, and image metadata mismatches.
- [x] Load a synthetic C#-compatible archive and the available review
      projects through the same public API.
- [x] Keep UI opening and Rust annotation conversion for a later slice.

#### 5B. Compatible atomic project saving — accepted

- [x] Serialize schema-v1 manifests and referenced page assets without
      dropping edit data.
- [x] Validate generated archives before replacing an existing project.
- [x] Commit native files atomically in the destination directory on Windows,
      Linux, and macOS; preserve the old file on every pre-commit failure.
- [x] Keep the shared archive writer buildable for WebAssembly.
- [x] Round-trip synthetic and real C# projects through load-save-load tests.

#### 5C. Multiple image pages and reordering — accepted

- [x] Own image, annotations, selection, and annotation undo/redo per page.
- [x] Add images after the active page, with a 20-page limit and safe picker
      cancellation/error handling.
- [x] Switch pages from a thumbnail rail or native page selector; accept
      pending text, cancel unfinished shape edits, and fit the selected page.
- [x] Keep tool and default style settings across page switches.
- [x] Reorder with buttons or whole-thumbnail mouse/touch dragging; preview
      the order, accept end-of-list drops, and keep scrolling separate.
- [x] Confirm page deletion and annotated image replacement; keep a blank
      editable page after deleting the last page.
- [x] Pass native/core tests, strict lint, and WebAssembly checks.
- [x] User checkpoint: imports, independent history, switches, reorder,
      replacement/deletion, page limit, and touch where available.

Project open/save UI and editable conversion are accepted. Continue with PDF
import, matching PDF export, then recovery as separate checkpoints.

#### 5D1. Editable schema bridge and save-state identity — accepted

- [x] Give annotations stable IDs through drafts, edits, layers, and history.
- [x] Track committed content revisions without counting selection or defaults.
- [x] Convert full-frame, uncorrected pages to the C# 612-point coordinate
      system, preserving layer order, styles, alpha, IDs, and rigid rotations.
- [x] Preserve untouched schema metadata and reject unsupported crops,
      filters, affine distortion, and text layout explicitly.
- [x] Round-trip all current tools through editor-schema-editor and ZIP tests.
- [x] Keep rendering and WebAssembly checks green; GUI behavior unchanged.
- [x] Host checkpoint: all-feature tests passed; no new manual UI behavior.

5D2 connects this bridge to open/save dialogs and dirty-document confirmation.
It also moves imported page geometry to point units while preserving image
pixel-based 1:1 navigation. Keep this foundation separate from the UI
checkpoint.

612 points is a coordinate-unit convention (8.5 inches at 72 points/inch), not
an image resize. Original image bytes and dimensions remain unchanged. Edited
geometry may accumulate small `f32` round-off; unchanged schema objects retain
their original values exactly. Keep schema-v1 output for this checkpoint;
introduce v2 only when a concrete requirement justifies it.

#### 5D2. Project UI, dirty guards, and point geometry — accepted

- [x] Add New, Open, Save, Save as, and standard command-key shortcuts.
- [x] Load editable v1 pages transactionally; keep current data on failure.
- [x] Snapshot saves and use the atomic writer; cancel/failure stays dirty.
- [x] Track page identity/order and annotation revisions against the last
      save.
- [x] Guard new/open/window close with Save, Discard, and Cancel.
- [x] Accept pending text, cancel shape drafts, and block editing during I/O.
- [x] Use point units consistently, retaining image-pixel 1:1 navigation.
- [x] Keep WASM open/download-save buildable; document browser limitations.
- [x] Test file/geometry paths, archive round trips, and existing rendering.
- [x] User checkpoint: native dialogs, dirty guard choices, reopen fidelity,
      image 1:1, and browser upload/download where available. See checklist
      below.

#### 5E. Transactional multi-page PDF import — accepted

- [x] Evaluate a portable renderer against synthetic and real form PDFs.
- [x] Rasterize at 300 DPI with white backgrounds and crop/rotation support.
- [x] Bound input bytes, page count, output dimensions, and batch size.
- [x] Add Import PDF after the active page; replace only an empty starter.
- [x] Stage the whole batch before changing pages, selection, or history.
- [x] Show native progress, cancel between pages, and ignore late results.
- [x] Preserve annotations on existing pages and dirty the imported document.
- [x] Verify save/reopen, native/WASM builds, errors, limits, and cancellation.
- [x] User checkpoint: PDF appearance, insertion, limits, cancel, save/reopen.
      Stop before PDF export or recovery.

Performance/progress, mixed text/shape selection, and bulk font controls are
also accepted. See the follow-up logs below. Matching PDF export is next;
keep recovery separate.

#### 5F. Matching PDF export — accepted

- [x] Flatten committed pages at 300 DPI using the editor's renderer,
      preserving order, images, formatted text, layers, alpha, and clipping.
- [x] Exclude selection handles, drafts, navigation, and other UI overlays.
- [x] Export all pages through a PDF save dialog, with native worker progress
      and cancellation; replace native destinations atomically.
- [x] Accept pending text without marking the project saved or changing its
      `.docscan` path, page selection, history, or navigation.
- [x] Bound page dimensions and total raster work before allocating pixels.
- [x] Test PDF round trips, rendering fidelity, cancellation, failures, and
      native/core/WASM builds. Document flattened/non-searchable output.
- [x] User checkpoint: export, view/print, page order, appearance, overwrite,
      cancel, dirty state, and browser download where available.

Stop at the export checkpoint. Recovery remains a separate slice.

#### 5G. Automatic recovery — accepted

- [x] Test native, atomic recovery storage and concurrent-instance isolation.
- [x] Snapshot committed dirty work every 10 seconds in a background worker.
      Skip pending text/drawing edits; never accept them automatically.
- [x] Offer startup Restore/Discard/Not now; restore into an unsaved copy,
      never the original project's path. Keep snapshots until saved or
      discarded.
- [x] Serialize writes and cleanup so late completion cannot resurrect old
      recovery after Save, New, Open, or normal exit.
- [x] Document per-user paths, failure reporting, and browser limitations.
- [x] Run automated checks; stop for native crash/relaunch manual testing.
- [x] User checkpoint: crash recovery, deferred restore, Save/Discard cleanup,
      and concurrent instances. All requested checks passed.

Native locations: `%LOCALAPPDATA%/FastFill/Recovery` on Windows,
`$XDG_STATE_HOME/fastfill/recovery` (fallback
`$HOME/.local/state/fastfill/recovery`) on Linux, and
`$HOME/Library/Application Support/FastFill/Recovery` on macOS.
`FASTFILL_RECOVERY_DIR` overrides the directory for isolated testing.
Browser recovery is deferred; browser Save still downloads a project.
User suggested browser-local recovery: record IndexedDB as a follow-up for
binary project/image data, rather than synchronous, string-only localStorage.
Handle quota/eviction, private browsing, and multiple live tabs explicitly.
See [IndexedDB](https://developer.mozilla.org/en-US/docs/Web/API/IndexedDB_API).

### 6. Correction, snapping, and camera — accepted

- Port image correction and form snapping, retaining calibrated fixtures.
- Port document detection and temporal capture gating with all 224 frames.
- Add native camera adapters only after recorded-image tests pass.
- Keep blocking native work outside UI updates. Browser implementations may
  use different adapters; avoid unconditional native-only dependencies.
- User checkpoints: lab parity first, then laptop camera, then SP6 cameras,
  permissions, orientation, countdown, cancellation, and suspend/resume.

Accepted on available laptop hardware. Physical sleep testing and actual
Windows/SP6 front/rear calibration remain deferred; do not mark them passed.

#### 6A. Correction geometry and offline preview — accepted

- [x] Test perspective correction and quarter-turn rotation independently of
      the GUI. Reuse the current crop schema and image dependency.
- [x] Validate source/crop/output limits before allocating output pixels;
      reject crossed, nonfinite, or numerically unusable quadrangles.
- [x] Preserve full-frame dimensions and pixels exactly. Treat normalized
      corners as pixel centers; include both endpoints in output dimensions.
- [x] Add a small native PNG preview command using optional JSON correction
      settings. Bound reads and refuse existing output files.
- [x] Test synthetic geometry, real sample forms, alpha interpolation, and
      command parsing. Keep native/core/WASM builds green.
- [x] User checkpoint: inspect generated crops/rotation. Main application
      behavior and corrected-project editing remain unchanged in this slice.

#### 6B. Portable image filters — accepted

- [x] Test grayscale, contrast, enhanced color, and adaptive B&W first.
- [x] Reuse schema-v1 filter settings and validate contrast before allocation.
      Apply filters after perspective correction/rotation, without modifying
      source pixels, dimensions, alpha, or explicit project files.
- [x] Retain gray-world balance, Lab CLAHE (8x8 tiles, clip 2), and Gaussian
      B&W calibration (31-pixel window, sigma 5, offset 12). Avoid blanket
      median blur that erases fine rules/dots; test that intentional change.
- [x] Reuse installed image primitives where semantics match. Add no native
      OpenCV dependency; keep core/native/WASM checks green.
- [x] Extend the existing preview command and document numeric filter modes.
- [x] User checkpoint: accept generated form/camera filter comparisons.
- [x] Add brightness -100..100, default 0, after the selected color mode.
      Combine with contrast in one clamped lookup; test limits, ordering,
      alpha/source preservation, JSON defaults, and invalid-input safety.
- [x] User checkpoint: accept the added brightness comparisons.
      Continue separately with page editing and corrected-project I/O in 6C.

Follow-up slices, each with its own checkpoint:

- **6C — Page editor:** original-image corner adjustment, full frame,
  rotation, live filtered preview, Accept/Cancel, and corrected-project I/O.
  Persist brightness in schema-v2 projects while retaining schema-v1 loading.
  Specify annotation behavior and keep displaced objects accessible, with a
  warning when moved. Apply changes transactionally; keep originals intact.
- **6D — Form snapping:** port line/box detection against existing form
  fixtures, then connect global smart-snap placement and Ctrl corner snapping.
- **6E — Document detection:** port still-image framing against calibrated
  fixtures before using any live camera. Do not recalibrate by convenience.
- **6F — Temporal framing:** replay all 224 laptop-camera frames, with stable
  overlay/countdown/capture decisions and recorded-sequence regressions.
- **6G — Native capture:** evaluate camera backends, then add device switch,
  live overlay, manual/automatic capture, permissions, and lifecycle handling.
  Test laptop first, SP6 next; defer unsupported browser camera behavior.

##### 6C1. Corrected-project compatibility — accepted

- [x] Test loading/editing crop, rotation, and filter metadata against the
      corrected image geometry, without moving annotations during loading.
- [x] Add schema-v2 brightness persistence; continue loading schema-v1 and
      preserve v1 output unless a new feature requires v2.
- [x] Stage corrected display pixels outside UI updates, retain original
      encoded assets for Save/recovery, and reuse decoded input pixels.
- [x] Test shared assets with different corrections, round trips, failures,
      display/PDF geometry, and source preservation.
- [x] User checkpoint: open corrected fixtures, edit annotations, Save as,
      reopen, and export. No interactive page-correction controls yet.

##### 6C2. Interactive page editor — accepted

- [x] Original-image mouse/touch corner dragging, full frame, quarter-turn
      rotation, color mode, brightness, and contrast with corrected preview.
- [x] Transactional Accept/Cancel and stale-preview protection; explicitly
      test cancellation and undo. Pending edits do not change page identity.
- [x] Include accepted corrections in dirty tracking, undo/redo, Save, PDF,
      and recovery snapshots. Retain original encoded source assets.
- [x] Follow image-space annotation centers, not fixed page-point positions.
      Apply independent X/Y local magnification as clarified by the user.
      Preserve stroke width and object rotation; do not warp/skew objects.
      Text follows vertical font scaling with natural glyph proportions.
      Reject unsupported resulting font sizes rather than silently clamp.
- [x] After mapping, keep partly visible objects in place. Translate only
      wholly displaced objects back into reach and warn on acceptance.
- [x] Coalesce preview work to one worker plus the latest settings. Reject
      stale completions; Accept requires the current settings' preview.
- [x] User confirms page editing, image-space annotation anchors, undo/redo,
      export, and displaced-annotation warnings. Physical touch untested.
- [x] User accepts compact scaled-font labels and near-instant debug
      corrections on 2026-10-16. Proceed separately to form snapping in 6D.

The old C# full-frame processor loses one pixel in each dimension. Preserve
the Rust viewer's exact full-frame dimensions instead; document this intended
difference rather than porting the off-by-one into the new pipeline.

##### 6D1. Portable form-feature detection — accepted

- [x] Port line/checkbox analysis and nearest-feature queries independently
      of GUI state, zoom, or annotation coordinates. Retain the calibrated
      1600-pixel analysis edge, line/mark/gap thresholds, and short-axis
      distance metric. Avoid a native OpenCV dependency.
- [x] Cover solid/dashed/dotted lines, adjacent labels, separate same-row
      fields, small boxes inside bordered forms, transparency, bad queries,
      and the existing sample-form/Hungarian Dátum fixtures.
- [x] Provide a bounded native diagnostic producing candidate overlays and
      JSON; compare against the existing Snap Lab before placement wiring.
- [x] User confirms legacy project opening and baseline line/box detection
      matches the C# implementation. Retain valid table/box top borders.
- [x] Add table-cell candidates at vertical dividers without removing full
      row rules. Cover stacked cells, merged rows, and stray text stems.
- [x] User confirms crowded-area click selection improves with drag slop.
- [x] User approves click-vs-drag rules and Ctrl-forced marquee gestures.
- [x] User confirms geometry-aware marquees and new gesture rules work.
- [x] User accepts group move cursors and all 6D1 follow-ups. Mark this
      slice complete and proceed to 6D2 on 2026-10-16; no commit requested.

##### 6D2. Smart-snap annotation placement — accepted

- [x] Keep a session-global, default-off Smart snap toggle for Text, Check,
      and Cross; retain it across tools/pages/projects, outside document data.
- [x] Cache calibrated features by corrected-display identity. Run one
      native worker at a time, coalesce to the current page, reject stale
      project/image completions, and bound the cache to live page images.
- [x] Snap clicked/tapped marks to a centered 78% inset square in detected
      boxes. Keep manual drag sizing and edits of existing objects intact.
- [x] Start new text at a nearby rule's left end, bottom-anchored above it.
      Keep natural widths, explicit newlines, and existing-text editing.
- [x] Show pending/failed analysis; use ordinary placement until ready,
      without asynchronous changes to annotations already created.
- [x] Test disabled/fallback snap, zoom independence, tools/pages, canceled
      gestures/stale workers, corrections/undo, style preservation, one-step
      history, save/reload, and native/core/WASM builds.
- [x] User accepts smart-snap placement on 2026-10-16. Address the separate
      caret, small-object controls, and Escape observations before 6D3.

##### 6D3. Ctrl-assisted crop corners — accepted

- [x] Add portable grayscale corner features with the C# calibration:
      1600-pixel analysis edge, up to 300 features, quality 0.02, minimum
      spacing 8 analysis pixels, and a 5-pixel structure-tensor window.
- [x] Analyze original pixels once in the existing native preview worker.
      Reuse results across crop, filter, and rotation changes; keep manual
      editing available if analysis is unavailable. No global snap toggle.
- [x] Snap only while Ctrl is held during a corner drag. Restore raw pointer
      placement immediately when Ctrl is released, without moving the mouse.
      Retain the calibrated 52-screen-pixel radius, clamped to 0.015–0.09
      of the displayed image's shorter side. Reject invalid snapped quads.
- [x] Test original/preview coordinates, letterboxing, modifier changes,
      convexity, cancellation, pointer ownership, lost touch, and stale jobs.
      Cover bounded analysis, real forms, transparency, and native/core/WASM.
- [x] User confirms Ctrl snapping, free dragging, and page-edit behavior
      on 2026-10-16. Continue with still-image document detection in 6E.

##### 6E1. Offline document detector and diagnostics — accepted

- [x] Port bounded still-image analysis and related outline/line candidates
      before GUI wiring. Keep the 1280-pixel edge, two Canny threshold pairs,
      paper-mask thresholds/morphology, polygon simplification, geometry,
      seeded progressive Hough, paper/Lab boundaries, continuing-side checks,
      and the C# 0.72 acceptance threshold. Add no native dependency.
- [x] Add a native probe with normalized JSON and candidate/accepted overlays;
      preserve source files and reject invalid/oversized inputs and outputs.
- [x] Cover synthetic perspective documents, distracting background lines,
      printed forms without outer boundaries, transparency, scale, and a
      representative subset of the committed laptop-camera recording.
- [x] Record gaps against the full C# detector rather than weaken tolerances
      or present this connected-outline checkpoint as complete lab parity.
- [x] User accepts the gallery on 2026-10-16, including minor errors and known
      misses against misleading high-contrast backgrounds. Retain these as
      documented limitations rather than block GUI integration on parity.

##### 6E2. Auto frame in Edit page — accepted

- [x] Record user acceptance of current detector limits. Defer further
      contour/pixel parity work; do not change accepted thresholds/fixtures.
- [x] Add explicit Auto frame alongside Full frame. Analyze original pixels
      lazily in the existing native worker, caching the result per session.
      Opening Edit page must not detect or alter existing crops.
- [x] Use the C# manual-review cutoff 0.45 for rejected candidates; label
      weaker suggestions. No usable result/failure must preserve the crop
      and manual editing. Keep tone/rotation settings and original bytes.
- [x] Reject stale sessions/results and let intervening manual edits win,
      including Full frame when already full. Block stale Accept during
      framing, keep Cancel/Escape available, and avoid overlapping workers.
- [x] Cover cache reuse, errors, races, current preview, history/annotations,
      save/reload, and native/core/WASM checks.
- [x] User confirms Auto frame, manual override, correction settings, and
      Cancel/Accept/Undo/Redo on 2026-10-16. Retain the documented 11-19
      background-edge limitation after a small scoring experiment regresses
      an existing fixture. Stop before temporal/countdown work in 6F.

##### 6F1. Temporal core and replay diagnostics — accepted

- [x] Port four-frame/two-vote consensus, 0.08 agreement, coordinate medians,
      0.35 smoothing, and calibrated previous-frame ranking hints. Keep hint
      bonuses separate from confidence and do not alter unhinted detection.
- [x] Port the countdown with a one-second handheld wait, 0.04 drift limit,
      confidence 0.72, area 0.1, and laptop-frame sharpness 25. Emit one Ready
      decision per capture cycle; reset on loss, blur, or explicit restart.
- [x] Cover invalid values, rejected candidates, jitter, changing majorities,
      cumulative drift, clock rollback, reset, and current-frame sharpness.
- [x] Replay all 224 committed laptop-camera frames in virtual 33 ms steps.
      Retain strict geometry/quality gates. Track the initially exposed
      detector-parity failure and its resolution separately in 6F2.
- [x] Add bounded offline replay diagnostics with raw/stable overlays,
      playback/scrubbing, countdown progress, and the capture event. Preserve
      source images and reject existing destinations; no live camera yet.
- [x] User accepts the restored detector and one-second replay on 2026-10-16.
      Stop before native capture in 6G; offline replay does not validate
      physical camera adapters.

##### 6F2. Full-recording detector parity — accepted

- [x] Restore connected contour candidates and strict diagonal edge maxima.
      User clarifies viewfinder screenshots contain app chrome absent from
      actual camera pixels; do not treat that status-bar crop as a blocking
      regression. Keep genuine camera fixtures and generic clutter checks.
- [x] Pass the full-recording test: 224 frames, raw
      correct >= 160, stable correct >= 180, wrong = 0 for both, and a
      correctly framed capture. Keep 0.1 normalized corner tolerance, image
      quality/geometry calibration, and three unhinted smoke checks. Only
      countdown duration changes from two seconds to one, as requested.
- [x] User accepts the complete replay checkpoint on 2026-10-16. Close 6F,
      commit the work unsigned, and stop for today. Leave 6G unstarted.

##### 6G1. Native camera backend probe — manually accepted

- [x] Evaluate native capture without coupling it to document/UI state.
      Prefer OS APIs over OpenCV/GStreamer or a bundled media runtime.
- [x] Add an opt-in command to list devices/formats and capture one PNG.
      Validate arguments, dimensions, frame buffers, and output safety;
      keep hardware access out of automated tests.
- [x] Check Linux build/device listing and native-free WASM compilation.
      Keep Windows Media Foundation and macOS isolation explicit; native
      Windows/macOS compilation still awaits CI. Install no container tools.
- [x] User marks the backend probe manually checked on 2026-10-19.
      Laptop checks cover enumeration, formats, image orientation/color,
      repeat capture/release, errors and permissions. SP6 checks remain
      separate; do not infer Surface hardware acceptance from laptop tests.

##### 6G2. Native preview and manual capture — manually accepted

- [x] Restore the stashed camera probe and exposure warm-up after the
      annotation refactor; share its adapter/decoder with the GUI.
- [x] Add a camera screen with immediate live preview, manual Capture, Retake,
      Use photo and Cancel. Add photos without discarding existing pages;
      reuse the page-correction editor after insertion.
- [x] Keep driver calls in a killable subprocess, bound frame delivery,
      prepare PNG off the UI thread and reject stale session completions.
- [x] Stop/reap workers on cancellation, capture and closure;
      expose recoverable errors and retain the 20-page limit.
- [x] User marks native preview/manual capture manually checked on
      2026-10-19, including immediate capture and uninterrupted window moves.
      Repeat on Windows/SP6 after laptop checks.

##### 6G3. Live framing and capture integration — manually accepted

Keep follow-ups small and manually testable:

- **6G3a — Live framing/countdown:** reuse the calibrated detector,
  stabilizer and one-second capture gate off the UI thread. Show a
  translucent quad and circular capture progress. Freeze the analyzed
  frame, carry its crop into pending page corrections and keep manual
  capture immediate. Bound analysis and reject stale sessions/results.
- **6G3b — Device and capture modes:** camera picker/switching, supported
  modes and higher-resolution capture, without silently distorting frames.
- **6G3c — Hardware lifecycle:** orientation, suspend/resume, permissions
  and actual SP6 front/rear camera calibration. Browser/macOS capture
  remains outside these native Linux/Windows checkpoints.

6G3a, 6G3b and 6G3c are manually accepted. Physical sleep testing and actual
SP6 hardware calibration remain deferred, separate from laptop acceptance.

6G3a implementation checklist:

- [x] Run hinted detection, consensus and one-second gating off the UI
      thread; keep one analysis job across canceled/restarted sessions.
- [x] Draw a translucent quad aligned with the contained preview and
      circular capture progress; retain immediate manual capture.
- [x] Freeze analyzed pixels/corners together, reject stale/current-missing
      final frames, and pass the crop into pending correction settings.
- [x] Preserve photo cancellation, manual capture, original pixels, errors,
      focus/move behavior and the existing recorded-sequence calibration.
- [x] Validate automated model/rendering, core, native and WASM checks.
      Pass 505 native/all-target and 270 dependency-light tests; keep the
      full replay opt-in. Strict Clippy, WASM and formatting checks pass.
- [x] User checkpoint: live quad, countdown/auto-freeze, motion/blur reset,
      Retake, manual capture and suggested-crop Accept/Cancel/Full frame.
      User accepts the captured-corner lag fix and 6G3a on 2026-10-19.

6G3b implementation checklist:

- [x] Discover named cameras and decoder-supported modes off the UI thread;
      bound discovery output and terminate stalled/canceled driver helpers.
- [x] Add touch-sized camera/mode pickers and Refresh cameras. Restart
      preview on changes; reject old frames, queries and queued starts.
- [x] Prefer full HD when available, at up to 30 FPS. Expose higher reported
      modes within image limits; never silently substitute a failed request.
- [x] Use the selected resolution for preview and the untouched photo.
      Preserve frame/corner alignment, immediate Capture, Retake and editing.
- [x] Pass 517 native/all-target and 270 dependency-light tests, strict
      Clippy, WASM, formatting and whitespace checks. Keep full replay opt-in.
- [x] User accepts device/mode selection, preview and auto-capture after
      resolution normalization on 2026-10-19. Laptop maximum is 1280×720;
      full-HD behavior is covered synthetically, not physically accepted.

6G3c implementation checklist — manually accepted:

- [x] Add lossless orientation controls shared by preview, detection and
      capture; keep window layout changes separate from sensor orientation.
      Mirror camera preview/review and overlay until Use photo; default on
      for explicit Front labels. Page editing/output stay unmirrored.
- [x] Handle sleep/interruption without stale auto-capture; release workers,
      reset tracking and provide an explicit fresh-preview recovery path.
- [x] Show actionable Linux/Windows camera permission/device guidance.
- [x] Automate pixel orientation, interruption, stale-completion, permission
      and recovery checks; retain native/WASM build coverage.
      Pass 536 native/all-target and 272 dependency-light checks, strict
      Clippy, WASM, formatting and whitespace checks; keep replay opt-in.
- [x] User accepts the final camera workflow on 2026-10-19, including
      mirrored snapshot review until Use photo opens the normal page editor.
      User defers physical sleep testing; do not mark it passed.
      Repeat SP6 front/rear/orientation and sleep tests there when available;
      do not infer their calibration from container or laptop tests.

### 7. Packaging and replacement decision — in progress

#### 7A. Prebuilt Windows test ZIP — Windows retest deferred

- [x] Build an optimized Windows x64 EXE in a dedicated CI job, with a static
      C runtime and release dependency cache; do not require a local compiler.
- [x] Package only the executable, instructions, license, sample form and
      revision/run metadata. Extract elsewhere; inspect imports and test
      loader/helper startup without opening a camera or GUI.
- [x] Upload a raw ZIP artifact on every run. Publish successful `master`
      checks separately as `rust-dev`; preserve C# releases and file names.
- [x] Exercise release creation, replacement and failed uploads locally with
      the real workflow shell block and a mock CLI; make no GitHub writes.
- [x] Remove the Windows camera-format unreachable match arm instead of
      suppressing its warning. Keep exhaustive handling of the six formats.
- [x] Run the Windows package job in GitHub, including extracted startup
      checks. User reports success in just under ten minutes; Windows native
      build/tests also pass. These platforms remain unavailable locally.
- [x] Obtain the CI build and launch the Windows EXE. User reports green CI
      and a brief Windows test; console/camera follow-up fixes are recorded
      below, not assumed verified from this initial test.
- [ ] Retest GUI-only startup and camera modes on Windows after the fix.
      User has no Windows access now; defer this check, not UI development.
- [ ] User checkpoint: download/extract on Windows without Rust installed;
      launch, open/save/export a project and try the SP6 cameras when available.
      Keep prior physical sleep/touch/DPI calibration deferrals explicit.

#### 7B. Font-size spinner — manually accepted

- [x] Replace the preset list with equal-sized minus/plus touch buttons and
      a numeric field. Step by 1 pt, including mouse wheel; clamp to 6–144 pt.
- [x] Accept valid numeric entry with Enter or an existing click-away accept
      path; reject invalid values without changing annotations. Escape first
      discards the numeric draft, not the page's pending text edit.
- [x] Preserve imported/scaled fractional sizes until the user changes them.
      Show mixed selections correctly; apply a chosen size to selected text
      only, in one undoable change, and keep shape geometry untouched.
- [x] Keep font changes to inline text pending until text acceptance; Escape
      must still restore the original text and format. Add regressions first.
- [x] Keep the control local to text options and existing appearance tokens;
      do not introduce a generic widget framework or redesign every panel.
- [x] Validate native tests, WASM compilation and lint. Hand off a Linux check;
      physical touch and the Windows camera retest remain deferred.
- [x] User checkpoint on Linux: enter 22 pt, step with buttons/wheel, reject
      200 pt, and verify Escape/Undo on inline text and mixed selections.
      User confirms these checks on 2026-10-20; touch remains deferred.

#### 7C. Stable confirmation overlays — manually accepted

- [x] Move replace-page, delete-page, delete-all and unsaved-project prompts
      out of the header into a centered modal layer. Never resize/reposition
      the editor, page rail or toolbars when a confirmation appears.
- [x] Use readable themed cards, a dimmed backdrop and equal-height/width
      touch actions. Fit small windows; keep styling in appearance.rs.
- [x] Block background pointer/wheel input; preserve existing workflow guards,
      Keep/Cancel, Escape, saving and undo behavior. Do not dismiss on a
      backdrop click or accept destructive actions implicitly with Enter.
- [x] Test layout stability, actual button routing, mouse/touch/wheel input
      isolation, and rendering above a zoomed page at both DPI scales.
- [x] Pass all 556 native/all-target tests, strict lint, WASM compilation and
      formatting. Exhaustive replay remains ignored/opt-in; physical touch,
      Windows camera and Surface DPI/sleep checks remain deferred.
- [x] User checkpoint on Linux: open/cancel each prompt; confirm delete-all
      then Undo; verify the page stays put and backdrop clicks do nothing.
      User confirms dialogs work well on 2026-10-20; touch remains deferred.

#### 7D. Compact vector tools and color swatches — accepted

- [x] Replace the eleven text tool buttons with centered vector icons in
      equal 40-pixel square targets. Keep native tooltips and an active-tool
      label; use consistent stroke weights and selected-theme foregrounds.
- [x] Replace Smart snap and Fill labels with monochrome magnet/bucket
      buttons using the same sizes, selected styles and applicability rules.
- [x] Show six round color swatches with a separated selection ring; retain
      pigment on hover and dim rather than whiten disabled colors.
- [x] Keep toolbar layout/styles local. Reuse iced canvas/native controls;
      add no new font, image, dependency or generic icon/widget framework.
- [x] Automate size/centering, theme/selected/disabled rendering, color/hover
      behavior and existing command routing; pass native/WASM/lint checks.
      All 563 native/all-target tests, strict Clippy, WASM and formatting pass;
      exhaustive replay remains opt-in. Preview both themes headlessly.
- [x] User checkpoint on Linux: inspect icons, hover labels, selection and
      color rings; use every tool, Fill and Smart snap, including after
      placement and in a narrower window. Keep Windows/touch checks deferred.
      User accepts the icons on 2026-10-20 except the ambiguous Text glyph;
      replace it with Aa using the existing bundled font. Verify that small
      follow-up headlessly; user explicitly defers scrollbar overlap.

#### Remaining UI polish and replacement gate — not started

- Polish the UI for regular use: cohesive visual styling and touch layout.
  Stable confirmations and compact tools have their own checkpoints above.
- During the toolbar/panel redesign, reserve scrollbar space so horizontal
  tracks do not cover button bottoms. User defers this temporary overlap.
- Profile the browser harness if full-release builds remain sluggish;
  debug builds now lightly optimize FastFill as well as its dependencies.
- Redesign the page rail with animated reorder feedback:
  - Draw a semi-transparent thumbnail above the UI, keeping the original grab
    offset so it follows the mouse/touch pointer even outside the rail.
  - Remove the dragged page from the displayed sequence; show a larger gap at
    its prospective insertion point. Animate neighboring pages as it moves.
  - On Escape or release outside the rail, animate the thumbnail back to its
    original place and close the gap. On a valid drop, animate it into the
    new position. Commit page order only on a valid drop; settling is visual.
  - Remove the temporary "Drag page to reorder" captions during the redesign.
  - Reuse iced opacity, overlays, and redraw scheduling with a small eased
    transition (initially 150–200 ms). Request frames only while animating.
    Keep a reduced-motion/instant-settle path and test interrupted animations,
    pointer offsets, scrolling, cancellation, and final ordering.
  - Feasible, moderate scope: estimate a few hundred production lines plus
    tests, localized to rail layout, drag state, and the floating overlay.
    No animation dependency or document-model rewrite should be needed.
- Separate prototype artifacts from existing C# release names/assets.
- Portable Windows build, documented Linux dependencies/package, icons,
  diagnostics, file opening, help, and user documentation.
- Automated integration/regression checks; focused fresh review.
- User release gate: 20-page project, repeated scans, save/reopen/export,
  Surface touch at 150%/200% scaling, memory and latency.
- Decide whether to replace the C# app; no automatic migration/removal.

## Progress log

### 2026-10-12 — planning and environment inspection

- Existing C# files and untracked `manual-checks.txt` remain untouched.
- No Cargo/Rust compiler is currently available through PATH or installed
  mise tools. User authorized installing required tools through mise.
- Checked iced's published 0.14.0 APIs and feature configuration rather than
  developing against the unreleased main branch.
- Added independent `Rust prototype` CI. Native checks run on Linux, Windows,
  and macOS; a separate job compile-checks `wasm32-unknown-unknown`.
- TDD red: `cargo test --no-default-features` failed because the tested
  `geometry` module did not exist. Green: three tests now cover page fitting,
  coordinate round trips, and invalid dimensions.
- Passed locally with Rust 1.99.0: formatting, all-feature tests, strict
  Clippy, native GUI compilation, and WASM GUI compilation.
- Native Windows and macOS compilation await the new CI workflow. GUI layout
  and DPI behavior await user testing because this container has no display.
- Next: user checks this viewer on Linux and Windows. Record feedback before
  starting checkpoint 2.

### 2026-10-12 — checkpoint 2A image and mouse navigation

- User accepted checkpoint 1 at normal DPI. No high-DPI display was available;
  keep that check open for later hardware rather than blocking progress.
- TDD red: four new navigation tests failed before `Navigation` existed.
  Green: fit, pan, 1:1, pointer anchoring, zoom limits, and invalid changes
  pass.
- TDD red: image-input tests failed before the validation boundary existed.
  Green: PNG/JPEG decoding enforces the existing 100 MiB and 50 MP limits and
  rejects empty, unsupported, oversized, truncated, or corrupt input.
- Added a cross-platform picker, image canvas, Fit and 1:1 controls, zoom
  buttons, pointer-anchored mouse-wheel zoom, and left/right-button panning.
- Passed nine tests, strict Clippy, formatting, native compilation, and WASM
  compilation. Windows and macOS remain covered by CI once pushed.
- User accepted image import and mouse navigation. Import errors worked but
  were too easy to miss in the normal status line.

### 2026-10-12 — checkpoint 2B touch navigation

- Made import failures a prominent warning panel while retaining the status
  text as the single source of error details.
- TDD red: interaction tests failed while the touch module was absent. Green:
  four tests now cover one-finger pan, two-finger pan/pinch, stable finger
  transitions, and suppression when a drawing tool owns input.
- Kept right-button mouse panning available as a temporary navigation input.
- Passed all 13 tests, strict Clippy, formatting, native compilation, and WASM
  compilation. Windows and macOS remain covered by CI once pushed.
- Added a minimal Trunk web harness for real multi-touch testing on Android.
  `mise run web` serves it to the LAN on port 24800. Browser page gestures are
  disabled only on the app canvas so document pinch gestures reach iced.
- Verified a linked Trunk bundle and fetched its HTML, JavaScript, and WASM
  assets through the local HTTP server.
- Container builds now use ignored `mise.codex.toml` and write Cargo output to
  `/tmp/fastfill-rust-codex-target`; host builds keep their own `rust/target`.
- User accepted touch actions provisionally through the Android WASM harness.
  Browser rendering was very slow; record it as a prototype limitation, not a
  native performance result. Repeat on better touchscreen hardware later.
- Checkpoint 2 is closed. Stop here by user request; checkpoint 3 remains
  unstarted.

### 2026-10-13 — checkpoint 3A rectangle drawing

- User confirmed the initial Rust CI passed after pushing checkpoint 2.
- Start the first annotation slice. Keep the next user checkpoint limited to
  drawing rectangles and isolating drawing input from navigation.
- TDD red: annotation tests failed before the annotation module existed;
  application tests then failed before drawing and replacement states existed.
  Green: eight core tests and eight application/input tests cover drafts,
  constraints, coordinates, pointer ownership, cancellation, and replacement.
- Added Navigate and Rectangle tools, blue drawing previews, black committed
  outlines, Ctrl-to-square, Escape cancellation, and a rectangle counter.
  Right-drag and idle mouse-wheel zoom work in either tool; touch navigation
  belongs to Navigate.
- Keep annotations in image coordinates. Draw partial off-page objects through
  clipped stroke bands, without modifying their stored bounds.
- Added a headless software-rendering test at 100% and 200% scale. It exposed
  iced's image-over-stroke ordering and lost nested stroke clips. Use separate
  stack layers and explicit band clipping to cover both renderers.
- CI now runs application/input/rendering tests on Linux, Windows, and macOS
  as well as the independent core tests and WASM compilation.
- Passed all 30 tests, the independent core test run, strict Clippy, formatting,
  and a linked Trunk WASM build locally. The expanded native CI awaits a push.
- Annotations remain temporary; selection, object resizing, deletion, and
  history await later slices. Stop at the 3A user check before adding them.
- User accepted the drawing checkpoint. Touch drawing can wait for better
  hardware.
- Follow-up: enable pointer-anchored wheel zoom in both tools while idle, and
  ignore it during drawing/panning. Two regression tests first reproduced the
  missing rectangle-tool zoom and unwanted zoom during a navigation drag.
- Follow-up validation passed: all 32 tests, strict Clippy, formatting, and
  the linked WASM build.
- User accepts the temporary confirmation bar shrinking the canvas. Record a
  stable confirmation overlay and a full visual-polish pass for checkpoint 7.

### 2026-10-13 — checkpoint 3B rectangle editing

- TDD red: selection tests failed before edit state and hit testing existed.
  Green: tests cover topmost selection, pending move/resize, all four corners,
  modifiers, cancellation, deletion, small targets, and off-page edits.
- Added Select and Delete controls. Drag anywhere in a selected box to move;
  drag a corner handle to resize. Rectangle mode temporarily edits its selected
  object when the pointer starts within its controls and draws elsewhere.
- Keep each edit separate from committed state until release. Escape, lost
  input, tool changes, and view changes restore the original rectangle.
- Keep stroke width independent of resize geometry. Ctrl constrains a resize
  to a square, Shift forces movement, and Alt forces the nearest-corner resize.
- Allow partly off-page edits while continuously retaining a small reachable
  portion instead of forcing full containment.
- Added a software-renderer regression check proving an edit preview replaces
  the committed outline instead of leaving a ghost at its old position.
- Passed all 49 feature-enabled tests, the dependency-light core suite, strict
  Clippy, formatting, and a linked Trunk WASM build.
- Follow-up: continuously clamp one document unit while dragging off-page.
  Tests cover crossing and reversing without a jump at three zoom levels.
- Follow-up: keep the visible selection frame snug to small objects. Retain
  larger mouse/touch hit regions invisibly around the actual corner handles.
- Next: user checks selection, handles, modifiers, cancellation, deletion, and
  coordinates through zoom/pan before checkpoint 3C undo/redo begins.

### 2026-10-13 — checkpoint 3C undo and redo

- User accepted selection, movement, resizing, deletion, off-page clamping,
  and snug selection frames. Committed checkpoints 3A and 3B as `f8003bd`.
- TDD red: history tests failed before undo, redo, and availability APIs
  existed. Green: creation, movement, resizing, deletion, pending drafts,
  no-op edits, selection restoration, and redo invalidation pass.
- Store full rectangle snapshots for now. This is the smallest reliable model
  for the one-page rectangle prototype; structured history can wait until
  document and annotation types justify it.
- Added Undo and Redo buttons plus platform-command Z, Shift+Z, and Y. On
  Windows and Linux the platform command is Ctrl; on macOS it is Command.
- Canceled page replacement preserves history. Successful replacement starts
  a new empty history with the new page.
- Passed all 54 feature-enabled tests, the dependency-light core suite, strict
  Clippy, formatting, and a linked Trunk WASM build.

### 2026-10-13 — checkpoint 3D oval annotations

- User accepted mixed undo/redo sequences, shortcuts, draft cancellation, and
  redo invalidation after a new edit.
- TDD red: model tests failed before oval kinds existed; the headless renderer
  then showed a rectangular outline for an oval. Green: shape kind, editing,
  history, curved outline, and segment clipping checks pass.
- Generalized the bounded annotation model to Rectangle and Oval without
  duplicating selection, editing, reachability, or history code.
- Approximate oval outlines with 128 segments and clip each segment explicitly.
  This avoids iced's known lost nested clip in the software renderer.
- Split the former monolithic entry point into a seven-line `main.rs`, app
  state in `app.rs`, app checks in `app_tests.rs`, canvas input/rendering in
  `page_canvas.rs`, and validated image selection in `picker.rs`.
- Passed all 58 feature-enabled tests, the dependency-light core suite, strict
  Clippy, formatting, and a linked Trunk WASM build.
- User accepted oval creation, constrained circles, editing, clipping,
  navigation, and history behavior.

### 2026-10-13 — checkpoint 3E straight-line annotations

- User accepted checkpoint 3D. Committed history, ovals, and the source split
  as `479eeb8` before starting this slice.
- TDD red: line tests failed before explicit line geometry, tools, and endpoint
  hits existed. Green: creation in every direction, live Ctrl snapping,
  selection, movement, endpoint editing, cancellation, deletion, history,
  reachability, and zoom-independent hit widths pass.
- Store directed line endpoints in the shared annotation geometry enum. Do not
  derive endpoints from a bounding rectangle or add arrow-specific state yet.
- Reuse the existing interaction and snapshot history. Endpoint edits keep the
  opposite endpoint fixed unless reachability correction is required.
- Render and clip one constant-width segment. Display only two endpoint handles
  while retaining the line's tight bounding frame for movement feedback.
- Passed all 68 feature-enabled tests, the dependency-light core suite, strict
  Clippy, formatting, and a linked Trunk WASM build.
- User accepted line creation, snapping, endpoint editing, history, and
  navigation behavior.

### 2026-10-13 — checkpoint 3F arrow annotations

- User accepted checkpoint 3E. Committed editable straight lines as `cefaeb1`
  before starting this slice.
- TDD red: arrow tests failed before arrow tools and linear kinds existed. A
  later check proved the arrowhead was missing from bounds and hit testing.
- Reuse directed endpoints, editing, snapping, cancellation, history, and
  reachability for both straight lines and arrows.
- Derive arrowhead wings from a normalized shaft direction and stroke width.
  Include them in bounds and hit testing without storing redundant geometry.
- Clip the shaft and both arrowhead sides independently at the page boundary.
- Passed all 73 feature-enabled tests, the dependency-light core suite, strict
  Clippy, formatting, and a linked Trunk WASM build.
- User accepted arrow direction, head proportions, editing, clipping, history,
  and navigation behavior.

### 2026-10-13 — checkpoint 3G checkmark and cross stamps

- User accepted checkpoint 3F. Amended the line commit to include arrows as
  `211bfa3`; future checkpoints remain uncommitted until explicitly requested.
- TDD red: mark tests failed before checkmark and cross tools existed. Green:
  centered click placement, outward sizing, fixed-square resize, movement,
  deletion, history, and bounded geometry pass.
- Use a page-relative minimum size and center it on the initial pointer. This
  follows form-filling placement reflexes and avoids oversized default marks.
- Use a slightly thinner page-relative default stroke for marks. Render only
  their two defining segments; do not draw a surrounding box.
- Passed all 81 feature-enabled tests, the dependency-light core suite, strict
  Clippy, formatting, and a linked Trunk WASM build.
- Follow-up screenshots exposed inset mark strokes and a split checkmark join.
  Marks now span their full selection bounds, and the checkmark uses one
  continuous path through its bottom corner.
- A second screenshot exposed the checkmark's acute miter outside its frame.
  Inset mark centerlines by half a stroke and use a round checkmark join so
  visible ink reaches but does not cross the selection bounds.
- User accepted centered placement, square resizing, rendering, joins, and
  containment for both mark tools.
- Follow-up screenshots exposed centerline-only bounds on lines and arrows.
  Their frames now union the exact painted bounds of each butt-capped segment,
  using direction and stroke width while leaving handles on true endpoints.
- Arrowhead screenshots then exposed three butt-ended paths at the tip. Render
  both wings as one round-joined polyline and include the rounded tip in its
  painted bounds.
- Apply one containment rule to every annotation: visible ink stays inside its
  selection frame. Rectangle and oval centerlines are now inset by half their
  effective stroke; undersized frames reduce the stroke instead of leaking.
- Thick oval strokes exposed the old 128 independent clipping segments. Join
  visible segments into continuous runs, close complete ovals, and use round
  joins so clipping does not create a comb-like outline.

### 2026-10-13 — checkpoint 3H annotation styles and fill

- Corrected the plan before checkpoint 4: styles, freehand tools, rotation,
  multi-selection, layers, and bulk deletion remain part of phase 3.
- Preserved the prematurely started inline-text prototype in Git stash
  `wip/rust: Phase 4 inline text prototype` for later checkpoint 4 work.
- TDD red: style tests failed before per-tool defaults and object styles
  existed; renderer checks then showed ignored colors and fill state.
- Store six fixed colors, 1–20 stroke width, and compatible fill state on each
  object. Keep independent session defaults for every current drawing tool.
- Apply style changes in Select mode as undoable document edits. Tool-default
  changes affect only subsequently created objects and do not pollute history.
- Clip solid rectangles directly and solid ovals as polygons, retaining both
  page clipping and the rule that ink remains inside its selection frame.
- Added a third functional toolbar row for colors, thickness, and compatible
  fill controls. Visual polish remains checkpoint 7 work.
- Centralized UI dimensions, canvas colors, annotation colors, and selection
  metrics in `appearance.rs`. Split the toolbar into control, tool, and style
  rows so checkpoint 7 can restyle components without touching interactions.
- Follow-up testing found that style controls targeted only tool defaults while
  a just-created object remained selected. Compatible selected objects now
  receive the change and the tool remembers it for the next object.
- Draft and edit previews now use their actual annotation color. Active edits
  remain temporarily above siblings for visibility, then return to layer order.
- Decoupled arrowhead geometry from stroke thickness. A thicker arrow changes
  only its painted stroke, not the arrowhead's centerline length or spread.
- Passed 90 all-feature tests, 61 core-only tests, strict Clippy, formatting,
  native and WASM compilation, and a linked Trunk WASM build.

### 2026-10-13 — checkpoint 3I pen and highlighter

- User accepted checkpoint 3H after the selected-object, preview-color, and
  arrowhead-thickness follow-up fixes.
- TDD red: ink tests failed before pen/highlighter tools and owned point paths
  existed. Filled-path interior selection also failed before polygon testing.
- Store sampled mouse or touch points in page coordinates. Ignore duplicate
  samples and events from pointers that do not own the active stroke.
- Render pen paths with round caps and joins. Render highlighter paths with a
  fixed translucent alpha while retaining their selected color and thickness.
- Let Pen Fill close the sampled polygon while retaining its outline. Keep
  highlighter unfilled and give it an independent yellow, 12 px default.
- Move or freely resize whole paths while keeping stroke thickness unchanged.
  Include visible stroke in bounds so ink stays inside its selection frame.
- Clip filled polygons and stroked path runs at page boundaries. Selection,
  cancel, delete, undo, redo, and reachability reuse existing annotation flow.
- Close a filled pen's outline as a joined path instead of leaving round caps
  at its start and end. Preserve separate runs where the page clips the path.
- User accepted drawing, editing, filling, and interior selection after the
  filled-outline join fix. Thick off-page clipping remains an accepted edge
  case for this checkpoint.
- Passed 98 all-feature tests, 66 core-only tests, strict Clippy, formatting,
  native and WASM compilation, and a linked Trunk WASM build.

### 2026-10-13 — checkpoint 3J rotation

- User accepted checkpoint 3I after the filled pen outline became one closed,
  round-joined path instead of two capped ends.
- TDD red: rotation tests failed before objects stored an angle, exposed a
  handle, and supported reset/history. A renderer check then failed until
  bounded geometry was transformed about its frame center.
- Rotate rectangles, ovals, marks, pen, and highlighter from a separate handle
  above the frame. Keep line and arrow editing limited to their endpoints.
- Hold Ctrl for 45-degree snapping. Preserve rotation through move, local-axis
  resize, cancel, undo, and redo; Rotation 0° records an undoable reset.
- Rotate painted geometry and oriented selection controls together. Continue
  clipping fills and stroke centerlines at the page boundary.
- Split annotation math, draft interaction, and state into focused modules.
  Split canvas event handling from rendering and move renderer tests out of
  the implementation file before starting another feature stage.
- Passed 102 all-feature tests, 71 core-only tests, strict Clippy, formatting,
  native and WASM compilation, and a linked Trunk WASM build.
- User accepted mouse rotation, snapping, reset, editing, and rendering after
  the refactor. Touch-specific rotation remains deferred for later hardware.

### 2026-10-13 — checkpoint 3K multi-selection and layers

- User accepted checkpoint 3J and its source split. Committed it as `e2ad857`
  before starting multi-selection; touch rotation remains assumed for now.
- TDD red: the multi-selection tests failed before selection sets, marquee
  drafts, grouped movement, bulk styles/deletion, and layer commands existed.
  Delete-all application tests failed before a confirmation state existed.
- Keep selected object indices sorted. Ctrl-click toggles one object; dragging
  from empty page space selects every object intersecting the marquee.
- Move a selected group with one shared constrained translation so every
  object retains a reachable part. Render active group edits above siblings.
- Apply color and thickness to every selected object. Apply Fill only to
  compatible rectangles, ovals, and pen paths.
- Delete a selection in one history entry. Require explicit confirmation for
  Delete all. Preserve selection and relative layer order through undo/redo.
- Send selected objects behind or in front of unselected objects while
  preserving the relative order within both groups.
- Split style, selection, and selection-rendering code into focused modules;
  keep UI dimensions and colors centralized for checkpoint 7 restyling.
- User accepted marquee and Ctrl-click selection, grouped movement, bulk
  styles, layers, both deletion paths, edge reachability, and history. Native
  touch behavior remains deferred for later hardware.
- Follow-up feedback exposed ambiguous membership for objects inside a group
  boundary. Draw a faint frame around every member beneath the strong outer
  frame; a headless renderer regression toggles an interior member while the
  group boundary remains unchanged.
- Passed 113 all-feature tests, 79 core-only tests, strict Clippy, formatting,
  native and WASM compilation, and a linked Trunk WASM build.

### 2026-10-14 — architecture review before checkpoint 4

- Audited the phase 3 commit spread. Model, editor, renderer, UI, and tests are
  legitimate layers of a vertical tool feature; forcing them together would
  couple the reusable core to iced. The avoidable coupling was inside large
  module roots and duplicated tool lists.
- Turned `annotations`, `app`, and `page_canvas` into directory modules. Their
  roots now expose state and wiring while focused children own model types,
  tool metadata, controller logic, views, rendering, clipping, input, and
  selection visuals.
- Added one `Tool::ALL` catalog for toolbar order and per-tool defaults. Tool
  capabilities now drive canvas input and cursors, so another drawing tool no
  longer requires duplicate exhaustive lists in GUI code.
- Kept geometry and editor state independent of iced and native APIs. Avoided
  a trait-per-tool design: current geometry families already provide the
  useful extension seam without speculative abstractions.
- The macOS CI failure was a platform-specific test bug. The application uses
  iced's command modifier correctly; its test now uses `Modifiers::COMMAND`
  instead of assuming Ctrl on every operating system.
- Passed 114 all-feature tests, 80 core-only tests, strict Clippy, formatting,
  native and WASM compilation, and a linked Trunk WASM build.
- User smoke-tested the existing tools and accepted the refactor before phase
  4 started.

### 2026-10-14 — checkpoint 4A plain inline text

- Applied the saved Phase 4 prototype without dropping its stash. Ported only
  the plain text lifecycle into the refactored module layout; multiline text,
  measured bounds, selection, and formatting remain later slices.
- TDD red: the isolated text tests failed before the Text tool, pending draft,
  acceptance, history, and deletion APIs existed.
- Place text by click or tap, left-aligned at that page point. Edit it in a
  focused, translucent input over the page; Enter or another page click
  accepts it and Escape cancels it.
- Keep empty or canceled drafts out of history. Include accepted text in undo,
  redo, delete-all, replacement confirmation, annotation count, and rendering.
- Keep edge placement consistent by storing the adjusted editor anchor and
  width used by final rendering. Use high-contrast black editor text.
- Match the editor's padding and 1.3 line-height in canvas rendering. A
  headless renderer check requires editing and finalized glyph bounds to be
  identical.
- Passed 124 all-feature tests, 85 core-only tests, strict Clippy, formatting,
  native and WASM compilation, and a linked Trunk WASM build.
- User accepted 4A after left-aligned placement, high-contrast input, shared
  edge clamping, and exact editor/finalized glyph positioning were verified.

### 2026-10-14 — checkpoint 4B multiline and measured text

- User accepted 4A. TDD red then covered multiline bounds, reopening, moving,
  edit history, deletion safety, and zoom-independent placement.
- Replace the single-line input with iced's multiline editor. Plain Enter
  accepts; Shift+Enter inserts a line break. Click-away and Escape retain
  their accepted and canceled meanings.
- Measure the same font, shaping, size, and line height used by the canvas
  renderer. Keep content bounds in page units, so zoom does not alter object
  geometry. Preserve only explicit line breaks; never soft-wrap at page edges.
- Preserve the text origin while content grows. Reopen text immediately with
  Text, or click-release with Select; drag with Select to move it as one
  undoable edit.
- Draw a selection frame for accepted text, allow deletion when not editing,
  and clip rendered glyphs at the page edge. Editing and finalized multiline
  glyph bounds match in headless renderer checks.
- Edge testing exposed bounds clamped to the remaining paper area. Text frames
  now retain full measured size outside the paper, while accepted glyphs are
  clipped to both paper and document viewport. This also prevents panned text
  from painting over toolbars.
- Passed 135 all-feature tests, 92 dependency-light core tests, strict Clippy,
  formatting, native and WASM compilation, and a linked Trunk WASM build.
- User accepted multiline entry, full-size edge frames, explicit-only line
  breaks, reopening, movement, cancellation, clipping, and undo.

### 2026-10-14 — checkpoint 4C formatting and rotation

- TDD red covered pending format changes, Escape restoration, acceptance as
  one history entry, remembered defaults, rotated hit testing, Ctrl snapping,
  reset, and undo/redo.
- Add generic Sans, Serif, and Monospace choices; 8–64 point sizes; bold,
  italic, underline, and six existing colors. Measure and render with the same
  selected font attributes.
- Keep format changes inside the inline draft. Escape restores every committed
  text property; Enter or click-away accepts them together. Accepted settings
  become defaults for the next text object.
- Rotate selected text using its own handle. Ctrl snaps to 45° increments and
  Rotation 0° participates in history. Hit testing and selection frames follow
  the rotated object.
- Keep editing horizontal and size it from unrotated measured bounds. Render
  accepted glyphs and underlines rotated and clipped to the page.
- Fall back to bundled Fira Sans instead of crashing when a minimal Linux or
  WASM environment lacks a requested generic or italic face. Normal desktop
  font installations retain their native generic families.
- Select clicks now retain an inactive text selection for move, rotation,
  deletion, and immediate undoable formatting. Text clicks still reopen the
  inline editor, whose formatting remains pending and cancelable.
- Step the font size by scrolling over its selector. Keep normal canvas wheel
  zoom elsewhere.
- Put text and shapes in one explicit layer order. Layer buttons now work for
  text, Select hit testing follows visible topmost order, and separate canvas
  layers prevent iced from batching all glyphs above paths.
- Passed 147 all-feature tests and 99 dependency-light core tests, strict
  Clippy, formatting, native and WASM compilation, and a linked Trunk build.
- User accepted Select-versus-Text behavior, inactive formatting, font-size
  scrolling, mixed text/shape layers, and the remaining 4C behavior.

### 2026-10-14 — checkpoint 4D1 shared annotation layout

- TDD red: backend-neutral rendering tests failed before a core rendering
  module exposed annotation paths and text layout.
- Generate rectangle, oval, line, arrow, mark, pen, and highlighter paths in
  page coordinates without iced or platform dependencies. Keep color,
  opacity, stroke width, joins, caps, fill, rotation, and layer order intact.
- Make the iced canvas a thin adapter over that shared layout. Existing
  clipping stays in the preview backend and all pixel-rendering regressions
  remain green.
- Keep text content, measured bounds, format, and rotation in shared layout.
  Actual glyph shaping remains iced-specific; the PDF probe must expose any
  metric mismatch or flatten the same shaped output rather than silently
  repositioning text.
- Add no dependency. The PDF backend decision remains deferred until 4D2.
- Passed 150 all-feature tests and 103 dependency-light core tests, strict
  Clippy, formatting, native and WASM compilation, and a linked Trunk build.
- User confirmed that existing annotation rendering remains visually
  unchanged. Start the PDF probe without reopening preview work.

### 2026-10-14 — checkpoint 4D2 PDF probe and toolkit decision

- TDD red: the PDF probe test failed before its feature and module existed.
  Green: valid page geometry produces a one-page PDF and invalid dimensions
  fail explicitly.
- Add optional Krilla 0.8.2 with no default features. Translate the shared
  paths, fills, strokes, joins, caps, colors, rotations, and opacity directly;
  do not duplicate annotation geometry in the PDF adapter.
- Reject text with `TextRequiresFlattening`. Iced/cosmic-text owns current
  font fallback, shaping, and measured bounds; reshaping with another engine
  could move or wrap text. Imported page images are likewise absent from this
  vector-only probe rather than silently rendered differently.
- Keep iced. It has passed the required desktop interactions, inline editing,
  shared layer rendering, native CI, and WASM compilation. It need not be the
  PDF serializer. Phase 5 should flatten the complete page with matching text
  layout, then use the PDF library as the container.
- Passed 153 all-feature tests, 103 dependency-light core tests, strict
  Clippy, formatting, all-feature WASM compilation, and a linked Trunk build.

### 2026-10-14 — checkpoint 5A safe project loading

- User confirmed the Phase 4 host tests and application startup before this
  slice began.
- TDD red: the archive tests failed before the project module existed. Green:
  six focused checks cover compatible data, hostile names, normalized-name
  collisions, unknown fields, model limits, geometry, assets, and manifest
  size.
- Mirror schema version 1 as a separate lossless project model. Preserve all
  crop, filter, annotation, style, transform, identity, and timestamp data;
  do not prematurely force normalized C# geometry into the Rust editor model.
- Bound entry count, compressed and expanded sizes, manifest and image bytes,
  decoded pixels, pages, annotations, ink points, and UTF-16 text lengths.
  Reject non-regular/unsafe entries, duplicate entries or IDs, malformed JSON,
  invalid transforms, off-page objects, and asset metadata mismatches.
- Load both real C# review projects through the same public API used by tests.
  Their legacy computed JSON members and omitted default text fields remain
  compatible.
- Add only pure-Rust ZIP/JSON/schema dependencies. All-feature WASM compilation
  and the linked Trunk build remain green.
- Passed 159 all-feature tests, 109 dependency-light core tests, strict
  Clippy, formatting, all-feature WASM compilation, and a linked Trunk build.
- UI opening and conversion into editable Rust pages remain deliberately out
  of this trust-boundary slice. User confirmed the host test suite remains
  green, so 5A is accepted.

### 2026-10-14 — checkpoint 5B atomic project saving

- TDD red: archive tests failed before save APIs existed. Green: schema-v1
  data and assets survive load-save-load, missing assets fail before output,
  invalid saves preserve the old file, and successful saves leave no
  temporary entries behind.
- Validate the model, encoded images, dimensions, formats, manifest size, and
  expanded size before writing. Reopen and compare the temporary archive
  before committing it.
- Use `atomicwrites` only on native targets. It writes beside the destination,
  syncs data, uses durable Unix rename, and uses `MoveFileExW` with replace and
  write-through flags on Windows. Keep the shared ZIP writer dependency-free
  from native filesystem APIs so WebAssembly still builds.
- Write one entry per referenced asset, even if multiple pages share it. This
  avoids ambiguous duplicate ZIP names while remaining compatible with the C#
  loader.
- Round-trip both available C# review projects without changing their project
  model or page assets.
- Passed 162 all-feature tests, 112 dependency-light core tests, strict
  Clippy, formatting, all-feature WebAssembly compilation, and a linked Trunk
  build locally. Native Windows and macOS replacement still rely on CI.
- User confirmed the host test suite passes; 5B is accepted.

### 2026-10-14 — checkpoint 5C multiple image pages

- TDD red: controller tests failed before multi-page state and messages
  existed. Green: independent annotations/history, switches, tool defaults,
  page limits, picker cancellation, stale replies, reorder, and confirmed
  deletion retain the expected page data.
- Move page ownership and page-specific controller actions into `app/pages`.
  Copy only tool settings between pages; keep each page's annotation history
  in place. Store no duplicated active-page annotation state.
- Add a fixed-width rail with image thumbnails, annotation counts, native
  page selector, import/replace/delete controls, and reorder buttons. A
  dedicated drag grip owns mouse/touch reorder; touch elsewhere scrolls.
- Preview reorder until release. Use the final release position, reject a
  drop outside the rail, ignore other pointers, and cancel on Escape or lost
  touch. Automated input tests cover these paths without a display.
- Keep annotation Undo/Redo per page. Page operations currently require
  explicit confirmation where destructive and are not part of undo. Project
  open/save UI, archive-to-editor conversion, and dirty revisions follow in
  the next slice.
- Add a software-rendered full-window layout check for rail/canvas separation
  and keep all existing rendering tests green. Let the tool row scroll
  horizontally when the narrower editor cannot fit every control.
- Passed 173 all-feature tests, 112 core tests, strict Clippy, formatting,
  all-feature WebAssembly compilation, and a linked Trunk build locally.
- Await manual checks: add several images, annotate each, switch and undo
  independently, reorder with buttons/grips, cancel a reorder, cancel and
  confirm replacement/deletion, and check the 20-page limit. Native touch
  remains a hardware checkpoint when a suitable device is available.

### 2026-10-14 — checkpoint 5C manual-test follow-up

- Reuse the last successful import directory for native Add/Replace dialogs
  during the current run. Fall back to the normal dialog location if that
  directory no longer exists; browsers cannot expose the selected folder.
- Allow drag from the whole thumbnail. Reflow thumbnails into their preview
  positions, highlight the moved card, and separate the drop marker from
  the card. Accept empty space below the final page as an end-of-list drop.
- Provide a separate right-edge scrollbar for mouse/touch list scrolling.
  Mouse wheel scrolling also works while reordering a long list.
- The previous Add path retained an annotated starter blank as another page.
  Make the first-image workflow explicit instead: ask before replacing an
  annotated starter, and retain its annotations/history if confirmation or
  image selection is canceled. Add on image pages still inserts a new page.
- TDD red: folder-memory and reorder-preview APIs were absent. Green: new
  tests cover whole-thumbnail mouse/touch drags, end-of-list release through
  actual input events, preview/cancel geometry, folder memory, and confirmed
  starter replacement. All 178 all-feature tests, strict Clippy, formatting,
  WebAssembly compilation, and the linked Trunk build pass locally.
- User confirmed the behavior; 5C is accepted. Record the requested animated
  floating-thumbnail/gap reorder under Phase 7. Keep the current labels until
  that rail redesign; no further UI implementation in this checkpoint.

### 2026-10-14 — checkpoint 5D1 editable schema bridge

- Commit accepted 5B/5C progress as `81489b8`, unsigned and without pushing.
- Split 5D into a core conversion checkpoint and a later open/save UI slice.
  Keep the current image-pixel GUI coordinate system unchanged until 5D2.
- TDD red: bridge APIs, object IDs, and content revisions did not exist.
  Green: all current tools preserve identity/order/styles through conversion
  and a real ZIP round-trip. Rotated rectangles, ovals, marks, and ink retain
  physical corners on portrait pages; loading starts with empty history.
- Generate IDs once per draft. Preserve them through editing, translation,
  rotation, layer changes, and history. Use the existing UUID dependency's
  native RNG/Web Crypto support rather than a new ID mechanism.
- Track only committed content revisions. Cancel, selection, and tool defaults
  do not dirty the document; undo/redo restores the previous revision, and
  a new branch receives a distinct revision.
- Preserve unchanged schema objects exactly, including unused metadata. Map
  arbitrary ARGB colors without reducing them to the palette; combine alpha
  and highlighter transparency once in backend-neutral paint. A software
  renderer test also verifies alpha is not multiplied twice on the canvas.
- Reject pending edits and unsupported crops, page rotation, filters, affine
  scaling/shear, fonts, and soft-wrap settings. The archive loader continues
  to preserve full schema-v1 projects. These are temporary editable-conversion
  limits, not the final v1 import policy; font metrics are not pixel-identical
  across the old and new renderers.
- TDD red also exposed finite normalized coordinates overflowing to NaN/Inf
  after point conversion. Reject geometry outside the editor's numeric range.
- Record user approval for meaningful schema-v2 output, retaining v1 import.
  No schema change in this foundation; original image bytes are never resized.
- Passed 190 all-feature tests, 123 core-only tests, strict Clippy, formatting,
  all-feature WebAssembly compilation, and a linked Trunk build locally.
- Next: host tests, then connect open/save dialogs, point-space page geometry,
  and dirty-document confirmation in 5D2. No new UI check in 5D1.

### 2026-10-14 — checkpoint 5D2 native project workflow

- User confirmed the 5D1 host tests; mark that foundation accepted.
- TDD red: project controller/actions were absent. Green: accept pending text,
  cancel drafts, prompt on dirty New/Open/Close, and retain the current pages
  on cancellation, unsupported content, or failed load/save. A successful
  Save-before-leave resumes its intended action; failed/canceled saves do not.
- Keep project transaction state, file I/O, and project UI in focused modules.
  Image/ZIP processing and native archive I/O run through background tasks;
  bounded schema/editor conversion stays in the controller. Install loaded
  pages only after the whole project is editable. Use request IDs to
  reject stale completions; preserve numbering across New.
- Show the filename and dirty `*` in the heading/title. Add New/Open/Save/Save
  as and Ctrl/Command+N/O/S/Shift+S. Native close requests use the same guard.
  Keep confirmation state on focus loss; Escape keeps the project.
- Identify saved content by ordered page IDs plus annotation revisions.
  Replacement gets a new page ID; selection/navigation/defaults do not count.
  Undo/redo restores dirty identity. Page operations still are not undoable.
- Snapshot all pages, retain original PNG/JPEG bytes, and encode blank pages
  as white PNGs. Use the tested atomic writer for native Save and Save as.
  Restore clean state/path only after success; retain successful folder choices.
- Use a 612-point width for image and blank pages. Retain exact source pixel
  dimensions in image metadata, and derive 1:1 from those pixels rather than
  point units. Update thickness labels to points. Renderer fixtures explicitly
  keep pixel-aligned test units so old raster assertions stay meaningful.
- Cover real ZIP save/reload, multiple pages, byte equality, stable IDs,
  fresh loaded history, native disk writing, page reorder/replacement dirty
  state, platform command keys, and command-key release during save.
- Keep browser upload/download-save buildable using the existing dialog API.
  Enable existing dependency features for file-size checks before browser reads
  and UTC time without unsupported WASM system-clock calls. A download receipt
  is not proof of disk persistence; browser tab closing remains unguarded.
  Browser archive/image processing still runs on its event thread.
- Full v1 archives remain readable, but editable imports retain 5D1's temporary
  crop/filter/affine/font/layout restrictions. Do not silently flatten/drop
  them.
  No new schema yet. User confirmed the manual file-workflow checkpoint;
  mark 5D2 accepted.
- Passed 206 all-feature tests, 123 core-only tests, strict Clippy, formatting,
  all-feature WebAssembly compilation, and a linked Trunk build locally.

Manual checkpoint — accepted (use copies for legacy files):

1. Annotate the blank page with shapes, rotated/multiline formatted text, and
   ink. Save, New, and reopen; compare placement, styles, layers, and text.
2. Create several imported image pages with annotations. Reorder and save;
   reopen and check order, page-specific annotations, switching, Fit, and 1:1.
3. Save an edit; make another edit and undo/redo. Check that the dirty `*`
   clears only at the saved revision. Selection/zoom/default changes stay clean.
4. Test New/Open/native window close while dirty: Cancel, Discard, and Save.
   Cancel the save dialog; verify the old project remains open. Save as creates
   another file; later Save updates it without another destination picker.
5. Try a corrupt/unsupported project and an unwritable save destination;
   verify a visible error, unchanged current data, and unchanged old save file.
6. Check Ctrl+N/O/S/Shift+S (Command on macOS), remembered project folders,
   and ordinary mouse editing. Browser upload/download is optional; save before
   closing/reloading its tab. Native touch remains a hardware checkpoint.

User requested a stop at 5D2. Resumed below after the green CI report.

### 2026-10-15 — checkpoint 5E multi-page PDF import

- User reported green GitHub CI/CD for the accepted 5D2 commit. Continue with
  PDF import only; matching PDF export and recovery remain separate slices.
- TDD red: core import APIs and controller workflow did not exist. Core tests
  cover sizing, page order, white backgrounds, printed standard-font text,
  crop/rotation/UserUnit, invalid input, count/pixel budgets, cancellation,
  renderer warnings, and the tracked printable sample form.
- Select Hayro 0.8 with embedded substitute fonts/CMaps and CPU rendering.
  It builds without a separate PDFium/native renderer installation. Keep the
  GUI-independent adapter behind `pdf-import`, enabled by the GUI feature;
  core-only tests need not compile the renderer. Existing `pdf` export probe
  remains separate and unchanged.
- Import every PDF page at 300 DPI as a white-backed PNG. Stage all images
  before inserting after the active page; replace only an empty starter.
  Retain annotated starter pages, other page IDs, selection, and history.
  Printed PDF content/forms become image backgrounds, not editable objects.
- Check input length before parsing, all page dimensions before rendering,
  and encoded output size before installation. Limits: 100 MiB input,
  20 project pages, 50 MP/page, 200 MP/batch, 600 MiB encoded batch output.
  Fail the complete batch on reported content warnings or native renderer
  panic. These limits are not an internal memory/CPU/decompression sandbox;
  use trusted PDFs and visually verify complex content.
- Native import owns one worker thread. Page progress/cancellation use the
  existing project-operation guard and request IDs. Cancel discards the batch
  and releases the UI; an in-flight page can finish but its late result is
  ignored. Cannot interrupt a single page or parsing. Reuse image folder memory.
- Keep the browser upload path buildable, but explicitly document synchronous
  browser rasterization and unavailable progress/cancel during that work.
  A browser worker runtime is outside this small native-first checkpoint.
- Controller checks cover all-or-nothing insertion, an annotated starter,
  modal/edit guards, stale replies, modifier release, and pending text/drafts.
  A real sample PDF raster passes through annotation, ZIP save, reload, and
  editable page reconstruction with stable IDs and exact raster dimensions.
- Remove stale prototype README claims that project open/save is absent and
  all content is temporary. Document PDF behavior and current limitations.
- Passed 224 all-feature tests, 123 core-only tests, strict Clippy, formatting,
  all-feature WebAssembly compilation, and a linked Trunk build locally.
  New Windows/macOS runtime checks still rely on CI and manual testing.

Manual checkpoint — accepted, with performance/selection follow-up below:

1. Import `docs/samples/fastfill-camera-sample-form.pdf` into a new project.
   Check printed text, lines, checkboxes, white paper, Fit, and pixel-based 1:1.
2. Import a multi-page PDF, ideally including landscape/rotated/cropped pages.
   Check order, orientation, aspect ratio, and successful folder memory.
3. Annotate an existing page, then import another PDF. Check insertion after
   that page, retained annotations/history, and an unsaved `*`. An annotated
   starter blank must remain rather than being silently replaced.
4. Annotate imported pages, reorder, save as `.docscan`, New, and reopen.
   Check all page images, annotations, IDs/order behavior, and clean state.
5. Cancel the picker or an in-progress multi-page import. Try a damaged PDF
   and a PDF exceeding remaining page capacity. Check that no partial pages
   appear and existing content remains usable. Password PDFs have no prompt.
6. Confirm progress and Cancel remain responsive in the native app. Browser
   upload is optional; large imports can freeze the harness while rendering.

Stop for this checkpoint; do not start PDF export or recovery yet.

### 2026-10-15 — 5E performance and mixed-selection follow-up

- User confirmed functional import checks. Reported slow import/open/save,
  page images appearing after annotations, minimal progress, and missing text
  from marquee/Ctrl-click groups. Do not advance to export yet.
- Found the supplied sample at `artifacts/reviews/docscans/sample-page.docscan`
  rather than `artifacts/previews/`. Its 1.3 MiB archive holds 18.8 MP of image
  data, about 75 MB as RGBA. Unoptimized local timings: validated open 0.91 s,
  validated ZIP write 0.81 s, and full read-back decode 0.81 s, excluding UI
  decoding. Add a small rerunnable `project_timing` example.
- Reuse pixels from bounded archive validation when staging GUI pages. Keep
  original PNG/JPEG handles for saving; use decoded RGBA handles for the page
  and rail. Share a display handle across repeated asset references. Install
  only after the entire project succeeds; no UI-thread image decode gap.
- Retain rendered PDF pixels for display and encode PNG with the existing
  image codec's fast default. This trades some compression for speed, not
  image quality. Keep 300 DPI and all input/output limits unchanged.
- Optimize dependencies in debug/test builds while leaving FastFill's code
  debuggable. Accept the one-time dependency rebuild; subsequent app edits
  reuse it. Keep agent build output under `/tmp`.
- Remove a redundant full decode in GUI save snapshot preparation. The archive
  writer still fully validates every source image before writing. Native
  read-back checks exact manifest/asset bytes, entry count, and ZIP CRC without
  decoding the already-validated, byte-identical image again. Keep atomic commit
  and failed-save preservation. Test changed/short/long/missing/corrupt entries.
- Add parsing/interpreting/rasterizing/compression stages and a work-stage
  progress bar, including single-page imports. Cancellation checks stage
  boundaries; the bar is not a time estimate. Browser rendering still blocks.
- Extend selection/history to several text objects plus shapes. Marquee and
  Ctrl-click include text; group movement uses a common reachability-limited
  delta, preview, cancel, and one-step undo. Apply shared color, compatible
  stroke/fill, deletion, layer order, and rotation reset to selected members.
  Show faint text-member frames; retain single-object edit/rotation controls.
- Do not add archive thumbnails or a new schema yet. Profile the corrected
  pipeline first; previews could be a later optimization for very large files.
- The same local sample now opens/validates in 82 ms and writes/validates in
  66 ms. Actual native atomic save including byte/CRC read-back takes 67 ms.
  These are container measurements, not native GUI/SP6 latency promises.
- Passed 234 all-feature tests, 129 core-only tests, strict Clippy, formatting,
  and all-feature WASM compilation. Native UI responsiveness and mixed-selection
  feedback still need the checks below.

Follow-up manual checkpoint — accepted, with font-control follow-up below:

1. Reimport the same PDF and check stage messages/bar and native cancellation.
2. Reopen `sample-page.docscan`; check that images and annotations appear
   together, then compare open/save responsiveness with the prior build.
3. Use marquee and Ctrl-click to select text plus shapes, or multiple text
   boxes. Check faint member frames, movement, cancel, color, layer changes,
   delete, and undo/redo. Single-text editing/rotation must still work.

### 2026-10-15 — mixed-selection font controls

- User confirmed near-instant load/save, visible progress, text marquee and
  Ctrl-click selection, and shared color changes. Accept those follow-up checks.
  Font, size, and B/I/U controls still disappeared for multi-selection.
- Show text controls whenever selected text exists, including mixed groups.
  Retain shape thickness/fill and shared colors; scroll the options row
  horizontally rather than clipping controls in narrower windows.
- Display mixed typefaces/sizes with a placeholder. Mark B/I/U active only
  when all selected text shares the attribute; click to enable all otherwise,
  or disable all when already active.
- Apply only the requested attribute to each selected text, retaining its
  other formatting, content, ID, anchor, and rotation. Measure each updated
  text separately. Stage/validate every result before one undoable commit;
  preserve shapes and block formatting during active drags or text drafts.
- Preserve pending inline edit formatting and Escape behavior. Shared color
  still reaches both shapes and text; font attributes apply only to text.
- TDD red: bulk API was missing. New core/controller checks cover mixed
  formatting, independent box measurement, field preservation, invalid batch
  rollback, active drags, shared color, shape thickness, and undo/redo.
- Passed 236 all-feature tests, 130 core-only tests, strict Clippy, formatting,
  and all-feature WASM compilation locally. User confirmed the focused UI
  checks below; mark 5E and its follow-ups complete.

Manual checkpoint — accepted:

1. Select text plus shapes, or multiple texts with different fonts/sizes.
   Check that font controls show alongside compatible shape options, with
   "Mixed" placeholders where applicable.
2. Change font/size/B/I/U. Check only text changes, no unwanted attribute
   copying, boxes update correctly, and one Undo restores the whole action.
3. Check shared color and shape thickness/fill, font-size wheel adjustment,
   and ordinary inline editing/Enter/Escape. Do not start PDF export yet.

Stop after the requested commit. Resume with matching PDF export next, then
recovery as a separate checkpoint; neither is implemented in this slice.

### 2026-10-15 — checkpoint 5F matching PDF export

- Scope: export only, leaving the C# app and recovery unchanged.
- Add tests for page order/dimensions, whole-document preflight, page-boundary
  cancellation, and exclusion of selection/draft overlays before wiring UI.
- Reuse iced's headless software renderer and the existing annotation drawing
  functions. Keep one explicit renderer layer per annotation to retain
  text/path order. A regression test caught that separate geometries alone
  still batch text above paths; mirror the UI stack's layer boundaries.
  Snapshot committed objects without copying selection or undo history.
- Flatten at 300 DPI in page-point coordinates; embed raw RGBA losslessly with
  the existing Krilla dependency. Enable its raster-images feature for GUI
  builds only; the standalone vector PDF probe stays dependency-light.
- Add an Export PDF save dialog, per-page native worker progress, cancellation,
  panic/error reporting, and stale-result guards. Keep edits blocked until the
  export worker acknowledges cancellation, since an atomic commit is final.
- Write native output to a same-directory temporary file, verify exact bytes,
  check cancellation, then replace atomically. Refuse non-PDF destinations.
- Preserve project path/name, saved identity, page selection, navigation, and
  annotation history. Accept pending text before the snapshot; cancel drafts.
  Remember the last successful export directory independently of project I/O.
- Bound output to 20 pages, 50 MP/page, 200 MP total, and 100 MiB PDF bytes.
- Add render/round-trip checks covering formatted Unicode text, rotation,
  translucent shapes/highlighting, clipping, real canvas parity, blank paper,
  and imported backgrounds. Add controller and atomic-write regressions.
- Document raster/non-searchable output, page-point printing dimensions, and
  the existing browser event-thread/download acknowledgment limitations.

Automated validation: all green. All-feature suite: 249 tests; core-only:
130 tests. Formatting, strict all-target/all-feature Clippy, native test
compilation, WebAssembly compilation, and diff whitespace checks pass.
The PDF-only feature build also passes its three vector-probe tests without
the GUI or raster-image feature.
Builds use `/tmp/fastfill-rust-codex-target`, never the host target directory.

Manual checkpoint 5F — passed by user; see follow-up below:

1. Open a copy of a multi-page project, reorder pages, and export. Check count,
   order, backgrounds, and annotation placement in a PDF viewer.
2. Include B/I/U, multiple fonts, Unicode, rotated text, overlapping text/shapes,
   highlighter, and objects crossing the page edge. Check appearance/clipping.
   Try print preview with fit-to-paper; output uses project point dimensions.
3. Export while zoomed/panned and with an object selected. Output must contain
   the full page, without selection handles, toolbars, or unfinished drawings.
4. Export directly from an active text editor. Pending text must be included;
   the editable project must still need saving. Clean projects stay clean.
   Project filename, selected page, selection/history, and navigation remain.
5. Cancel the save dialog. On a larger export, use Cancel PDF export and wait
   for acknowledgment. Existing destination/project data must remain intact
   unless the file had already committed before cancellation.
6. Overwrite a disposable PDF successfully; try an invalid/non-PDF destination
   and check the visible error. Repeat export to check remembered folder.
7. Optional browser harness: download/open the PDF. Large work still blocks
   browser progress/cancel handling; native export is the primary checkpoint.

### 2026-10-15 — 5F acceptance and browser follow-up

- User confirms PDF export works, including native error handling and browser
  project import/PDF export. Mark the export checkpoint accepted. Recovery
  remains separate; address these focused follow-ups before advancing.
- Replace intimidating atomic-write paths in export errors with the chosen
  destination and a useful reason. Keep write/read-back verification context.
- Reproduce page-image bleed over the toolbar with a rendering regression at
  100%/200% DPI. iced clips later stack children but not its first child; use
  an empty base so every page layer, including its background, is clipped.
  Apply the same fix to the scrolling thumbnail canvas; a separate failing
  regression reproduced thumbnails covering the rail's page controls.
- Replace browser save/download modals with one small Blob/download adapter
  shared by PDF export and project saving. Keep native dialogs unchanged.
  Remove the temporary link and revoke its URL after download startup; report
  synchronous browser errors without claiming verified disk persistence.
- Add standalone Node assertions for PDF/project filenames, MIME types, byte
  subarrays, immediate clicks, delayed URL cleanup, and DOM failure cleanup.
  Run this check in CI without adding a JS framework or package dependencies.
- Keep overall WASM responsiveness as a known harness limitation. Document a
  fully optimized application build for comparison before deeper profiling.

Automated follow-up validation: 253 all-feature Rust tests and the standalone
browser adapter check pass. Native/WASM strict Clippy, formatting, and diff
whitespace checks pass. Trunk builds the complete WASM bundle; its generated
JS imports the copied download module correctly. Container build/distribution
outputs stay under `/tmp`, outside the host target and dist directories.

Focused manual recheck — passed by user:

1. Zoom/pan a large image page under the toolbar and toward the rail. Nothing
   should cover controls or their backgrounds, including in the browser.
2. Try an unwritable PDF destination. Error should show the chosen filename
   and a useful reason, with no `.atomicwrite` or random temporary filenames.
3. Browser Save/Save as and Export PDF should start normal browser downloads
   without FastFill's extra modal. Open the resulting `.docscan` and PDF.

### 2026-10-15 — 5F follow-ups accepted

- User confirms all requested native/browser checks passed. Close checkpoint
  5F, including the export error, clipping, and direct-download follow-ups.
- Hide Save as in WASM only: it is equivalent to Save's download-copy flow.
  Keep native Save as and the existing Ctrl+Shift+S shortcut unchanged; the
  shortcut remains a Save/download alias in the browser.
- Add a platform-aware project-controls check and update the prototype guide.
- Validation: 254 all-feature tests, formatting, and strict native/WASM
  Clippy pass.

Next separate checkpoint: 5G recovery. Do not start it in this change.

### 2026-10-15 — checkpoint 5G native automatic recovery

- Add a single native worker for periodic snapshots, loading recovery, and
  ordered cleanup. Reuse the project's bounded archive loader, decoded image
  cache, original encoded assets, and atomic verified writer. No dependency
  or C# application changes.
- Snapshot changed, committed dirty content about every 10 seconds; skip
  pending draws/text and modal file operations. Do not change explicit-save
  identity, history, navigation, or the current editor. Recovery is a safety
  net, not an explicit save; a crash may lose work after the last snapshot.
- Use UUID session names and OS file locks in a private per-user directory.
  Ignore live sessions and foreign links/names. Offer abandoned snapshots
  newest first with Restore, Discard, and Not now. Keep corrupt files until
  explicit discard. Restore an unsaved copy without the original file path;
  keep its snapshot until explicit Save/Discard or a newer atomic snapshot.
- Queue cleanup behind in-flight writes on Save, New, successful Open, and
  normal close. Canceled/failed dialogs retain current work and recovery.
  Wait for cleanup acknowledgment before closing. Ignore late restore results
  once closing, and keep the window open if cleanup fails.
- Show persistent, actionable recovery warnings. Retry snapshot/cleanup I/O
  failures on later ticks without a retry loop; worker/startup failure leaves
  ordinary editing and explicit saving available.
- Per user request, paths use FastFill/fastfill, never a -rust suffix. Keep
  the C# workspaces subdirectory and explicit project files untouched.
- Record browser-local recovery as a possible IndexedDB follow-up. Do not
  add it to this native checkpoint; account for quotas, eviction, private
  browsing, and concurrent tabs before promising browser recovery.
- Automated checks: 271 all-feature Rust tests (17 new recovery checks),
  130 core-only tests, native/WASM strict Clippy, formatting, browser download
  adapter assertions, whitespace checks, and a complete Trunk bundle build.
  Container outputs remain under /tmp; host target/dist stay untouched.

Manual checkpoint 5G — passed by user:

1. Add at least two pages, shapes, and accepted text; reorder pages. Wait
   about 15 seconds, then kill the FastFill process with the system task
   manager (not the window close button). Relaunch: choose Not now, close,
   relaunch again, then Restore. Check page order, images, text, formatting,
   and layers. Restored work must be unsaved, without an original save path.
2. Save the recovered work to a new .docscan. Close and relaunch: no recovery
   prompt for that session. Existing explicitly saved projects must not
   change unless you deliberately save over them.
3. Make another accepted change; wait for a snapshot. Cancel an attempted
   Save as, then force-kill/relaunch: recovery must still exist. Restore,
   make another change, wait, then close normally and choose Discard. The
   next launch must not offer that session. Also check New + Discard.
4. Run two instances. Create unsaved work in each and wait for snapshots.
   Launching another instance must not offer either live project's recovery.
   Kill only one original instance; a new launch should offer only its
   abandoned work. The still-running instance must remain unaffected.
5. Optional error check: set FASTFILL_RECOVERY_DIR to an existing regular
   file, not a directory. Startup should show a visible recovery warning;
   annotation editing and explicit Save must still work. Unset afterward.

User confirms all requested checks passed. Close checkpoint 5G and Phase 5;
browser recovery remains a documented follow-up. Phase 6 is not started.

### 2026-10-15 — Phase 5 accepted

- Record user acceptance of all 5G manual checks and close the final planned
  Phase 5 checkpoint. Keep browser recovery as a documented follow-up.
- Leave the implementation and staged changes unchanged. Wait for an
  explicit request before committing or starting Phase 6.

### 2026-10-15 — Phase 5 commit and checkpoint 6A

- At user request, commit accepted Phase 5 work as unsigned 0d9f1c0. Do not
  push. Keep subsequent Phase 6 changes separate and uncommitted.
- Split Phase 6 into correction geometry, filters, transactional page editing,
  form snapping, still detection, temporal framing, and native capture. Stop
  at each checkpoint; do not combine camera work with this foundation.
- TDD red: ten new correction tests fail at the unimplemented geometry API.
  Green: add bounded four-corner projective correction and quarter turns,
  using the existing image dependency for exact rotations. No new dependency.
- Keep the pipeline independent of iced, filesystem, and project mutation.
  Share finite/in-bounds/convex crop validation with the archive model, using
  f64 cross-products. Reject unusable projections and oversized output before
  allocating a destination. Accept the existing reversed-winding contract.
- Include endpoint pixel centers in dimensions. Full-frame identity keeps
  exact dimensions and bytes; arbitrary crops use inverse bilinear sampling.
  Accumulate premultiplied colors to avoid transparent-edge halos without a
  large float-image copy. Borrow full-frame input while rotating to avoid
  an unnecessary full-size clone.
- Add the native correct_page preview example and a small JSON preset. Bound
  PNG/JPEG reads to 100 MiB and settings to 16 KiB; write PNG atomically with
  no overwrite. Test empty/missing/extra arguments and destination safety.
- Add 14 core correction checks and three preview checks. Cover exact crops,
  skewed/perspective corners, nonfinite/concave/crossed input, output budgets,
  negative/large rotation counts, alpha, single-pixel axes, JSON, and the real
  sample form. Make native CI test all targets so examples run too, and
  trigger checks when the sample PNG changes.
- Validation: 288 all-feature/all-target tests and 147 core/example tests
  pass; strict native/WASM all-target Clippy, formatting, and diff whitespace
  checks pass. Trunk builds the complete browser bundle. Existing GUI behavior
  and C# sources remain unchanged.
- Generate and inspect full-form, cropped/rotated-form, and recorded laptop
  camera previews under artifacts/previews/correction-6a.k02IEf/. The camera
  example uses manually specified corners on folded-paper-0059.jpg, not an
  automatic detection result. Detector/224-frame replay work is deferred.

Manual checkpoint 6A — visual previews accepted by user:

1. Inspect the generated full-form and cropped/rotated-form PNGs. Printed
   text and lines should remain legible, with no missing final row/column.
   The preset removes outer margins, then rotates 90 degrees clockwise.
2. Optionally rerun from rust/ with fresh output filenames:
   `mise exec -- cargo run --no-default-features --example correct_page --`
   followed by input PNG/JPEG, output PNG, and examples/correction.json.
   Omit JSON for identity; change its rotationQuarterTurns to 0 for crop only.
   Rerunning with an existing output must refuse without changing that file.
3. Inspect camera-manual-crop.png for the perspective/rotation smoke check.
   This is a manually framed, folded laptop-camera sample, not an SP6 image
   or an auto-framing test; four corners cannot flatten a physical fold.

User accepts both generated form and camera previews. Command safety remains
covered by automated tests. Proceed to the separate 6B filter checkpoint.

### 2026-10-15 — 6A accepted; checkpoint 6B filters

- Record user acceptance of the corrected form and manually framed camera
  previews. Keep the main editor and C# application unchanged in 6B.
- TDD red: ten filter tests fail at the unimplemented filter dispatcher;
  contrast-bound validation already passes. Green: add schema-compatible
  filters after perspective correction and rotation, with no new dependency.
- Match BT.601 grayscale and the legacy linear contrast curve about 128.
  Validate contrast -100..100 before allocating output; apply contrast last.
  Preserve source pixels, output dimensions, alpha, and hidden RGB at alpha 0.
- Keep gray-world balance plus 8x8 Lab CLAHE with clip limit 2. Use an sRGB
  lookup and in-place Lab bytes instead of a large floating-point image.
  Ignore transparent pixels in statistics; keep flat/empty tiles unchanged
  and blend neighboring tile lookup tables. Test reference Lab colors,
  contrast gain, boundary seams, cast removal, and colored marks.
- Reuse image's optimized byte Gaussian blur with the legacy 31-pixel
  neighborhood, sigma 5, and threshold offset 12. Treat transparent neighbors
  as white paper. Intentionally omit median blur: synthetic rules, dots, and
  faint strokes must survive; the real form retains over 95% of dark ink.
  Camera noise and very faint ink remain visual calibration concerns.
- Extend the preview JSON with the existing numeric filter schema and add
  examples/filter.json. Test settings-to-PNG, invalid contrast, empty args,
  source/output safety, pipeline ordering, and tiny images. Print CLI errors
  using their human-readable message.
- Validation: 303 all-feature/all-target tests and 162 core/example tests
  pass; strict native/WASM all-target Clippy, formatting, and whitespace
  checks pass. Trunk builds the browser bundle. Keep container builds under
  /tmp, away from host targets.
- Generate full-size original, grayscale, enhanced (+10 contrast), B&W,
  and original +35 contrast for both the form and the manually cropped
  laptop-camera frame. Comparisons live under
  artifacts/previews/filters-6b.V3q7ss/index.html. Click images for full size.
  Camera corners are manual; no detection/224-frame replay is claimed here.

Manual checkpoint 6B — original filters accepted 2026-10-15:

1. Open the comparison HTML or the individual PNGs. Check the printed form's
   small text, dotted fields, fine rules, checkboxes, and tinted headers.
2. Compare the camera's color cast and handwriting across modes. Enhanced
   color can amplify compression/noise; B&W can lose very faint writing.
   Grayscale/original remain available when thresholding is unsuitable.
3. Optional: run correct_page with examples/filter.json and a fresh output
   filename. Edit mode 0/1/2/3 or contrast -100..100; bad settings must fail
   without creating output or changing an existing file.

User accepted the shown filters, including the laptop-camera comparison.
Keep these samples; no replacement color document is needed. Brightness is
an additional checkpoint before 6C; do not start page editing or commit yet.

### 2026-10-15 — 6B brightness correction

- TDD red: the preview-settings test rejects the unknown brightness field.
  Green: add brightness -100..100, default 0, to PageCorrection. Keep the
  schema-v1 filter DTO unchanged; defer brightness persistence to schema-v2
  page editing in 6C, with schema-v1 loading retained.
- Apply additive brightness after the color mode, combined with contrast in
  the existing byte lookup. Clamp/round once to avoid losing recoverable
  tones between adjustments. No new dependency or second image pass.
- Add five tests for JSON defaults/round trips, RGB offsets/limits, combined
  contrast clipping, every mode, alpha/hidden RGB, source preservation, and
  invalid settings at both correction entry points. Extend the CLI test to
  verify brightness output and reject bad settings without creating output.
- Document the formula and add examples/brightness.json. Extend the existing
  comparison HTML with the same manually cropped laptop-camera image at
  brightness -15, 0, +15, and brightness +10 with contrast +25. Inspect the
  generated PNGs; retain the original samples and accepted filter outputs.
- Validation: 308 all-feature/all-target tests and 167 core/example tests
  pass. Strict native/WASM all-target Clippy, formatting, whitespace checks,
  and the full Trunk browser build pass. All build output remains under /tmp.

Manual checkpoint — brightness accepted 2026-10-15. User inspected the
brightness comparisons and approved completion of 6B. Commit the pending
6A/6B work, then start 6C separately. The main editor and C# app remain
unchanged in this commit.

### 2026-10-15 — 6B committed; checkpoint 6C1 compatibility

- Mark 6B accepted and create unsigned commit f88ae26 for the pending 6A/6B
  work. Do not push. Keep subsequent 6C changes uncommitted for review.
- Split 6C into corrected-project I/O and interactive page editing, each
  with its own checkpoint. Keep correction controls out of 6C1.
- TDD red: v2 archives reject the brightness field; staged cropped/rotated
  images still have source dimensions (9x7 instead of 7x5). Green: accept
  schema-v1/v2 and stage each page's corrections from its original pixels.
- Base annotation coordinates on a 612-point width and corrected output
  aspect. Remove blanket rejection of crop/rotation/filter metadata; retain
  explicit rejection of unsupported affine/font/soft-wrap layouts. Test
  rotated shape/ink frame corners on a corrected page.
- Persist brightness as a page-level v2 field. Omit zero for v1 compatibility;
  missing schemaVersion remains legacy v1. Upgrade snapshots only when
  nonzero brightness needs v2, and never implicitly downgrade loaded v2.
- Validate correction/output limits before decoding assets. Decode each
  source asset once and stage corrected displays before installing any page.
  Share source handles, not corrected results, across differently edited
  pages. Use corrected display dimensions for the rail, 1:1, and PDF aspect;
  keep source info/encoded bytes intact for saves and recovery.
- Test all color modes, shared assets with different corrections, unchanged
  source bytes, annotation editing and reload, v1/v2 round trips, schema
  failures, preflight output limits, and actual corrected PDF pixels/aspect.
- Validate the existing Scan 2026-09-15 11-19.docscan through the GUI's
  open/save pipeline. Generate three-page v1/v2 fixtures under
  artifacts/previews/correction-6c1.sXnKAa/ and validate both through the same
  pipeline. The v2 camera page adds brightness +15; both retain source assets.
- Validation: 316 all-feature/all-target tests and 170 core/example tests
  pass. Native/WASM strict all-target Clippy, formatting, whitespace, and the
  full Trunk build pass. Keep all compilation output under /tmp.

Manual checkpoint 6C1 — accepted 2026-10-15:

1. Open correction-v1.docscan and correction-v2.docscan from the generated
   fixture directory. Inspect original form, cropped/rotated grayscale
   form, and enhanced camera page. V2's camera page should be brighter.
2. Check Fit, 1:1, wheel zoom, page thumbnails, and annotation selection.
   Move/resize a rectangle and edit a text label; undo/redo those changes.
3. Save as to a new filename, reopen, and verify corrections/annotations
   persist without repeated cropping, rotation, or brightness changes.
4. Export PDF; verify page orientation, backgrounds, annotation placement,
   and page order match the canvas. Optionally open the existing 11-19 scan.

User confirmed all checks. Record the font-size control mismatch: the
fixtures use 22 pt, but the dropdown offers sizes in 4 pt increments. Defer
replacement with a 1 pt spinner to Phase 7 UI polish, as requested.

Next: 6C2 adds correction controls, transactional previews, accessibility of
displaced objects, and correction-aware dirty/history/recovery behavior.
The C# application remains unchanged. No new commit requested.

### 2026-10-15 — 6C2 interactive page editor

- Add Edit page to the rail for image pages. Keep correction input, view,
  preview work, and per-page version handling in focused modules rather than
  expanding the main controller. Add no dependencies; leave C# untouched.
- Show the original image with a translucent quadrangle and mouse/touch
  corner handles, alongside a corrected image preview. Add Full frame,
  quarter-turn rotation, all four color modes, and brightness/contrast
  sliders with 1-step buttons. Reject crossed or invalid corners.
- Keep draft settings and pixels outside committed page state. Accept only
  a completed preview matching the draft; Cancel/Escape change no page data.
  Window resize/focus loss preserve the draft, aborting handle gestures.
  Block other document commands and require Accept/Cancel before closing.
- Run native preview work off the UI thread, retaining decoded originals
  between changes. Permit one running job plus the latest queued settings;
  reject stale settings/session completions. Browser work remains on its
  UI thread, matching the current PDF-import harness limitation.
- Clarification: image-space anchors, not fixed page-point positions, govern
  annotation placement after correction. Map centers through the original
  image, and scale width/height independently. Keep strokes and annotation
  rotation fixed; do not apply perspective skew or warp to annotations.
  Text follows vertical magnification with natural glyph proportions.
  Reject unsupported resulting font sizes (6–144 pt); never drop objects.
- TDD red: composing perspective derivatives accumulates annotation size
  drift on crop/reset. Green: compare local magnification in fixed source
  axes, making forward/reverse scale ratios cancel at the image anchor.
  Filter-only changes preserve geometry exactly without float rebuilding.
- After image-space mapping, translate only wholly off-page annotations
  enough to remain reachable. Warn explicitly when this extra move occurs.
  Partly visible items need no additional translation or clipping constraint.
- Stage/validate all mapped annotations and metadata before committing.
  Couple page-version keys to existing annotation snapshots, making each
  accepted correction one undo step with exact pixels/geometry restoration.
  Support interleaved annotation/page edits; release unused image versions
  when a new edit discards redo. Retain original encoded image bytes.
- Test preview coalescing, stale/error results, canceled/reopened sessions,
  invalid crops, no-op acceptance, modal guards, dirty state, original bytes,
  Save/reload, recovery, reachability warnings, and exact undo/redo. Cover
  independent geometry scales, natural text scaling, pending/stale staged
  changes, perspective round trips, rotation, and unclamped image anchors.
- Exercise mouse/touch handle ownership, release positions, and lost-touch
  rollback with synthetic events. Render the real sample form offscreen;
  assert overlay/header visibility. Use the bundled font and a redraw event
  in the headless helper, avoiding unavailable container fonts and
  uninitialized button status. Inspect page-editor-readable.png under
  artifacts/previews/page-editor-6c2.d8M45H/.
- Validation: 343 all-feature/all-target tests and 180 core/example tests
  pass. Strict native/WASM all-target Clippy, formatting, whitespace checks,
  and full Trunk build pass. All compilation output stays under /tmp.

Manual checkpoint 6C2 — functional checks accepted 2026-10-15:

1. Run the native app and Add image, or open a corrected project copy. Choose
   Edit page in the rail. Confirm original corner handles and corrected
   preview are separate and legible; try touch if a device is available.
2. Drag all four corners. Try an invalid/crossed crop, then restore a valid
   crop or Full frame. Check both rotation buttons, every color mode, and
   brightness/contrast. Rapid slider changes must settle at the last value.
3. Cancel or press Escape: page, annotations, dirty state, and history stay
   unchanged. Resize/switch focus while editing: keep pending settings.
4. With existing rectangles, lines, ink, and text, accept a crop/rotation.
   Centers follow image locations, shape axes scale independently, strokes
   and object angles stay fixed, and text keeps natural glyph proportions.
   No annotation warping is expected. The draft preview omits annotations.
5. Crop enough to displace an annotation completely. Confirm a visible
   warning and a reachable object. Partly visible objects need no extra move.
   Undo/redo must restore the image, settings, and annotations together.
6. Interleave page corrections with annotation edits and check undo/redo.
   Save as, reopen, and export PDF; confirm geometry/settings persist and
   corrections are not applied twice. Check recovery if practical.

Pause for user acceptance before 6D form snapping. No commit requested.

### 2026-10-15 — 6C2 font-label and preview-performance follow-up

- User confirms page editing, image-space annotation anchoring, quick
  undo/redo, PDF export, and displaced-object warnings. Record two issues:
  scaled font-size digits overflow the dropdown, and each live correction
  takes several seconds with high CPU use. Physical touch remains untested.
- TDD red: the compact font-size display type is absent. Green: round labels
  to at most one decimal place, but retain the original floating-point value.
  Do not round document geometry repeatedly during crop/reset. Keep the
  1 pt font spinner deferred to Phase 7, as previously agreed.
- Native debug builds optimized only dependencies; FastFill's perspective
  and filter loops remained unoptimized. Enable level-1 optimization for
  application code while retaining debug symbols/assertions and level-2
  dependency optimization. This also applies to the browser harness.
- Measure the existing CLI on the 1241x1754 sample form, including decode
  and PNG output. Crop/rotate drops from 1.62 s to 0.68 s elapsed (1.57 to
  0.53 s user CPU); enhanced color from 2.38 s to 1.31 s (2.35 to 0.99 s
  user CPU). Outputs before/after have identical SHA-256 hashes. These are
  container measurements, not a guarantee for host or Surface hardware.
- Keep full-resolution previews and the existing single-worker/coalescing
  behavior. Try the compiler setting before adding thumbnail correction,
  caching layers, or a second asynchronous full-resolution Accept pipeline.
  Comparisons are under artifacts/previews/correction-performance.0aM0As/.
- Validation: 344 all-feature/all-target tests and 180 core/example tests
  pass. Strict native/WASM all-target Clippy, formatting, whitespace checks,
  and full Trunk build pass. Compilation remains isolated under /tmp.

Follow-up checkpoint — accepted 2026-10-16: user confirms all checks passed,
near-instant page editing, and compact one-decimal font labels. Mark 6C
complete and commit its pending work unsigned, as requested. Keep subsequent
6D form-snapping changes separate and stop at its next manual checkpoint.

### 2026-10-16 — 6C committed; portable form snapping checkpoint 6D1

- Commit accepted 6C1/6C2 work unsigned as 154542c. The owner subsequently
  signs history; current commit 308314a has the identical tree. Do not push.
- Split 6D into portable form detection, global smart-snap placement, and
  Ctrl-assisted page corners. Keep each checkpoint independently testable;
  do not add placement/UI state before the detector comparison is accepted.
- TDD red: form-feature analysis is absent. Green: add normalized checkbox
  and solid/dashed/dotted rule candidates with nearest-feature queries.
  Keep the calibrated 1600-pixel analysis edge, 31-pixel Gaussian window,
  threshold offset 10, box-size/aspect limits, broken-mark grouping,
  gap trimming, and short-axis distance units. Reject invalid queries.
- Use installed image resizing/blur and a small eight-connected component
  walk instead of introducing OpenCV. Replace horizontal opening with row
  runs. Include enclosed rectangular white regions so boxes connected to
  larger table borders survive. Composite transparency on white first.
- Intentional limit: axis-aligned edge/interior checks, not arbitrary
  perspective/rotated contour approximation. Intended input is corrected,
  upright form pixels. Image resampling is not OpenCV pixelwise parity.
- TDD red from visual review: padded letter counters produce 72/60 boxes
  instead of the forms' five/14. Green: check rectangular white interiors
  as well as ink frames and narrow the edge bands. Lock both actual counts;
  retain the real checkboxes while excluding letters in titles/body text.
- Copy the existing Hungarian form PNG into tests/samples/datum-form.png
  so Dátum/short-dotted-line regressions also run on CI. Extend workflow
  triggers to all sample fixtures. Leave C# algorithms/labs untouched.
- Add tests for attached boxes, separate same-row fields, short dots, label
  exclusion, ordinary text, transparency, small/empty images, normalized
  output at the final row, invalid queries, and resized-image consistency.
- Composite onto white before grayscale downsampling; hidden transparent
  RGB otherwise makes false ink rules. Keep a regression for those fringes.
- Add form_probe: bounded native PNG/JSON diagnostics with atomic,
  non-overwriting output. Test empty/malformed arguments, candidate overlay
  pixels, existing outputs, and invalid input without filesystem changes.
- Generate and inspect both refined overlays under
  artifacts/previews/form-snap-6d1.UbfZ3p/. Sample form: 14 boxes, 40 rules;
  Hungarian form: five boxes, 33 rules. Table borders are valid rules, not
  OCR-classified fields. No new application snapping controls in this slice.
- Validation: 359 all-feature/all-target tests and 195 core/example tests
  pass. Strict native/WASM all-target Clippy, formatting, whitespace checks,
  and full Trunk build pass. Keep all compilation output under /tmp.

Manual checkpoint 6D1 — baseline accepted 2026-10-16; follow-up below:

1. Open the generated comparison index.html or sample-form-refined.png and
   datum-form-refined.png. Blue boxes should match real checkboxes, not
   enclosed letters. Orange rules should leave printed labels/text alone.
2. Inspect part B's dotted field, the Dátum field, the three short dotted
   fields near év/hónap, and separate same-row solid fields. Test another
   PNG/JPEG with form_probe if useful; use a new output filename each time.
3. Accept detector output before 6D2 placement. Corner snapping stays 6D3.

### 2026-10-16 — legacy text loading compatibility follow-up

- User requires loading existing C# projects, without adding Rust auto-wrap.
  Inspect Scan 2026-09-15 09-00.docscan: two short texts, hahó and bubu.
  Their absent maximumWidth fields default to 0.4; neither word needs wrap.
  The prior rejection treated a creation/fitting hint as proof of soft wrap.
- TDD red: setting a valid legacy width hint blocks editable conversion.
  Green: remove that false rejection. Keep annotation content, explicit
  newlines, bounds, translations, rotation, and untouched schema metadata.
  Retain existing format/range validation; do not introduce wrapping state.
- Add regression coverage for missing early-C# font/width fields, accented
  content and explicit newlines, annotation geometry, and unchanged saves.
  Genuine legacy automatic wrapping may flow differently under Rust's
  explicit-newline policy; document this without blocking project loading.
- Inspect the original C# font picker too: Arial/Calibri are valid choices
  missing from the Rust mapping. Load them as Sans and preserve their family
  names through non-font edits, rather than reject legacy pages or silently
  rewrite family metadata. Test every original picker choice and Consolas.
- Run the actual 09-00 archive through the GUI's open/save/reopen pipeline
  using FASTFILL_DOCSCAN_FIXTURE. Both pages load successfully. Repeat for
  11-19, blank-with-annot, images-with-annot, and sample-page: all five
  provided .docscan files pass. No source files change. Leave this follow-up
  uncommitted for the user's check.

### 2026-10-16 — click selection and table-cell detection follow-up

- User confirms legacy loading succeeds and detector output matches C#.
  Keep table/box top edges as valid candidates; do not add OCR filtering.
- TDD red: an empty-space click inside overlapping diagonal-line frames
  selects a group. A zero-area marquee still intersects those frames.
  Green: require 4 mouse/8 touch screen pixels before displaying or applying
  a marquee. Derive page-space thresholds from zoom at gesture start.
  Preserve ordinary single-object picking, Ctrl-click, real marquee drags,
  and dragging an existing group from its members' frames.
- Cover exact clicks and small jitter at three zoom levels for mouse and
  synthetic touch. Confirm real drags select both overlapping frames and
  a subsequent click on an actual stroke selects only one object.
- TDD red: a synthetic three-column, three-row table snaps to full rows.
  Green: add cell segments where vertical ink connects two horizontal rules.
  Coalesce thick divider columns; prefer the shorter candidate on distance
  ties. Keep every original row rule, including top and bottom borders.
- Cover shared stacked borders, T-junctions, merged rows, scaled images,
  thin/thick rules, and text stems touching just one rule. Retain real-form
  tests, including checkboxes, Dátum, short dots, and part B's dotted field.
- Do not wire annotation snapping or change C# code in this follow-up.
  Ask for a crowded-area click check before continuing to 6D2 placement.
- Validation: 363 all-feature/all-target tests and 199 core/example tests
  pass. Strict native/WASM all-target Clippy, formatting, and whitespace
  checks pass. Physical touch remains untested; use synthetic touch events.
  Keep all compilation output isolated under /tmp.

### 2026-10-16 — consistent marquee geometry follow-up

- User accepts click-drag slop, but notes small marquees still pick distant
  diagonal lines/arrows when only their outer bounds intersect the marquee.
- TDD red: reproduce bounding-box-only hits for diagonal lines, arrowheads,
  open/filled pen, highlighter, and rotated frames. Green: intersect line
  shafts/arrowheads and transformed ink paths, including stroke thickness.
  Test filled ink as a polygon, including its closing stroke. For bounded
  shapes and text, intersect the actual rotated frame, matching point picks.
- Keep mouse/touch drag thresholds, Ctrl-click membership, and existing
  selected-frame movement. A small empty marquee must no longer select
  overlapping diagonal frames; a larger one crossing their strokes must.
- Put rectangle hit queries in one focused annotations/marquee.rs module.
  Keep geometry independent of GUI rendering and avoid new dependencies.
  Test contained polygons and edge-only overlap, not just corner inclusion.
  Measure stroke distance instead of inflating marquees along screen axes;
  diagonal strokes must not acquire wider hit areas at rectangle corners.
- TDD red: thick line edges at high zoom and filled pen closing edges are
  not clickable. Green: include physical line width and the filled closing
  stroke in existing point queries, retaining the mouse/touch hit tolerance.
- Pause for a crowded-area mouse check. No commit or 6D2 wiring requested.
- Validation: 373 all-feature/all-target tests and 209 core/example tests
  pass. Strict native/WASM all-target Clippy, formatting, and whitespace
  checks pass. Mouse/touch thresholds retain synthetic coverage; physical
  touch remains untested. Compilation output stays isolated under /tmp.

### 2026-10-16 — selection gesture intent follow-up

- User reports marquee starts over selectable objects immediately moving
  or resizing them. Ask about the intended gesture rules; user approves
  click-to-select, unselected-drag-to-marquee, selected-drag-to-edit, and
  Ctrl+drag forcing a fresh marquee while Ctrl-click toggles membership.
- TDD red: presses on unselected bodies/corners immediately edit them;
  Ctrl presses toggle too early to allow forced marquees. Green: defer
  ordinary/Control click picks until release, retaining the previous
  selection while pending. Marquee completion replaces selection; cancel,
  foreign pointers, and invalid releases cannot commit a click or edit.
- Latch movement past the existing mouse/touch screen-pixel threshold.
  A real drag returning to its start must not become a toggle click.
  Keep geometry-based marquee hits and selection changes outside history.
- Preserve selected-object, selected-group, and drawing-tool edits. Keep
  Shift/Alt explicit move/resize overrides, including Ctrl constraints.
  In Select, press Ctrl after starting a handle drag to constrain it;
  Ctrl held before pointer press instead forces a marquee.
- Match cursor feedback to pending/forced marquees. Only selected text
  offers the move cursor; group members retain their selected move target.
- Cover mouse and synthetic touch at three zoom levels across rectangle,
  oval, line, arrow, pen, highlighter, and text. Test corners, rotation
  handles, Ctrl toggling/replacement, cancellation, pointer ownership,
  release positions, no content/history changes, and cursor feedback.
  Update earlier click fixtures to send both pointer press and release;
  explicitly select a loaded object before testing its undoable move.
- Pause for manual mouse checks. Do not commit or begin 6D2 placement.

### 2026-10-16 — selected-group cursor follow-up

- User confirms gesture behavior, but group members show a crosshair even
  though dragging moves the group. The cursor queried only single-object
  controls and selected text; group hit testing lived in the press handler.
- TDD red: group-member edit hits are absent. Green: share group targets
  through the existing selected-object hit query and use that result for
  both cursor feedback and gesture dispatch. Retain move access through
  members' frames, but do not treat empty combined-group gaps or an
  unselected topmost object as group-move targets.
- Keep the move cursor throughout an active group drag. Only Ctrl held
  before a new press offers a forced-marquee crosshair; Ctrl pressed during
  an existing edit cannot change its cursor to a different gesture mode.
- Cover shape, ink, and text groups, mouse/touch, three zoom levels, invalid
  query scales, overlapping unselected layers, Ctrl override, and cursor
  feedback before/during movement. Retain all earlier selection tests.
- Pause for the cursor check; leave changes uncommitted and 6D2 untouched.
- Validation: 383 all-feature/all-target tests and 218 core/example tests
  pass. Strict native/WASM all-target Clippy, formatting, and whitespace
  checks pass. Keep compilation isolated under /tmp; physical touch remains
  untested beyond synthetic input coverage.

### 2026-10-16 — 6D1 accepted; smart-snap placement checkpoint 6D2

- User accepts the final selected-group cursor follow-up. Close 6D1 and
  connect the existing portable detector without changing its calibration
  or the C# implementation. Leave all current changes uncommitted.
- Add a session-global, default-off Smart snap button for Text, Check,
  and Cross. Retain its state across tools, pages, and projects, including
  temporary selected-object editing. Do not serialize it or dirty history.
  Intentionally reset it at application restart; no preference file yet.
- Keep scheduling/cache/placement in app/snap, pixel work in its worker,
  and the two small draft-placement hooks in the portable annotation core.
  Reuse corrected display identities, existing tasks, and undo history;
  add no dependencies or new annotation/document types.
- Native analysis uses one background thread at a time. Coalesce page
  changes to the current image after it finishes. Cache only live displayed
  images, reject removed/replaced/corrected images and other projects, and
  process completions even while a modal editor is open. Browser analysis
  intentionally uses the existing UI-thread fallback until worker support.
- Query in page/image coordinates with the C# short-axis radius of 0.04,
  independent of zoom. Clicked/tapped marks use centered 78% inset squares;
  moving beyond 8 logical screen pixels keeps manual drag sizing instead.
  Preserve styles and existing-object editing. Freeze each gesture's hint;
  late completions cannot alter drafts or committed annotations.
- New text starts at a nearby rule's left end, bottom-anchored above it.
  Reuse natural-width measurement, explicit newlines, format controls, and
  table-cell candidates. Content/font changes keep its bottom on the rule;
  later editing of accepted text does not snap it again. Save only ordinary
  annotation geometry, not an ongoing relationship to the printed rule.
- Show Off/No image/Analyzing/Ready/Unavailable. Pending, missing, failed,
  and unmatched features fall back to normal placement. Cache failures to
  avoid retry loops; toggling off/on explicitly retries. Analysis alone
  cannot modify annotations, history, or project dirty state.
- TDD: draft tests cover inset previews, manual drags, existing edits,
  pointer ownership, invalid hints, text anchoring, styles, and history.
  App regressions cover zoom, synthetic touch, global state, stale results,
  page coalescing, live cache pruning, correction undo/redo, modal delivery,
  failures/retries, the real sample form/Dátum fixtures, and table cells.
  Save/reload retains text/mark geometry within existing normalized-float
  precision and preserves original source bytes. Native worker delivery
  also runs without a GUI runtime.
- Additional TDD red: a rejected duplicate pointer press could overwrite
  an active mark's frozen hint. Green: attach hints only after a successful
  interaction start. Foreign pointers and canceled gestures stay harmless.
- Validation: 402 all-feature/all-target tests and 222 core/example tests
  pass. Strict native/WASM all-target Clippy, formatting, whitespace, and
  Trunk browser-build checks pass. Keep build output isolated under /tmp;
  physical touch remains untested beyond synthetic input coverage.

Manual checkpoint 6D2 — accepted 2026-10-16:

1. Open tests/samples/sample-form.png. Select Check or Cross, enable Smart
   snap, and wait for Ready. Click/tap several boxes at different zooms;
   marks should fit inside them with the chosen color/thickness. The snap
   button should remain available after placement. Drag to size manually;
   moving/resizing an existing mark must not snap it somewhere else.
2. Select Text and click above solid/dotted fields, including Part B.
   New text should begin at the field's left end and stay immediately
   above its rule while typing/changing format. Explicit newlines work;
   long text does not wrap automatically. Accept/Escape and later editing
   keep their usual behavior. Check Dátum in tests/samples/datum-form.png
   and a multi-column bordered table if available.
3. Switch tools, pages, and projects: the toggle should stay on during the
   run, but a fresh app launch starts off. Turning it off restores ordinary
   placement. Blank pages and analysis still pending also place normally;
   objects must not jump when analysis finishes.
4. Accept a crop/rotation/filter change and wait for Ready before placing
   again. Snap must follow the corrected image, including after undo/redo.
   Snap creation itself is one undo step; canceling a gesture adds none.
5. Save/reopen and export a PDF. Snapped marks/text should stay where
   placed. Browser smoke checks are useful if convenient; expect a brief
   UI-thread analysis pause rather than native background responsiveness.

Stop here for user acceptance; do not begin 6D3 or create a commit.

### 2026-10-16 — 6D2 accepted; annotation interaction follow-up

- User accepts smart-snap placement and reports three separate interaction
  issues. Close 6D2; address these without starting crop-corner snapping.
- TDD red: reopening text starts the caret at the beginning regardless of
  the click; tiny selected objects still intercept presses as resize hits;
  idle Escape leaves the last annotation selected. Green: reuse iced's
  shaped-text editor hit testing, share handle eligibility between rendering
  and picking, and clear idle selection outside content/history.
- Position the first-click caret using the annotation's inverse rotation,
  selected font/style, actual zoom, and explicit newlines. Keep the editor
  horizontal and preserve its existing unrotated sizing. Let iced handle
  Unicode caret boundaries rather than estimating character widths.
- Keep small-object frames visible and movable. Hide/inactivate corner,
  endpoint, and shape rotation controls below a conservative screen-space
  threshold leaving at least half each edge/shaft clear. Share handle size
  and stroke constants with drawing. Account for diagonal shafts and
  rotated, axis-aligned handle squares so visible controls never overlap.
  Zoom restores handles; even Alt cannot activate a hidden resize target.
  Existing resize gestures can still finish after shrinking below cutoff.
- Include the thin visible frame stroke in small-object move hits, avoiding
  accidental new objects from screen/page rounding at a selected corner.
  Retain normal mouse/touch stroke-picking tolerance. Test small moves and
  zoom-restored resizing through the application, not only core hit queries.
- Escape clears selection only when no text editor, annotation gesture, or
  page reorder is active. It keeps the current tool/settings, content, dirty
  state, and history. During an active edit, the first Escape still reverts
  it and retains selection; a second Escape clears it. Focus loss/window
  resize and confirmation-dialog Escape keep their prior behavior.
- Cover mouse/touch targets, all shape kinds, invalid scales, zoom,
  rotated/narrow-frame non-overlap, inactive Alt targets, Unicode/formatted
  caret placement, undo/redo, and pending text cancellation. Software-render
  tiny check/cross/arrow selections at 1x/2x DPI to verify that only their
  frames remain. Update old resize/rotation fixtures to zoom in where their
  narrow objects now intentionally hide controls.
- Validation: 410 all-feature/all-target tests and 225 core/example tests
  pass. Strict native/WASM all-target Clippy, formatting, whitespace, and
  Trunk browser-build checks pass. Keep all build output under /tmp and
  preserve the host target/dist directories. Physical touch remains untested.

Manual checkpoint — accepted 2026-10-16:

1. With Text, click existing text near its middle and on different lines.
   The initial caret should be near the clicked glyph. Repeat after zooming
   and rotating; verify typing and Escape still behave normally.
2. Select small checks/crosses and arrows, then zoom out. Only the thin
   frame should remain; dragging inside it moves rather than resizes, even
   with Alt. Zoom in to restore non-overlapping controls and verify resizing.
3. Place an annotation, press Escape, and change its style. The last object
   should stay unchanged; the next annotation should use the new style.
   While editing/moving, first Escape should revert, second should deselect.

Leave changes uncommitted and stop before 6D3 for user acceptance.

### 2026-10-16 — reuse text already placed on a snapped rule

- User accepts the caret, small-handle, and Escape follow-ups. A new request:
  clicking a filled rule should reopen its existing text with the caret at
  the end instead of creating another annotation over it.
- TDD red: clicking past a short field's text creates a new empty draft ID.
  Reuse the topmost text whose left/bottom anchor matches the detected rule
  within one page point, allowing normalized-coordinate rounding. Derive
  this from ordinary geometry; add no persisted field links or schema data.
  Text overflowing from a neighboring cell is not an anchor match.
- Keep direct text clicks ahead of snap lookup, preserving glyph-based caret
  positioning. Reopening through a rule moves the caret to DocumentEnd,
  including multiline/Unicode content, without moving or restyling the text.
  Opening/canceling creates no content/history change; accepted typing edits
  the existing ID in one undo step. Off/pending/unmatched snap stays ordinary.
- Remove the obsolete editor reset on pointer release: annotation finish
  cannot open a text draft. A stray release must not reset the chosen caret
  or replace content while an editor is already active.
- Cover mouse/touch at three zoom levels, direct caret picks, missing/pending
  features, late results, format/geometry/ID preservation, Escape, undo/redo,
  overlapping neighboring fields, layer order, invalid queries, and drafts.
- Validation: 414 all-feature/all-target tests and 227 core/example tests
  pass. Strict native/WASM all-target Clippy, formatting, and whitespace
  checks pass. Keep build output isolated under /tmp; no commit requested.

Manual checkpoint — accepted 2026-10-16:

1. Fill a short text value on a long rule. With Text and Smart snap on,
   click an empty part of that same rule: reopen the original text at its
   end. Typing should append; Escape should discard that edit, not the field.
2. Click existing text directly: caret should still follow the clicked glyph.
   Check another field/table cell and snap off: do not reuse the first field.

User confirms the filled-rule reuse check and requests an unsigned commit
of the accepted 6D1/6D2 work and follow-ups, then continuation with 6D3.

### 2026-10-16 — accepted snapping committed; crop-corner checkpoint 6D3

- Commit accepted 6D1/6D2 and their interaction/compatibility follow-ups as
  `1c8da37`, unsigned, without pushing. Keep the C# application unchanged.
- Add portable Shi–Tomasi corner hints using grayscale source pixels and
  the existing 1600-pixel analysis bound. Retain 300 corners, quality 0.02,
  8-pixel spacing, and a 5-pixel structure-tensor window. Reuse image/alpha
  handling; add no dependency. Portable borders/resampling need not match
  OpenCV pixelwise. These are local hints, not document auto-framing.
- Analyze once while the existing preview worker decodes the original;
  reuse the same source/hints across crop, rotation, and filter adjustments.
  Existing session guards reject stale hints along with stale previews.
  A hint-analysis error leaves manual corrections available. The browser
  retains the existing UI-thread fallback rather than adding a worker stack.
- Keep raw pointer positions separate from snapped targets. Ctrl press or
  release changes the target immediately, without moving the pointer. Cache
  modifier events locally too: another pointer event can arrive before the
  root rebuilds the view. Reject snapped quads that cross or collapse, using
  the existing convexity validator; keep ordinary manual placement instead.
- Use the original image's fitted/letterboxed dimensions for the calibrated
  52-screen-pixel radius, clamped to 0.015–0.09 of the short image axis.
  Lost touch or unavailable release positions restore the exact unsnapped
  starting corner. Keep the annotation Smart snap toggle independent.
- TDD: first reproduce missing modifier/touch snapping, then the root-view
  modifier lag. Cover rectangular/skewed targets, straight-edge rejection,
  resize normalization, feature spacing/caps, real forms, transparency, bad
  queries, letterboxing, pointer ownership, lost touch, invalid snaps, hint
  reuse, stale jobs, and transactional Accept/Cancel/Undo/Redo.
- Validation: 425 all-feature/all-target tests and 232 core/example tests
  pass. Strict native/WASM all-target Clippy, formatting, whitespace, and
  Trunk browser-build checks pass. Keep target/dist output isolated under
  /tmp. Physical touch remains untested; browser validation is build-only.

Manual checkpoint 6D3 — accepted 2026-10-16:

1. Open an image page, choose Edit page, and drag a corner near a visible
   document/printed-box corner. Without Ctrl, movement stays free. Hold Ctrl
   to snap; release Ctrl without moving the mouse to restore the raw point.
   Press Ctrl again, release the mouse, and verify the snapped point stays.
2. Change rotation, filters, brightness, and contrast. Snapping on the left
   continues to follow the original image, not the corrected preview. Try a
   different window shape to check letterboxing. No nearby corner means
   ordinary dragging; the annotation Smart snap button is unrelated.
3. Cancel/Escape leaves the page unchanged. Accept commits the crop as one
   Undo/Redo step. If touch is available, verify free corner dragging and
   cancellation still work; no physical touch check has run in the container.

User confirms behavior and requests continuation with 6E. Keep accepted 6D3
changes uncommitted; no new commit was requested.

### 2026-10-16 — 6D3 accepted; offline framing checkpoint 6E1

- User confirms crop-corner snapping. Close 6D3 and proceed without a new
  commit. Split 6E into offline detector inspection and difficult-frame/GUI
  integration; keep the C# application and all capture/temporal code intact.
- Add a portable still-image detector and `frame_probe INPUT OUTPUT.png`.
  Return normalized accepted/rejected corners separately, confidence, area,
  sharpness, and analysis dimensions. Emit green accepted/amber rejected
  overlays and JSON; keep inputs intact and reject existing output files.
  Apply existing 100 MiB/50 MP trust limits before processing/allocation.
- Keep the 1280-pixel analysis edge; grayscale 5-tap binomial smoothing;
  Canny 60/180 and 35/120 with the original repeated rectangular closing;
  HSV paper V >= 115/S <= 105, elliptical closing/opening; hull perimeter
  tolerances 0.01/0.015/0.02/0.03/0.04; convexity, minimum area 0.05,
  aspect <= 5, 40% area saturation, and 2% border/0.12-corner penalties.
- Add line-pair recovery in this offline slice after real frames prove
  connected outlines insufficient. Retain 1 px/1 degree progressive Hough,
  votes 70/50, lengths 18%/12%, gaps 60/80, 14 distinct lines per orientation,
  3 degree/1.5% line deduplication, pair spacing, and intersection margins.
  Seed sampling deterministically and consume traced pixels; a full global
  Hough peak approximation invented rays and failed the clutter fixture.
- Retain 24 samples per side, 1.5% boundary offsets, paper/Lab contrast,
  9-pixel Lab smoothing, and paired continuing-side penalties. Reuse the
  existing tested Lab conversion in a private shared image-color module;
  do not duplicate it or expose correction internals to the detector.
- TDD: observe missing detector imports, then recorded-frame misses, then
  a cluttered synthetic document's displaced bottom. Fix implementation
  differences without weakening the 0.72 acceptance threshold or existing
  synthetic 0.04/0.02 and recording 0.1 normalized corner tolerances.
  Cover blanks, transparent hidden RGB, tiny images, scale, input preservation,
  saturated documents, internal form rules, candidate JSON, and command I/O.
- Test frames 0059/0112/0168 directly from the trusted committed laptop-camera
  TAR, without an archive dependency/extraction. This is still-image coverage,
  not the full 224-frame temporal/countdown checkpoint in 6F.
- Generate 14 local fixture overlays: the original card screenshot, seven
  HOYA still/sequence images, three recorded sheets, and three booklets.
  Card/HOYA/sheet frames are accepted; inspected representative outlines
  follow their boundaries. Booklet 1/2/3 remain rejected at approximately
  0.657/0.712/0.640, with
  incorrect candidate outlines. Do not treat those polygons as valid crops
  merely because manual review can use a lower confidence threshold.
- Explicit parity gaps: component hulls/enclosed edge regions are present,
  but original concave-contour candidates are not. Portable resampling,
  border/rounding, Lab smoothing, and segment enumeration may differ from
  OpenCV. Investigate these for the booklet misses in 6E2; no blanket
  threshold reduction. No previous-frame hints/stabilization are added yet.
- Validation: 437 all-feature/all-target tests and 244 core/example tests
  pass. Strict native/WASM all-target Clippy, formatting, whitespace, and
  Trunk browser-build checks pass. Keep compilation/distribution under /tmp;
  only generated review previews go under the ignored artifacts directory.
  Browser validation is build-only; no new physical-camera claim is made.

Manual checkpoint 6E1 — accepted 2026-10-16:

1. Open `artifacts/previews/framing-6e1.Z0H3zH/index.html` in a browser.
   Inspect the green card/HOYA/sheet outlines; each image links to full size
   and the untouched original. Report any corner that misses the boundary.
2. The three amber booklet outlines are known misses, deliberately rejected
   by automatic detection. Following gallery acceptance, retain them as
   documented limits rather than block manual Auto frame integration.
3. Optionally run `frame_probe` on another photographed PNG/JPEG. Existing
   output files must be refused. The desktop editor intentionally has no new
   button yet; current manual correction and Ctrl snapping remain available.

User accepts the gallery, including minor errors and known misses against
misleading high-contrast backgrounds, and requests continuation with 6E2.
Proceed with Auto frame integration; retain detector limits and defer
further parity tuning. Keep changes uncommitted; no commit requested.

### 2026-10-16 — 6E1 accepted; Auto frame checkpoint 6E2

- User accepts the gallery's minor errors/known misses against misleading,
  high-contrast backgrounds. Retain the detector as accepted; defer further
  contour/pixel parity work rather than recalibrate thresholds or fixtures.
- Add explicit Auto frame beside Full frame in Edit page. Opening the
  editor stays manual and never changes an existing crop automatically.
  Reuse the existing preview worker and decoded original, with a lazy
  standard-library OnceLock for analysis results per editor session.
  No second worker, native dependency, or document-format change.
- Apply a detected quad only to the draft and corrected preview. Retain the
  C# 0.45 manual-review cutoff for candidates below automatic acceptance;
  label those Low-confidence. Render notices in a visible warning panel.
  All results still require checking corners and explicit Accept. Missing,
  unusable, or failed detection keeps the current crop and manual workflow.
- Reuse analysis across repeated clicks, crops, filters, tone, and rotation;
  always analyze original pixels, not the corrected preview. Preserve all
  non-crop settings and original encoded assets. Browser analysis retains
  the existing UI-thread fallback; responsiveness is not claimed here.
- Disable duplicate requests during preview/analysis work. Accept cannot
  commit a cached old preview during framing; Cancel/Escape stays available.
  Invalidate active crop gestures when Auto frame starts. Old sessions and
  late worker completions cannot reframe another editor or page.
- Track manual edit revisions as well as settings. Any intervening valid
  manual change wins, including an unchanged Full frame or settings changed
  and restored. Discard the late crop, finish the latest manual preview, and
  report that manual changes were kept. A later Auto frame reuses the cache.
  Worker failures are nonfatal notices, not blockers for manual acceptance.
- TDD: first reproduce missing request/cache APIs. Add ten regressions for
  explicit requests, pending Accept, duplicate jobs, original-space cache
  reuse, 0.45 cutoff, invalid/weak/failed results, worker failure, manual/ABA
  races, stale sessions, Cancel, one-step history, text/shape preservation,
  original asset bytes, Save/reload, and corrected-dimension PDF export.
- Validation: 447 all-feature/all-target tests and 244 core/example tests
  pass. Strict native/WASM all-target Clippy, formatting, and whitespace
  checks and the Trunk browser build pass. Compilation remains isolated
  under /tmp; no C# source or CI changes. Inspect the software-rendered panel at
  `artifacts/previews/auto-frame-6e2.sMqDXi/panel.png`; physical touch and
  browser interaction are untested in this container.

Manual checkpoint 6E2 — accepted 2026-10-16:

1. Import a photographed document or open an existing image project. Enter
   Edit page: the saved crop must stay unchanged. Press Auto frame and check
   all four original-image corners plus the corrected preview. Full frame
   restores the source; Ctrl-assisted dragging remains available.
2. Try rotation, color mode, brightness, and contrast before/after framing.
   They stay as selected. A booklet photo may yield a Low-confidence warning
   and an imperfect suggestion at the intentional lower review threshold.
   Adjust it manually. If changing settings while analysis runs, those edits
   win; press Auto frame again after completion to reapply its suggestion.
3. Cancel/Escape leaves the original page/annotations unchanged. Reopen,
   Auto frame, Accept, then Undo/Redo: corrections and image-anchored objects
   change together in one step. Optionally Save/reopen or export the PDF.

Leave changes uncommitted and stop before recorded temporal replay in 6F.

### 2026-10-16 — 6E2 accepted; bounded 11-19 framing investigation

- User confirms the editor checkpoint, but reports incorrect right corners
  in `artifacts/reviews/docscans/Scan 2026-09-15 11-19.docscan`.
  Compare the original 3264 x 1836 image with its saved manual crop; never
  modify the project, asset, or saved crop. Generate a diagnostic overlay at
  `artifacts/previews/framing-11-19.JIXI7I/before.png`.
- Candidate generation finds the card: its closest quad is within 4.3 px
  of every saved manual corner at the 1280 x 720 analysis resolution.
  Selection is the failure: that quad scores 0.786, while a larger quad
  joining the card's left edge to speaker/cable edges scores 0.854.
  Confidence is a heuristic, not a correctness guarantee.
- Try a bounded scoring change: require nearby Canny-edge support for
  sampled paper/Lab boundary contrast, reusing the existing distance map
  and 0.6% tolerance. It still chooses the wrong 11-19 outline and breaks
  the existing recorded-frame 0168 corner tolerance. Remove the experiment
  and temporary instrumentation; leave detector behavior, thresholds, and
  tests unchanged. Do not weaken the regression or tune to this one image.
- Also test the user's rectangularity suggestion. This already contributes
  0.33 of the existing score through area/minimum-enclosing-rectangle area.
  Add a stronger 0.75 penalty for missing rectangularity as an experiment:
  it rejects the speaker edge, but picks an incorrect, shorter crop that
  cuts off about 13% of the card's height. Frame 0168 regresses too.
  Keep the comparison overlay at
  `artifacts/previews/framing-11-19.JIXI7I/stronger-rectangularity.png`.
  Remove the extra penalty; stronger shape bias is not a safe small fix
  and must not exclude genuine perspective-skewed documents.
- Accept this as a known background-edge ranking limitation, as requested.
  Manual corner adjustment and Ctrl snapping remain the workaround. A
  broader scoring/candidate investigation is separate future work, not a
  prerequisite for accepting 6E2. No new manual checkpoint or commit.
- Validation after restoring the detector: all eight still-image framing
  regressions pass with their original tolerances. Formatting and whitespace
  checks pass. Retained changes for this investigation are documentation only.

### 2026-10-16 — accepted work committed; 6F1 temporal/replay checkpoint

- Commit accepted 6D3–6E2 work unsigned as `7c3177d`, without pushing.
  The owner's signing rebase changes it to `54bed57` with an identical tree.
  Keep the C# app unchanged. No commit is requested for the new 6F work.
- TDD: first observe missing temporal APIs. Port bounded four-result history,
  two-vote clusters, current-cluster tie preference, coordinate medians,
  0.08 agreement, and 0.35 smoothing. Retain current-frame sharpness rather
  than smooth away blur. Validate finite metrics and convex median results.
- Add optional previous-frame ranking hints using the original 0.12 radius
  and bonus. Invalid hints are ignored. Keep confidence separate from rank:
  hints cannot invent corners or turn rejected candidates into detections.
  The unhinted page editor and detector calibration stay unchanged.
- Port the current C# two-second countdown, confidence >= 0.72, area >= 0.1,
  sharpness >= 25, and cumulative drift <= 0.04 from the countdown anchor.
  Use monotonic Duration timestamps, handle clock rollback, expose progress,
  and emit Ready once until loss/blur or an explicit reset rearms the gate.
  Test jitter, majority changes, smoothing, invalid/rejected input, blur,
  loss, resets, repeated timestamps, rollback, and capture rearming.
- Add `frame_replay INPUT_DIR NEW_OUTPUT_DIR [INTERVAL_MS=33]`, bounded to
  300 sorted regular JPEG/PNG frames and existing 100 MiB/50 MP image limits.
  Stream one source at a time, copy original bytes, and emit JSON plus a
  server-free HTML viewer with playback, scrubbing, optional outlines,
  countdown, capture jump, and per-frame metadata. Reset for size changes.
  Refuse existing outputs and escape embedded JSON against HTML injection.
- Extend the recorded regression to all 224 trusted committed laptop frames
  with virtual 33 ms timestamps. Retain all C# gates: raw correct >= 160,
  stable correct >= 180, wrong = 0, and at least one correctly framed Ready.
  Preserve the separate three-frame unhinted smoke test and its tolerance.
- The new full gate fails: raw 143 correct/34 wrong/47 missing; stable
  151 correct/33 wrong/40 missing; no capture, peak progress about 0.957.
  Keep this test enabled and visibly red. Do not skip it, lower thresholds,
  loosen tolerances, or claim 6F complete merely to keep CI green.
- Investigate the earlier deferred contour gap and an OpenCV diagonal NMS
  tie mismatch. The combined experiment reaches raw 181 correct/0 wrong
  and stable 190 correct/0 wrong, but still never captures and regresses the
  accepted card screenshot: its status bar becomes the proposed bottom.
  Remove both detector experiments; retain no pixel/candidate changes.
  This is broader detector-parity work, not a safe calibration-only tweak.
  Compare the [upstream diagonal NMS rule][canny-nms-reference] when resuming.
- Split 6F at this checkpoint: portable temporal logic and diagnostics are
  ready to inspect; full-recording parity remains explicitly pending in
  6F2. Do not start camera adapters in 6G or mark the full milestone complete.
- Validation: native all-feature/all-target and core/example checks pass
  when explicitly excluding the new known-red full-recording gate with
  `-- --skip recorded_laptop_sequence --quiet`; it is not ignored in source
  or disabled in CI. Strict native/WASM Clippy, formatting, whitespace,
  and the final browser build pass. The eight temporal unit checks and
  four replay-command checks pass. Rebuild/replay the actual corpus after
  removing experiments; verify the four-frame hold demo reaches Ready.
  All compilation/browser output stays under /tmp, not host target/dist.

Initial manual checkpoint 6F1 — superseded by the one-second replay below:

1. Open `artifacts/previews/temporal-6f.eCb9ro/replay/index.html`. Play,
   scrub, step, and toggle raw/stable/prior-hint outlines. The actual laptop
   sequence is a diagnostic failure: it does not reach a capture decision.
2. Open `artifacts/previews/temporal-6f.eCb9ro/hold-demo/index.html`.
   This deliberately repeats one real frame four times at 1000 ms intervals;
   it is not footage or a detector-success claim. Check 0%, 50%, and 100%
   progress, Jump to capture, and playback-speed-independent decisions.
3. Inspect raw/stable differences and metadata. Browser interaction needs
   host validation; no browser executable is installed in this container.
   The viewer's controls, stale-image guard, and progress logic were checked
   using V8 with DOM/image stubs, not a real browser or touchscreen.

[canny-nms-reference]:
  https://github.com/opencv/opencv/blob/4.11.0/modules/imgproc/src/canny.cpp#L585

### 2026-10-16 — 6F follow-up: one-second wait and real camera fixtures

- User reports the framing is usable through much of the recording, but
  two seconds is impractical for handheld steadiness. Shorten only the wait
  to one second; keep confidence, area, sharpness, voting, smoothing, motion
  limits, and corner tolerances unchanged.
- User clarifies that the card/status-bar image is a screenshot of a
  viewfinder, not raw camera pixels. Retain it as diagnostic-only rather
  than require the detector to ignore artificial app chrome. Restore the
  connected-contour candidates and strict diagonal Canny maxima. Genuine
  camera fixtures and the synthetic background-extension check still gate
  changes. The separate 11-19 background-ranking limit remains documented.
- TDD: update timing checks first and observe three failures against the
  two-second gate. Verify no capture at 999 ms, capture at 1000 ms, 50%
  progress at 500 ms, cumulative drift/blur resets, clock rollback, and one
  event per cycle. Update the replay's timing metadata and event checks.
- The container interruption stops the corpus run and clears /tmp build
  output. Preserve the partial report as `interrupted-replay`, rebuild in
  the isolated target, and rerun all core/example tests and the full replay.
  Do not alter host target/dist, C# source, or CI. Keep changes uncommitted.
- Full-recording acceptance now passes: raw 181 correct / 0 wrong / 43
  missing; stable 190 correct / 0 wrong / 34 missing. The first correctly
  framed capture decision occurs at frame 83 (2706 ms), after 1023 ms of
  steady input at the recording's 33 ms interval. Missing early/blurred
  frames remain missing; do not claim detection on every frame.
- Validation: all 259 core/example tests pass, including the full enabled
  224-frame gate without skips. Strict native and WASM core Clippy,
  formatting, and whitespace checks pass after the rebuild. Before the
  interruption, unchanged source also passed native all-feature checks
  (with the duplicate corpus run excluded), strict native/WASM all-feature
  Clippy, and a Trunk browser build. Physical camera access is still untested.
- Regenerate the complete offline replay and verify its JSON matches the
  embedded report, all 224 image files exist, and countdown/capture metadata
  matches the regression. Retain the small 500 ms repeated-still demo.

Manual checkpoint 6F — accepted 2026-10-16:

1. Open `artifacts/previews/temporal-6f-1s.N3mPss/index.html`, then the
   updated 224-frame replay. Play, scrub, toggle raw/stable/prior-hint
   outlines, and use Jump to capture. Verify the green outline follows the
   paper at frame 83 rather than a fold or background edge.
2. Inspect countdown progress immediately before that capture. Its one-second
   wait uses virtual recording timestamps, not playback/analysis speed.
   The separate repeated-still demo shows consensus, 0%, 50%, and 100% at
   500 ms frame intervals; it is not footage or camera-adapter validation.
3. The earlier two-second report remains available for comparison, clearly
   marked historical. Keep screenshot/status-bar effects diagnostic-only.
   Offline replay acceptance does not validate native camera adapters.

User accepts the replay checkpoint and requests a commit and stop for today.
Mark 6F1/6F2 complete, create an unsigned commit, and do not push. Leave
native capture in 6G unstarted until the user requests continuation.

### 2026-10-19 — keep exhaustive camera replay out of routine tests

- User reports the 224-frame regression takes 191.90 seconds locally and
  Rust CI jobs exceed 15 minutes. The native matrix runs the corpus twice
  per platform, once with core features and again with all features.
- Mark only the exhaustive recording test with standard Rust `#[ignore]`.
  Keep its 224-frame, correctness, quality, and capture assertions intact;
  run it only when explicitly requested with `-- --ignored --nocapture`.
  Existing Cargo/CI commands respect this without workflow changes.
- Keep three independent real-camera smoke checks, temporal timing/loss/
  reset checks, and the small replay-command tests in the routine suite.
  Do not change production detection, countdown, fixtures, or calibration.
- Document the opt-in command in the Rust README. Leave 6G unstarted.
- Validation: 258 core/example tests pass; only the full replay is ignored.
  The detector suite takes 5.43 seconds, excluding compilation, with nine
  passing tests and one ignored test. Verify the opt-in command with
  `--ignored --list`: it selects exactly that recording test without
  rerunning the unchanged corpus. Formatting, core Clippy, and whitespace
  checks pass. Keep changes uncommitted; no commit requested.

### 2026-10-19 — simplify annotation state and editing

- Replace parallel shape/text stores and layer bookkeeping with ordered
  enum content and UUID selection; simplify snapshots and mixed operations.
- Share geometric drafts, previews, commits and style updates. Centralize
  tool capabilities while keeping text layout and concrete geometry separate.
- Preserve project schemas, legacy metadata, identity, alpha, off-page
  geometry, defaults, pending-text acceptance/cancellation and history.
- Fix dragging a single selected object behind unselected layers. Preserve
  Ctrl toggles/marquees; cover all tools, zoom, mouse/touch and undo/redo.
- Add schema-oracle and editing contracts plus a dependency-free source-size
  check: `node tools/source-size.mjs`. Remove 220 production source lines
  without new production files, dependencies or generic frameworks.
- Validation: 473 native and 270 core tests pass; the full replay stays
  opt-in. Strict Clippy, WASM, formatting and whitespace checks pass.
  User accepts manual refactor checks and the overlap-drag follow-up.
- Keep native camera/exposure work stashed separately; C# remains unchanged.

### 2026-10-19 — test change committed; native camera probe checkpoint 6G1

- Commit the accepted slow-test change unsigned as `a4bd8bb`, without
  pushing. Start only the backend spike, not a GUI capture workflow.
- Evaluate Nokhwa's native adapters. Its Linux V4L wrapper generates
  bindings with libclang, absent from this container. Use rscam's direct
  V4L2 mode on Linux instead; it needs only existing libc. Use Nokhwa's
  Media Foundation adapter on Windows. Do not install missing tools or
  add OpenCV/GStreamer/libv4l2/mozjpeg runtime requirements.
- Keep the spike in an opt-in example and optional `native-camera` feature.
  Reuse Nokhwa's packed-pixel conversions and the existing bounded JPEG
  decoder. No document/editor/correction/CI source changes. macOS returns
  Unsupported for now; WASM excludes all native camera dependencies.
- Add list, formats, and one-frame PNG capture with explicit device/mode
  selection. Default to 640x480@30 MJPG, without silent mode fallback.
  Keep 50 MP/100 MiB bounds and reject malformed, padded, or changed frames.
  Discard three exposure-startup frames, stop streams on read/decode errors,
  and refuse existing output files with atomic no-overwrite PNG output.
- Isolate driver calls in a child process with a 20-second timeout. Kill
  and reap stalled workers, surface failures, and keep all camera access
  out of automated tests. This diagnostic does not establish GUI threading,
  streaming latency, full-resolution still capture, orientation, or lifecycle.
- TDD: first observe missing command/spec/decoder/output APIs. Eight new
  tests cover empty/malformed arguments, explicit modes and Unicode paths,
  RGB/BGR/gray/YUYV/NV12/JPEG decoding, bounds/stride/mismatch rejection,
  output preservation, worker success/failure, and timeout cleanup.
- Validation: 266 core/example tests pass with `native-camera`, with only
  the full 224-frame replay ignored. Strict camera-enabled core Clippy,
  formatting, whitespace, and all-feature/all-target WASM compilation pass.
  Inspect the WASM dependency tree: no camera crates. Native Windows/macOS
  builds await existing CI; no cross toolchains are installed here.
- Smoke-check help, enumeration, empty arguments, existing destinations,
  and a guaranteed missing device. Enumeration finds the two Linux video
  nodes without opening a stream. Do not capture from the user's camera
  in the container; physical image/color/orientation/release checks await
  the user. Keep new probe changes uncommitted and stop before 6G2.

Manual checkpoint 6G1 — pending:

1. From `rust/`, follow the camera-probe commands in README: list devices,
   inspect formats for the chosen ID, and capture a photographed document
   into a new PNG. Start on the Linux laptop; use a listed exact mode if
   the default is unavailable. Report device ID and selected format.
2. Check dimensions, orientation, aspect, and colors. Capture again to a
   different path, then open another camera app to confirm device release.
   Retry an existing destination: it must fail without replacing the file.
3. Keep no-device/permission/busy/unsupported-mode errors visible. No live
   preview/framing/countdown/project insertion is included yet. Confirm
   this adapter checkpoint before GUI integration in 6G2; repeat on SP6
   later, since laptop validation does not establish Surface compatibility.

### 2026-10-19 — camera probe exposure warm-up

- User confirms the captured image looks about right, but is dark because
  the camera has insufficient time to adjust. Three discarded frames give
  only about 100 ms at 30 FPS, not a useful exposure-settling interval.
- Drain frames for at least two seconds after receiving the first frame
  from either native stream, retaining the three-buffer minimum, then
  read a fresh frame. Do not count slow device opening/first-read latency
  as exposure-settling time; reproduce that case with a delayed fake frame.
  Release every discarded buffer immediately; do not sleep over queued
  stale images. Keep the 20-second worker timeout and existing cleanup.
  This is deliberately a timed allowance, not exposure-stability analysis.
- TDD: first reproduce missing warm-up APIs. Add virtual-clock checks at
  5/30/120 FPS, immediate buffer release, slow initial reads, and errors
  during startup, warm-up, and final capture. No real waits or camera
  access in these new checks. Keep 6G1 acceptance pending a brightness retry.
- Validation: routine camera-enabled core/example tests pass (269 tests;
  only the exhaustive replay ignored). All 11 probe checks, rerun after
  first-frame timing calibration, pass in 0.1 seconds. Strict core Clippy,
  formatting, whitespace checks, and the native probe build pass. Preserve
  the user's sample image and unrelated untracked simplification notes.
  Leave changes uncommitted; do not start 6G2 or claim hardware acceptance.

### 2026-10-19 — restore camera work; checkpoint 6G2 manual capture

- User requests removing obsolete text/duplicate camera stashes and popping
  the remaining camera work. Drop `ac46a5a` and `c85ce43`, restore `2684df6`,
  preserve both sides of its progress-log conflict and drop the restored
  stash. Keep the annotation refactor and C# application unchanged.
- Move the probe's native adapter, decoding, exposure warm-up and checks
  into the camera library module. The diagnostic example remains available.
  Reuse that implementation for live preview; add no further dependencies.
- Add a Camera screen, initially using device 0 and the explicit probe mode.
  Warm exposure before exposing capture; freeze the actual displayed frame,
  allow Retake and add the photo only after Use photo. Insert after the
  active page without replacing its annotations, then open page corrections.
- Isolate streaming in the current executable's hidden helper mode. Bound
  dimensions, payloads, errors and GUI frame delivery; discard surplus
  previews rather than growing a backlog. A watchdog terminates stalled
  workers; process guards reap them outside UI updates. Signal termination
  immediately on draft disposal; never hold the UI-shared child lock while
  waiting for a driver process to exit.
- Keep preview/PNG work away from document updates. Cancel, Escape, focus
  loss and close release the stream; stale frames, queued starts and photo
  preparation completions cannot revive canceled sessions. Resume explicitly
  after focus loss; retain normal unsaved-project confirmation on close.
- TDD: first reproduce missing camera protocol/module APIs. Add protocol
  round trips, malformed/oversized/truncated packets and Unicode errors,
  timeout/cancellation/reaping checks using fake child processes and modal
  state tests for capture, preservation, stale events, errors and page limits.
  No automated test opens a real camera. Keep the long replay opt-in.
- Validation: all 494 native/all-target tests and 270 dependency-light
  core tests pass; the full replay remains ignored. Rerun all five camera
  screen checks after tightening delivery to one in-flight frame across
  iced's internal event queue. Strict native/camera-core Clippy, formatting,
  whitespace and all-feature WASM checks pass. Confirm camera dependencies
  are absent from the WASM tree and the probe's help still works.
- Manual checkpoint 6G2 is pending. Use the GUI to retry exposure brightness
  and verify live responsiveness, orientation/color/aspect, retake/cancel,
  camera release, page correction and project save/reopen. Keep native
  Windows/SP6 validation explicit. Do not start 6G3 before this check.
- Keep all restored/new work uncommitted; no new commit was requested.

### 2026-10-19 — immediate camera preview; preserve it across window moves

- User reports the basic GUI capture flow works. Holding back live frames
  during warm-up looks like a broken camera, and moving the app can trigger
  focus loss, terminate capture and display a misleading red warning.
- Show the first live frame immediately, even while dark. Keep only Capture
  disabled for two seconds after the first displayed frame; use neutral
  exposure-adjustment text. Preserve the standalone probe's timed warm-up.
- Keep the live stream, last frame, session ID and exposure timer on focus
  loss or window move/resize. Clear input modifiers only. Explicit Cancel,
  Escape, Capture, errors and app closure still stop the worker.
- TDD: reproduce the focus-loss session reset and missing immediate-preview
  read path. Cover a single immediate read without clock access, preserved
  one-shot warm-up, visible startup frames, early-capture rejection, and
  capture readiness across subsequent frames/focus changes without sleeps.
- Validation: all 15 camera-core and seven camera-screen checks pass;
  strict all-feature/all-target Clippy, WASM, formatting and whitespace
  checks pass. Keep hardware access out of automated tests.
- User follow-up: confirm immediate preview and moving/defocusing the window
  no longer pauses it. Keep this change uncommitted; do not start 6G3 yet.

### 2026-10-19 — remove manual capture gate; clarify correction cancellation

- User confirms immediate preview and uninterrupted window moves. Remove
  the remaining two-second manual-capture gate and its GUI timer; enable
  Capture as soon as a frame is available. Keep the standalone one-shot
  diagnostic's warm-up, since it has no live preview to judge exposure.
- Preserve the accepted Use photo transaction: it adds the page before
  opening corrections. Cancel in the page editor discards only correction
  edits; Delete page removes the photo. Explain this in the camera review
  screen and README instead of changing cancellation into silent deletion.
- TDD: reproduce the disabled first-frame Capture button. Check capture
  before any frame stays disabled, the first frame permits capture without
  clocks/sleeps, and canceling subsequent corrections retains the photo,
  existing annotations and original correction settings.
- Validation: all 196 application checks pass, including the first-frame
  and correction-cancellation regressions. Strict GUI/camera-core Clippy,
  all-feature WASM, formatting and whitespace checks pass.
- Keep changes uncommitted; do not start 6G3 in this follow-up.

### 2026-10-19 — accept 6G1/6G2; connect live auto-framing in 6G3a

- User marks both native-camera checkpoints manually checked. Record their
  acceptance and split 6G3 into live framing, device/mode controls and
  hardware lifecycle checks. Keep Linux laptop acceptance separate from SP6.
- Reuse hinted document detection, four-result consensus and the calibrated
  one-second gate. Run one background analysis across camera sessions while
  preview continues; coalesce incoming frames without a CPU-work backlog.
- Overlay a translucent green quadrangle on the contained image, including
  letterboxing, and draw circular progress around a touch-sized Capture.
  Keep manual capture immediately available. Auto-capture freezes the exact
  analyzed pixels and matching crop rather than a newer preview image.
- Reject stale analysis and reset after long gaps, changed dimensions or
  input errors. At the final capture, require the current raw quad to agree
  with consensus at the existing 0.04 gate tolerance. Historical votes alone
  cannot capture a missing or suddenly moved document. Do not retune the
  detector, consensus, sharpness threshold or recorded-sequence fixtures.
- Carry automatic corners into pending page-editor settings. Use photo
  still inserts an untouched source image; Accept applies the crop, Full
  frame removes it and Cancel keeps the uncorrected photo. Manual capture
  retains its displayed frame and can use the editor's Auto frame button.
- Keep camera I/O/PNG preparation, framing and preview widgets in focused
  modules. Expose the existing ordered crop-point accessor for the GUI;
  add no dependencies or generic task framework. Leave the C# app untouched.
- Tests cover virtual countdown/quality/gap changes, bounded work across
  cancel/reopen, detector failures with manual capture, exact source-frame
  freezing and crop handoff. Render portrait/landscape overlays at 100/200%
  DPI, verify partial/full countdown drawing and pending-crop Cancel/Accept
  with undo. No real camera opens and the full replay remains opt-in.
- Validation: 505 native/all-target checks and 270 dependency-light checks
  pass, with only the full recording ignored. All 14 camera checks pass,
  including four portrait/landscape and DPI render cases. Strict Clippy,
  all-feature WASM, formatting and whitespace checks pass. Builds stay in
  isolated container directories; host target/dist directories are untouched.

Manual checkpoint 6G3a — pending:

1. Run the native app, choose Camera and point at a printed form against a
   contrasting background. Confirm immediate preview, a green quadrangle
   around the paper and a ring filling during the one-second steady hold.
2. Move/blur/remove the document mid-countdown. It must reset or reacquire,
   not capture an old frame. Hold steady again and check the frozen image
   matches its corners; no further automatic shot occurs until Retake.
3. Use photo: inspect the suggested crop in page corrections. Adjust it,
   choose Full frame or Accept. Cancel must keep the full, uncorrected photo.
   Save/reopen an accepted cropped page and verify its source is retained.
4. Retake, press manual Capture immediately, and check it freezes the visible
   image without waiting. Camera Cancel/Escape must add no page. Move the
   window or switch focus while previewing; neither should require restart.
5. Check responsiveness and camera release. This slice still uses the
   accepted 640×480 MJPG mode; defer device selection/high-resolution stills
   to 6G3b and real Surface orientation/suspend/resume checks to 6G3c.

Stop here for the user check. Keep restored/new changes uncommitted; no
commit or push was requested.

### 2026-10-19 — remove averaging lag from the captured crop

- User reports the 6G3a flow works, but slow movement makes the averaged
  frame trail the document. Keep preview smoothing and countdown unchanged;
  use the exact frozen image's raw corners for the final crop, after the
  existing agreement check. No additional detection pass or threshold change.
- Add a virtual-time gradual-drift regression. Verify capture completes,
  tracking still smooths, and the crop uses current corners and source pixels.
  Confirm the test fails with the old averaged crop, then passes with the fix.
- Validation: all 205 application tests pass, including all 15 camera tests.
  Strict all-feature/all-target Clippy, formatting and whitespace checks pass.
- Manual follow-up: gently move the document during auto-capture; verify
  frozen corners align despite live-overlay lag. Keep 6G3a open for this check;
  do not start 6G3b or commit without a request.

### 2026-10-19 — accept 6G3a; device/mode checkpoint 6G3b

- User accepts the captured-corner lag fix and requests the next slice,
  explicitly requiring at least full-HD support. Mark 6G3a manually accepted;
  keep real SP6 lifecycle/orientation calibration in 6G3c.
- Replace the numeric device field with named cameras and a supported-mode
  picker. Discover devices/modes in bounded, killable subprocesses. The modal
  owns each query handle, so switching, cancellation and closure kill it.
  Keep driver work off the UI thread and reject stale completion/session IDs.
- Share typed device/mode results with the standalone probe; add no dependency.
  Validate dimensions, RGBA byte limits, rate and format at the worker boundary.
  Filter/deduplicate unsupported modes and retain exact requests, not silent
  fallback. Linux continuous grids offer endpoints/common sizes and exact
  integer frame intervals; do not fabricate rounded fractional-FPS support.
- Prefer the largest supported resolution up to 1920×1080 at <=30 FPS,
  preferring MJPG at that size. Higher reported modes remain selectable.
  Stream and capture at the same selected resolution: keep original pixels
  and matching corners without re-opening the camera for a separate still.
- Reset preview/tracking on mode/device changes, retain mode on Retake and
  lock choices for a frozen photo. Keep Capture immediate, recover from missing
  or busy devices, and leave Use photo/correction cancellation unchanged.
- Add checks for catalog normalization, full-HD defaults/pixel preservation,
  Linux rational intervals and stepwise grids, malformed/empty worker args,
  bounded query output, timeouts, drop cancellation/reaping, camera/mode
  switching, stale queries/frames, errors and frozen-photo preservation.
  Use synthetic children and unpolled discovery tasks; no real camera opens.
- Validation: 517 native/all-target tests and 270 dependency-light tests pass;
  only the exhaustive recorded replay remains ignored. All 19 camera-screen
  and 21 camera-core checks pass. Strict Clippy, all-feature WASM, formatting
  and whitespace checks pass. Build the native app and smoke-check named-device
  discovery through its real helper flag without opening a stream. Keep all
  compilation under isolated `/tmp` targets; leave host target/dist untouched.
  Windows/macOS native compilation and physical capture remain CI/user checks.

Manual checkpoint 6G3b — pending:

1. Open Camera. Check named devices and the selected dimensions/FPS/format.
   Full HD should be preferred if advertised; higher resolutions remain listed.
2. Switch between 640×480 and 1920×1080 (or the highest supported mode).
   Check aspect, colors, preview responsiveness, auto-frame and manual Capture.
   Changing modes must restart tracking without reusing the old image/crop.
3. Capture, Retake and capture again. Verify Retake retains the selected mode.
   Use photo, Full frame, Accept and save/reopen; source pixels should retain
   the chosen resolution, not the displayed preview widget's dimensions.
4. If multiple cameras exist, switch between them. Check errors from a busy,
   unavailable or metadata-only device; retry another choice or Refresh cameras.
   Cancel during discovery/preview and reopen; no old result should overwrite
   the new choice or keep a device busy after cancellation.
5. Repeat front/rear and full-HD checks on SP6 when available. This laptop
   checkpoint does not establish Surface orientation/suspend/resume support.

Stop for this manual checkpoint. Do not start 6G3c or commit without a request.

### 2026-10-19 — full-HD preview upload flicker

- User reports rapid black/disappearing preview frames at full HD while other
  modes work. Installed Iced code uploads RGBA images >=2 MiB asynchronously;
  replacing every visible handle immediately exposes a not-yet-ready image.
- Use Iced's explicit image-allocation task. Hold the previous visible frame
  until upload completes, retain its allocation for immediate rendering, and
  acknowledge delivery only after upload to bound work end to end. Preserve
  original resolution and exact analyzed/captured pixels; add no resize step.
- Keep manual Capture on the visible image during an incoming upload. Ignore
  stale completion after capture, mode/session changes or camera cancellation.
  Upload failure leaves the previous picture and manual capture available.
- Reference the already-transitive `iced_runtime` package directly because
  Iced's allocator re-export requires its unnecessary all-codecs image feature.
  Add no new package, media runtime, image codec or unsafe code.
- TDD: reproduce early visible-frame replacement before the fix. Drive real
  allocation tasks with a headless software renderer; cover delayed full-HD
  replacement, first-frame readiness, retained full-HD export dimensions,
  capture during upload, errors/recovery and existing stale-result guards.
- Validation: all 212 application tests pass, including all 22 camera-screen
  checks. Strict all-feature/all-target Clippy, WASM, formatting and whitespace
  checks pass. Keep 6G3b open for a full-HD camera retry; physical GPU/camera
  acceptance cannot be inferred from headless tests. Leave changes uncommitted.

### 2026-10-19 — resolution-independent live framing

- User confirms the upload-flicker fix and clarifies this laptop camera's
  maximum is 1280×720, not full HD. Higher resolutions are covered by synthetic
  tests, not physical acceptance. User reports more sensitive framing and
  inability to finish automatic capture at the highest available resolution.
- Corner drift already uses normalized coordinates. The detector's pixel
  filters, Laplacian sharpness and analysis cost were not similarly bounded
  across the accepted VGA preview and larger modes, which can change both
  quality gating and stale-result frequency despite the same apparent scene.
- Share the existing detector through a small live-preview entry point capped
  at a 640-pixel longest edge. Keep source validation, aspect, hints, normalized
  corners and the full-resolution frame for capture. Change no confidence,
  movement, sharpness or countdown threshold. Still-image Auto frame and
  offline replay retain the existing 1280-pixel path and calibration.
- TDD: first reproduce differing analysis dimensions. A rescaled, mildly
  blurred synthetic form passes the VGA sharpness gate but fails it at raw
  720p; normalized live analysis detects the same corners and reaches capture
  at 640×360, 1280×720 and 1920×1080. Add three recorded-camera live samples
  checking the outer sheet and quality; extend allocation/aspect checks for
  both analysis limits and portrait inputs. Keep the full replay opt-in.
- Validation: 522 native/all-target and 272 dependency-light checks pass;
  keep the full 224-frame replay ignored. Strict Clippy, all-feature WASM,
  formatting and whitespace checks pass. No camera opens in these tests.
  Stop for a 1280×720 live-camera retry; do not claim resolution normalization
  removes genuine blur or hand motion. Keep changes uncommitted.

### 2026-10-19 — accept 6G3b; lifecycle checkpoint 6G3c

- User accepts the normalized 720p preview/framing/countdown and requests
  the final 6G3 slice. Mark 6G3b manually checked. Laptop maximum is 1280×720;
  do not claim physical full-HD or SP6 coverage.
- Add explicit quarter-turn and horizontal-mirror controls. Apply them to
  accepted frames off the UI thread, before upload, detection and encoding,
  so preview, crop and saved pixels share coordinates without resampling.
  Retain orientation for mode changes/Retake/Resume; reset on choosing a
  different camera. No sensor metadata, display-rotation hook or per-device
  persistent preferences are added.
- Add Pause/Resume. User explicitly chooses to remain paused after sleep
  until Resume preview. Keep pause intent through discovery/configuration
  changes; reject old frames, uploads, queries, jobs and queued starts.
  Frozen photos and Use photo/Cancel semantics remain unchanged.
- Reuse the independent driver watchdog: a three-second wall-clock polling
  gap or rollback kills/reaps the helper and reports an interruption, mapped
  to neutral paused status. This catches sleep/long process interruptions
  without stopping preview for window moves or focus changes. It runs on
  wake, not before suspend; very brief sleeps are not guaranteed to pause.
  Also reject analysis by frame wall age: Linux Instant may exclude sleep.
- Include actionable Windows desktop-camera and Linux session/device-access
  guidance with camera errors. Never change permissions, require elevation,
  developer mode or package identity. Keep unsupported macOS/browser capture
  unchanged; retain the existing native/WASM isolation.
- Automated checks use exact rotation/mirror pixels, real PNG encoding,
  headless allocations, synthetic query results and killable test children.
  Cover pause/configuration, interruption, stale ready-to-capture results,
  discovery, frozen photos, permission/busy errors and recovery. Inject clock
  jumps; never sleep a real machine or open a real camera from tests.
- Validation: 532 native/all-target tests pass, including 221 app checks;
  all 272 dependency-light tests pass. Keep the full 224-frame replay ignored.
  Strict all-feature/all-target Clippy, all-feature WASM, formatting and
  whitespace checks pass. Use only isolated `/tmp` compilation targets.
  Native Windows/macOS compilation remains a CI check; physical camera,
  permission prompts and system sleep remain user checks.

Manual checkpoint 6G3c — accepted; sleep/SP6 checks deferred:

1. Rotate a printed form through all four settings, then toggle Mirror preview.
   Verify preview colors/aspect and quad placement. Capture keeps the same
   mirrored orientation; Use photo opens the unmirrored page editor. Check
   Full frame / Accept; save/reopen.
   A named Front camera defaults to mirrored live preview; unknown names do not.
2. Retake and change resolution: retain orientation and reset countdown.
   If multiple cameras exist, select another and check fresh orientation.
3. Pause; change orientation/mode or Refresh cameras. Stay paused with no
   old image/countdown until Resume preview. Capture should work immediately
   after a fresh image appears. Move/resize/focus changes must keep it running.
4. Deferred by user: sleep with live preview open for at least five seconds.
   On wake, remain paused, then Resume. No pre-sleep result may trigger a
   capture. A frozen photo must survive sleep. Cancel/reopen and check release.
5. Try a busy/denied/unavailable camera. Check inline guidance, retry/refresh,
   and unchanged project contents after cancellation. Repeat front/rear,
   portrait/landscape and suspend/resume checks on actual Windows/SP6 hardware.

User accepts this checkpoint after the follow-ups recorded below. Physical
sleep and Surface calibration remain deferred, not inferred from laptop tests.

### 2026-10-19 — preview-only mirroring and shared camera playback control

- User requests front-camera mirroring only as a live positioning aid.
  Rotate source pixels, but mirror only the displayed live image and overlay.
  Detection, captured preview, page corrections and PNG/PDF content retain
  unmirrored coordinates/pixels, so printed text remains readable.
- Native adapters expose device names, not facing metadata. Default Mirror
  preview on for an explicit Front name token, case-insensitively; reject
  ambiguous Front/Rear labels and leave generic cameras unmirrored. Keep the
  manual checkbox. Refresh preserves the selected camera and manual override;
  switching to another camera restores its own default.
- Keep normal and mirrored renderer allocations ready when mirroring is on.
  Wait for both before replacing the visible frame, then switch immediately
  to normal preview on capture. Retain one frame in flight through upload;
  no high-resolution blank-frame regression or late crop coordinate conversion.
- Replace separate Pause/Start buttons with one context-sensitive control:
  Pause while running, Resume preview while paused, Start after errors,
  Retake after capture. Preserve existing pause/configuration/session guards.
- User explicitly defers physical sleep testing. Record it as deferred, not
  passed. Keep existing watchdog/stale-frame checks and hardware limitations.
- TDD: reproduce mirroring of source pixels before fixing it. Check four
  lossless rotations, all mirror settings, asymmetric preview/quad reflection,
  readable captured PNG pixels, full-HD dual allocations, front-name defaults,
  device switching and manual overrides retained across refresh.
- Validation: 535 native/all-target tests pass, including 224 app checks
  and 34 camera-screen checks. Keep exhaustive replay opt-in. Strict Clippy,
  all-feature WASM, formatting and whitespace checks pass. Hardware checks
  remain manual; no actual camera or host build directory is used here.

### 2026-10-19 — keep camera review mirrored until Use photo

- User finds the immediate unmirror on capture confusing. Keep camera live
  preview, frozen review and crop overlay mirrored for the whole camera
  screen, including PNG preparation. Unmirror only when Use photo leaves
  that screen for page editing. Source pixels and crop coordinates remain
  normal throughout; saved/exported content stays readable.
- Remove the now-unnecessary second renderer allocation per mirrored frame.
  Upload one display image while retaining the normal source pixels; keep
  existing upload readiness, backpressure and stale-session guards.
- TDD: reproduce the capture-time flip in the actual camera view. Verify
  mirrored pixels before capture, after capture and during Use photo,
  followed by normal source bytes in the page editor. Preserve asymmetric
  overlay reflection, PNG encoding and high-resolution allocation checks.
- All 536 native/all-target checks pass, including 35 camera-screen checks.
  Strict Clippy, all-feature WASM, formatting and whitespace checks pass.
  Physical sleep testing remains deferred, not marked accepted.

### 2026-10-19 — accept 6G3c and commit native camera integration

- User accepts the final camera review flow and requests all pending changes
  be committed. Mark 6G3c, 6G3 and Phase 6 accepted on available hardware.
- Retain physical sleep and actual Windows/SP6 front/rear calibration as
  explicit deferred checks. Do not convert deferred coverage into acceptance.
- Include the restored native probe, Linux/Windows adapters, killable driver
  helpers, manual capture, live framing/countdown, device/mode controls,
  orientation, preview-only mirroring, lifecycle recovery and regression tests.
- Preserve the existing 536 native/all-target and 272 core passing checks,
  strict Clippy, WASM, formatting and whitespace validation. Keep exhaustive
  replay opt-in and all compilation outside host-owned target directories.
- Commit unsigned as requested by repository policy. Do not push or start
  Phase 7/UI polish in this turn; keep the C# app and release workflow intact.

### 2026-10-19 — Phase 7A prebuilt Windows testing and warning cleanup

- User wants Windows testing without installing Rust or compiling a 23 GB
  target directory. Start only the CI/portable-delivery checkpoint of Phase 7;
  do not begin the broader UI polish or replace the C# app.
- Add a dedicated Windows release job using the existing pinned toolchain,
  explicit MSVC x64 target and static CRT. Cache release dependencies with
  the official cache action; keep bulky debug output off that job/zip.
- Add one ZIP containing the renamed prototype EXE, short run instructions,
  license, sample form and exact build revision/run link. Check PowerShell
  syntax before compilation, inspect runtime imports, extract the archive
  away from Cargo output and run a bounded no-camera startup check there.
- Upload the ZIP without double-wrapping it. Publish only successful master
  runs after native, WASM and package checks to a separate public `rust-dev`
  prerelease. Pull requests get artifacts only; grant release writes only
  to the publishing job. Keep C# dev/version assets and latest release intact.
- Fix the reported Windows unreachable-pattern warning: the installed enum
  has exactly six supported formats. Replace filter_map with exhaustive map;
  a future enum change must be handled explicitly, not silently discarded.
- Document direct download/extract/run, unsigned-build warnings, artifact
  retention and remaining hardware checks. No Windows tools are installed
  in the container; the first Windows CI run remains the packaging gate.
- Test the workflow's actual publication shell with a mock GitHub CLI for
  new/existing releases and failed uploads. Assert separate rust-dev names,
  prerelease/non-latest flags and no tag advancement after upload failure.
- Validation: all 536 native/all-target tests, three release-automation tests,
  strict Clippy, all-feature WASM, formatting and whitespace checks pass.
  Check expression context availability against GitHub's reference; set the
  runner-temp target directory in a step, not the unavailable job-env context.
  Windows packaging/script execution and real hardware remain CI/user checks.

### 2026-10-19 — Cache all Rust CI compilation jobs

- The Windows portable job already cached release dependencies, but native
  Linux/Windows/macOS tests and WASM checks started cold on every run.
- Cache Cargo registry/git data and compiled target output in each job.
  Separate native OS/architecture, WASM metadata and static-CRT releases;
  include pinned Rust and Cargo manifest/lockfile hashes in cache keys.
- Restore the preceding compatible cache when dependencies change, so Cargo
  can reuse unchanged crates. Keep all tests and checks running on cache
  hits; leave C# workflows and local target directories unchanged.
- Omit CI-only debug symbols and incremental compiler state to keep caches
  smaller. Preserve existing optimization, debug assertions and local
  debugging settings. The first successful run must populate the caches.
- Add dependency-free Node checks for cache paths, key/fallback isolation,
  restore ordering and compact CI settings; run them in native Linux CI.
- Validation: all eight Node test entries pass, including the four new
  cache checks and existing publication/download checks; whitespace checks
  pass. Actual cache population, hits and timings still require GitHub CI.

### 2026-10-19 — Isolate macOS capacity from Windows publication

- User reports macOS ARM64 runner acquisition failed after about eighteen
  minutes with GitHub capacity/internal-server errors. No macOS build or
  test failure is established. Its matrix cancellation blocked publication
  despite a successful Windows portable job and native Windows checks.
- Keep Linux/Windows in the required native matrix. Move macOS to an
  independent Intel runner job and reuse the same cached native steps with
  a YAML anchor. GitHub documents both the runner label and YAML aliases.
  Use runner.os for Linux-only steps so the shared list needs no matrix.
- Keep macOS failures visible in overall workflow status, not ignored with
  continue-on-error. Publication depends only on Linux/Windows, WASM and
  the package job; do not bypass any of those checks with always().
- Add a regression for gate membership, the independent macOS runner and
  shared tests. All nine Node test entries and whitespace checks pass.
  Actual scheduling/publication still require another GitHub run.
- At the time of the report, cache changes were still uncommitted; the
  seventeen-minute Windows test run did not measure warm-cache performance.
  Preserve C# workflows, existing release assets and deferred device checks.

### 2026-10-19 — Preserve the portable ZIP during publication

- Inspect rust/logs_106435626898.zip: artifact upload/download and digest
  verification succeeded, but the download action extracted the raw ZIP.
  GitHub CLI then failed because the expected archive file was absent.
- Set skip-decompress on the release download to keep the uploaded ZIP
  intact. Check that it is nonempty before any release or tag operation,
  with a clear error if missing. Keep permissions and release gates intact.
- First reproduce the missing setting and missing/empty-file failures.
  Extend the real-shell mock tests with archive fixtures and assert no
  GitHub calls occur without a usable file. All twelve Node test entries
  and whitespace checks pass; actual publication still awaits GitHub CI.
- Confirm release dependency caching from the same log: restore succeeded
  and only FastFill recompiled, finishing the release build in 24.34 seconds.
  Leave the supplied log ZIP untouched and untracked.

### 2026-10-20 — Windows console and camera-mode discovery feedback

- User reports the CI build succeeds and the Windows EXE starts, but a
  terminal remains open beside the GUI. Both a regular webcam and an offline
  Android-camera driver are listed; mode discovery fails for both after a
  few seconds with the same empty/oversized-catalog error. Refresh repeats it.
- Mark Windows release binaries as GUI-subsystem applications; keep debug
  consoles and the separate command-line camera probe unchanged.
- An empty supported-mode list has a nonempty JSON reply. The reported error
  instead means the helper returned no payload; the actual Windows driver
  failure cannot be established in this Linux container.
- Query Windows formats through the already-transitive Media Foundation
  bindings instead of Camera::new, which selects/sets a format before it
  can list modes. Keep capture negotiation and device choices unchanged;
  do not let an offline camera remove the usable camera from the picker.
- Flush catalog replies explicitly. Retain only 4 KiB of stderr while
  draining the rest to avoid pipe stalls. Observe worker exit status before
  reaping, and distinguish empty replies from oversized catalogs. Preserve
  timeout, sleep interruption, cancellation and process cleanup behavior.
- Reproduce the previously hidden nonzero exit and misleading empty-reply
  errors first. Add empty-mode/error serialization and offline-to-working
  camera selection checks. Exercise noisy stderr and helper exit diagnostics
  without hardware; keep physical Windows/SP6 verification deferred.
- Extend extracted Windows package checks to inspect the GUI subsystem and
  verify stdout/stderr pipes from the actual release EXE. Device-name listing
  opens no capture stream and accepts a well-formed driver error on the CI
  host, so no camera is required. Actual PowerShell/MSVC checks remain in CI.
- Validation: all 539 native/all-target tests and twelve Node automation
  entries pass; exhaustive replay remains ignored/opt-in. Strict Clippy,
  all-feature WASM compilation, formatting and whitespace checks pass.
  Compilation stays under /tmp, not host target/dist.
- Next Windows check: confirm Explorer starts only the GUI, regular-webcam
  modes/preview work, and switching to the offline driver and back recovers.
  If mode listing still fails, capture the new complete helper error;
  do not claim the physical driver failure fixed from Linux-only tests.
- User requests committing the fix and continuing without Windows access.
  Defer physical retesting explicitly; do not mark camera behavior accepted.

### 2026-10-20 — Start UI polish with checkpoint 7B

- Commit the Windows console/camera-query fix unsigned as requested, with
  physical Windows retesting explicitly deferred while hardware is absent.
  Start one small UI checkpoint, not the entire visual redesign at once.
- TDD red: the old wheel handler changes 24 pt to 32, not the required 25.
  Replace preset sizes with a numeric field and 40-pixel minus/plus buttons;
  wheel and buttons change sizes by one point within the model's 6–144 range.
- Keep numeric input uncommitted until Enter or an existing editing action
  accepts it. Partial/invalid values never alter annotations or history.
  Escape/Undo discard a numeric draft first; inline text remains pending.
  Invalid Enter retains the field for correction and shows an explicit error.
- Reuse the existing text-format handler for measurement, selected-text
  transactions, and pending-text Accept/Escape behavior. Use existing accept
  paths for page/tool/navigation/save changes rather than duplicate dispatch
  lists. Keep modal/project-operation guards ahead of numeric editing.
- Display imported/scaled sizes compactly without changing stored precision.
  Support decimal dot/comma, preserve mixed-font selections, apply a chosen
  size to selected text only, and undo a group size change in one operation.
- Move text-option rendering and numeric-field handling into text_controls,
  with separate colocated tests. Shrink view.rs; use existing spacing/theme
  tokens and native iced controls, not a new dependency or widget framework.
- Thirteen focused tests cover numeric drafts, validation, limits, fractional
  values, wheel steps, inline Escape, grouped Undo, and project guards. A
  headless layout check verifies button/entry sizes stay fixed for 6–144 pt.
- Respect captured Delete events so editing the numeric field cannot also
  delete an annotation; keep Escape available to discard its pending value.
- Validation: all 550 native/all-target tests pass; exhaustive replay stays
  opt-in. Strict Clippy, all-feature WASM compilation, formatting and whitespace
  checks pass. Keep compilation under /tmp and the C# app unchanged.
- User confirms the Linux checkpoint on 2026-10-20. Mark 7B accepted and
  continue to the stable-confirmation overlay checkpoint. Keep physical
  touch and the Windows camera retest deferred.

### 2026-10-20 — Stable confirmation overlays, checkpoint 7C

- Record the user's Linux acceptance of 7B, then start the next small UI
  checkpoint. Keep hardware-only calibration and camera checks deferred.
- TDD red: opening Replace page increases the header height and moves/shrinks
  the editor. Reproduce this with the actual headless iced layout before
  connecting the overlay. Keep a regression for all six prompt paths at two
  window sizes, including each editor/toolbar/rail descendant's bounds.
- Replace the three page/annotation header prompts and project-leave banner
  with one local confirmation module. Keep project, page and annotation
  workflow ownership unchanged; no document/history/schema changes.
- Use native stack, opaque and mouse-area widgets instead of a modal widget
  framework or dependency. Keep an explicit renderer layer under the dialog
  so zoomed page images cannot cover its card or translucent backdrop.
- Centralize themed card/backdrop styling in appearance.rs. Center readable
  cards and equal-width 40-pixel actions, fitting down to a 320-pixel window.
  Use red destructive actions and explicit Keep/Cancel options. Backdrop
  input does not dismiss the dialog; Enter never confirms destruction.
- Six colocated tests check unchanged editor layout, actual button routing,
  backdrop mouse/touch/wheel isolation, late command guards, Escape without
  data loss, Delete all/Undo, and image/modal layering at 100%/200% scale.
- Validation: all 556 native/all-target tests pass; exhaustive camera replay
  remains opt-in. Strict Clippy, all-feature WASM compilation, formatting and
  whitespace checks pass. Rebuild cold container caches under /tmp only;
  leave host target/dist, C# code and release workflows unchanged.
- User confirms dialogs work well on 2026-10-20. Mark 7C manually accepted
  and continue with compact vector tools. Keep Windows/real-touch deferrals.

### 2026-10-20 — Compact vector tools and swatches, checkpoint 7D

- Accept the user's 7C dialog check and start one toolbar-only polish step.
  Keep larger panel/rail animation, startup/help and packaging work separate.
- TDD red: Navigate measures 83.536 by 40 pixels, not the required equal
  40-pixel square target. Retain the check across all tools, options and colors.
- Draw eleven tools plus magnet/bucket as monochrome native canvas paths.
  Use one themed button/foreground policy for selected and disabled glyphs;
  center rectangle/oval with equal bounds and stroke weight. Add no platform
  font, emoji, bitmap, dependency or generic icon framework.
- Preserve active-tool labels and native half-second hover hints. Suppress
  hints when disabled so they cannot remain above modal/busy workflows.
  Keep smart-snap readiness visible, global snap state, per-tool defaults and
  existing annotation command routing, including after temporary selection.
- Replace named color buttons with six round 32-pixel patches in 40-pixel
  targets. Add a two-pixel selection ring with a two-pixel pigment gap; keep
  colors unchanged on hover and retain their RGB pigment when dimmed.
- Move toolbar rendering to its own module with colocated tests; reduce
  view.rs to window composition/document editing. Keep touched style/size
  tokens in appearance.rs, with padding derived from icon/swatch size.
  Maintain three fixed-height rows so tools/options never resize the page.
- Six new checks cover full-target mouse/touch routing and disabled controls,
  geometry/centering, all thirteen glyphs at 100%/200% in light/dark themes,
  selected/disabled rendering, swatch hover/ring gaps, and post-placement snap.
  Reuse headless rendering with optional theme/cursor parameters; inspect
  temporary icon grids and editor preview without adding generated assets.
- Validation: all 562 native/all-target tests pass; exhaustive camera replay
  remains ignored/opt-in. Strict Clippy, all-feature WASM, formatting and
  whitespace checks pass. Keep builds/previews under /tmp, not host target
  or dist; leave C# code, serialization and release workflows unchanged.
- User accepts the icons on Linux except the ambiguous Text glyph. Replace
  that glyph with Aa using existing bundled Fira Sans, preserving selected
  and disabled foregrounds. Add a headless letter-region regression at both
  DPI scales and themes. Keep other icons unchanged.
- Record scrollbar/button overlap for the later toolbar layout redesign,
  as requested. Mark 7D accepted; do not infer Windows/real-touch checks.
- Validate the Aa follow-up: all 563 native/all-target tests, strict Clippy,
  WASM compilation, formatting and whitespace pass. Inspect light/dark
  previews, including selected/disabled states. Leave changes uncommitted.

## References

- [iced 0.14 documentation](https://docs.rs/iced/0.14.0/iced/)
- [iced 0.14 features and
  MSRV](https://github.com/iced-rs/iced/blob/0.14.0/Cargo.toml)
- [Hayro PDF renderer](https://github.com/LaurenzV/hayro)
- Existing behavior: [user guide](../docs/USER_GUIDE.md).
- Existing checks and fixtures: `../src/FastFill.Checks/`, `../tests/`.
