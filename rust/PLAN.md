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

### 6. Correction, snapping, and camera — in progress

- Port image correction and form snapping, retaining calibrated fixtures.
- Port document detection and temporal capture gating with all 224 frames.
- Add native camera adapters only after recorded-image tests pass.
- Keep blocking native work outside UI updates. Browser implementations may
  use different adapters; avoid unconditional native-only dependencies.
- User checkpoints: lab parity first, then laptop camera, then SP6 cameras,
  permissions, orientation, countdown, cancellation, and suspend/resume.

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

### 7. Packaging and replacement decision — not started

- Polish the UI for regular use: cohesive visual styling, tool icons, touch
  layout, and confirmations that overlay the canvas without resizing it.
- Replace the font-size dropdown with a touch-friendly numeric spinner:
  1 pt increments instead of the current 4 pt choices. Show and preserve any
  valid imported size, including the fixtures' 22 pt. Retain size validation,
  wheel adjustment, and pending-text Accept/Escape semantics.
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

## References

- [iced 0.14 documentation](https://docs.rs/iced/0.14.0/iced/)
- [iced 0.14 features and
  MSRV](https://github.com/iced-rs/iced/blob/0.14.0/Cargo.toml)
- [Hayro PDF renderer](https://github.com/LaurenzV/hayro)
- Existing behavior: [user guide](../docs/USER_GUIDE.md).
- Existing checks and fixtures: `../src/FastFill.Checks/`, `../tests/`.
