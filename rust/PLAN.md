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

#### 3K. Multi-selection, layers, and bulk deletion — not started

- [ ] Ctrl-click to toggle objects and drag an empty-area selection rectangle.
- [ ] Move selections together and apply compatible style changes in bulk.
- [ ] Delete selected objects and delete all with confirmation.
- [ ] Send selected objects to back or bring them to front.
- [ ] Preserve selection and layer order through undo/redo.
- [ ] User checkpoint: mixed-object selection, layers, and bulk operations.

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

## References

- [iced 0.14 documentation](https://docs.rs/iced/0.14.0/iced/)
- [iced 0.14 features and
  MSRV](https://github.com/iced-rs/iced/blob/0.14.0/Cargo.toml)
- Existing behavior: [user guide](../docs/USER_GUIDE.md).
- Existing checks and fixtures: `../src/FastFill.Checks/`, `../tests/`.
