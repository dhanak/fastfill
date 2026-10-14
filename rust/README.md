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

Project dialogs remember their last successful folder during the run. File
operations block editing until finished. Page geometry, fonts, and strokes use
point units; 1:1 still matches the imported image's original pixels. Saving
does not resize or recompress imported images.

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
  move it. Drag empty page space to select every intersecting shape. Ctrl-click
  toggles individual shapes. Drag any selected shape to move the whole shape
  selection. Faint member frames identify each shape inside the strong group
  boundary. A single shape retains resize, endpoint, and rotation controls.
- **Bulk styles:** color and thickness apply to every selected object. Fill
  applies only to compatible rectangles, ovals, and pen paths.
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
Pages, annotations, and history are temporary. Mixed text/shape multi-selection
and project open/save UI are not implemented. Schema-v1 archive load/save APIs
are covered by automated compatibility tests. Minimal Linux or WASM environments
without requested generic or italic fonts fall back to bundled Fira Sans.
Android browser rendering is currently slow; native touchscreen performance
still needs hardware testing.
