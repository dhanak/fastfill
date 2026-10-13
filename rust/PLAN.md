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
- Retain `.docscan` compatibility. Until compatibility and atomic-save tests
  pass, do not overwrite existing projects from the Rust application.
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

#### 3B. Select, move, resize, and delete — ready for user check

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
- [ ] User checkpoint: selection, drag edits, modifiers, and deletion.

Undo/redo, rotation, other shapes, and multi-selection belong to later slices.

### 4. Text and toolkit decision gate — not started

- Inline text, typeface/size/styles, transparent editing background, correct
  unrotated editor sizing, stable positioning, explicit accept/cancel.
- Formatting remains pending until accepted; Escape restores all changes.
- Test draft lifecycle, focus transitions, measurement, and undo.
- Prototype shared text/shape layout for screen and PDF before selecting a
  renderer/export dependency. Flag mismatches rather than hiding them.
- User checkpoint: rotated/bold text, multiline entry, touch and keyboard.
- Decide whether iced remains suitable before proceeding.

### 5. Projects, pages, imports, and PDF — not started

- Incremental slices: safe `.docscan` loading; compatible atomic saving;
  multiple pages/reorder; PDF import; matching PDF export; recovery.
- Keep the current 20-page limit and trust-boundary protections.
- Test C#/Rust cross-reading, archive limits, hostile input, interrupted
  saves, dirty revisions, cancellation, and exported page geometry.
- User checkpoint with copies of existing projects, never originals first.

### 6. Correction, snapping, and camera — not started

- Port image correction and form snapping, retaining calibrated fixtures.
- Port document detection and temporal capture gating with all 224 frames.
- Add native camera adapters only after recorded-image tests pass.
- Keep blocking native work outside UI updates. Browser implementations may
  use different adapters; avoid unconditional native-only dependencies.
- User checkpoints: lab parity first, then laptop camera, then SP6 cameras,
  permissions, orientation, countdown, cancellation, and suspend/resume.

### 7. Packaging and replacement decision — not started

- Polish the UI for regular use: cohesive visual styling, tool icons, touch
  layout, and confirmations that overlay the canvas without resizing it.
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

## References

- [iced 0.14 documentation](https://docs.rs/iced/0.14.0/iced/)
- [iced 0.14 features and
  MSRV](https://github.com/iced-rs/iced/blob/0.14.0/Cargo.toml)
- Existing behavior: [user guide](../docs/USER_GUIDE.md).
- Existing checks and fixtures: `../src/FastFill.Checks/`, `../tests/`.
