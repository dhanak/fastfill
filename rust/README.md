# FastFill Rust prototype

Experimental cross-platform rewrite. The production C#/WinUI application is
unchanged. See [PLAN.md](PLAN.md) for scope, progress, and checkpoint gates.

Install the pinned toolchain and run all local checks:

```sh
mise install
mise exec -- cargo fmt --check
mise exec -- cargo clippy --all-targets --all-features -- -D warnings
mise exec -- cargo test --all-features
mise exec -- cargo check --target wasm32-unknown-unknown
```

The WASM command needs its standard library once per toolchain:

```sh
mise exec -- rustup target add wasm32-unknown-unknown
```

Run the current native prototype:

```sh
mise exec -- cargo run
```

Debug builds optimize dependencies (including image/PDF codecs), while keeping
FastFill code unoptimized for quick edits and debugging. The first build after
this setting changes is slower; subsequent application edits reuse that work.
Use `mise exec -- cargo run --release` to assess release performance.

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
- **Save / Save as:** the first save chooses a destination; later Save writes
  that file atomically. Save as chooses another destination. Use `.docscan`
  filenames. Original PNG/JPEG bytes remain intact; blank pages save as white
  PNGs. Save accepts pending text and cancels unfinished shape drags.
- **Dirty marker:** `*` beside the filename means unsaved content. Page
  imports, replacement, deletion, reorder, and annotation changes count.
  Selection, navigation, and tool defaults do not. Undo back to the saved
  revision clears the marker; redo restores it.
- **Close:** the native window's close button also guards unsaved changes.
  Save failures/cancellation never continue New, Open, or Close. Escape
  cancels the confirmation; changing focus does not discard it.
- **Shortcuts:** Ctrl+N New, Ctrl+O Open, Ctrl+S Save, Ctrl+Shift+S Save as.
  Use Command instead of Ctrl on macOS.

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
PDF export and recovery remain separate upcoming checkpoints.

Project dialogs remember their last successful folder during the run. File
operations block editing until finished. Page geometry, fonts, and strokes use
point units; 1:1 still matches the imported image's original pixels. Saving
does not resize or recompress imported images.

Opening reuses fully validated image pixels for display, keeping the original
encoded bytes for saving. PDF import likewise retains its raster rather than
decoding the new PNG again. Saving fully validates source images once; native
temporary-file read-back verifies exact bytes and ZIP CRC before atomic commit.
No archive thumbnail/schema change is required for these optimizations.

In browsers, Open uploads a file and Save/Save as downloads a `.docscan`; no
original path can be reused. Download completion means the browser accepted
the download request, not verified disk persistence. Closing/reloading the
browser tab is not guarded; save first. Large archive/image processing can
still pause the browser. These are harness limits, not native behavior.

Editable conversion currently accepts full-frame, unfiltered, unrotated pages,
rigid annotation rotation/translation, and the three mapped font families.
It rejects crop/filter corrections, affine scaling/shear, other fonts, and
legacy soft-wrap settings instead of silently altering them. The archive API
still preserves complete schema-v1 data; these are temporary editor limits,
not a permanent v1 compatibility policy. Generic font rendering is not promised
to be pixel-identical to C#. Schema-v2 output is allowed when needed; this
checkpoint retains v1 output. Test existing projects with copies, not originals.

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
  changes. Click accepted text with Text to reopen it. Select clicks select
  text without opening the editor; drag to move it or use its rotation handle.
  Choose Sans, Serif, or Monospace; size; bold; italic; underline; and color.
  Editor changes remain pending until acceptance; changes to inactive selected
  text apply immediately and support undo. Scroll over the size selector to
  step through sizes.
- **Styles:** choose black, white, red, blue, green, or yellow. Use −/+ for
  stroke widths from 1–20 points. Rectangle, Oval, and Pen offer Fill.
  Each tool remembers its own settings; Select changes the chosen object
  with undo.
- **Select:** click or touch a shape, line, ink path, or text, then drag to
  move it. Drag empty page space to select intersecting shapes and text.
  Ctrl-click toggles individual members. Drag any selected member to move the
  whole selection. Faint member frames identify membership inside the strong
  group boundary. A single object retains its applicable editing controls.
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

Save projects explicitly; automatic recovery is not implemented yet.
Schema-v1 archive load/save
APIs are covered by automated compatibility tests. Minimal Linux or WASM
environments without requested generic or italic fonts fall back to bundled
Fira Sans.
Android browser rendering is currently slow; native touchscreen performance
still needs hardware testing.
