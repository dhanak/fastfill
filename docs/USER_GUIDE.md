# FastFill user manual

FastFill scans, corrects, annotates, saves, and shares documents on Windows 10
and 11. The interface supports touch, mouse, pen, and optional keyboard use.
Screenshots in this guide were captured on Windows 10.

[← Back to README](../README.md)

## Install and launch

FastFill requires 64-bit Windows 10 version 1809 or later. The portable build
needs no installer, developer license, .NET runtime, or separate Windows App
SDK runtime.

1. [Download the latest portable release][release].
2. Extract the ZIP to any folder.
3. Keep every extracted file together.
4. Run `FastFill\FastFill.exe`.

Development builds use a persistent self-signed certificate. To trust it for
the current Windows account:

1. Open `FastFill\FastFill.cer` and select **Install Certificate**.
2. Select **Current User**.
3. Place it in **Trusted Root Certification Authorities**.
4. Finish the import, then run `FastFill.exe`.

Only trust the certificate downloaded from this repository. Self-signing does
not create public SmartScreen reputation. If Windows shows an unrecognized-app
warning, select **More info**, then **Run anyway**.

## Typical workflow

1. Start a camera scan, import an image or PDF, open a project, or recover an
   autosave.
2. Review page boundaries, rotation, color, and contrast.
3. Add and arrange annotations.
4. Add, import, delete, or reorder pages.
5. Save the editable `.docscan`, export a PDF, or share it.

## Start or resume

[![FastFill start screen][shot-home]][shot-home]

- **New camera scan** creates a project and opens the camera.
- **Import image / PDF** creates a project from JPEG, PNG, or PDF input.
  Images open in page review. Every page of a PDF is imported at 300 DPI;
  select **Edit page** afterward to crop or filter an imported PDF page.
- **Open project** opens an editable `.docscan` file. FastFill also opens a
  `.docscan` passed to `FastFill.exe`, including through Windows **Open with**
  or a file association.
- **Recover autosave** opens the latest recoverable workspace. The button is
  disabled when no autosave exists.

FastFill opens maximized. The top status line reports capture, rendering,
autosave, and modifier state.
**Shortcuts** is always available. Project commands become available after a
page has been accepted or imported.

## Capture a page

Auto-capture is always on:

1. Hold the document inside the camera view.
2. A cyan quadrilateral shows the stabilized boundary FastFill currently
   recognizes. A faint quadrilateral is only a tentative candidate.
3. Keep the document steady while the ring around the shutter completes its
   two-second countdown. FastFill then takes the photo automatically.

Tap the shutter at any time for immediate capture. Use the upper-right camera
button to switch between available front and rear cameras. Use **Back** to
return without capturing. If detection misses the document, capture manually
and correct the four corners on the next screen.

FastFill stops the camera while its window is hidden and restarts it on
return. If access is denied, enable camera permission in Windows Settings.

## Review and correct a page

[![Page review and correction][shot-review]][shot-review]

- **Adjust corners** switches between the original image with four crop
  handles and the processed preview. Drag each handle independently.
- **Ctrl+drag** a crop handle to snap it to a nearby image corner or edge.
  Drag without Ctrl for unrestricted placement.
- **Auto-detect** reruns boundary detection on the still image. This can
  replace a poor live-camera result.
- **Full frame** resets all crop handles to the image corners.
- **Color** offers Original, Enhanced color, Grayscale, and Black and white.
- **Contrast** applies correction from -100 to +100. Color and contrast
  changes appear in the processed preview immediately.
- **Rotate** turns the page clockwise by 90 degrees.
- **Retake** returns a new capture to the camera. While editing an existing
  page, **Cancel** discards the review changes instead.
- **Accept page** stores the crop and corrections and opens the editor.

Choosing a color mode, moving contrast, or rotating leaves corner-adjustment
mode so the processed result is visible. Select **Adjust corners** again to
return to the source image and handles. **Auto-detect** and **Full frame** are
shown only while corner adjustment is active.

## Editor layout

[![FastFill editor and selection controls][shot-editor]][shot-editor]

The left rail manages pages, the center shows the current page, and the right
rail contains annotation tools. On narrow windows the rails become horizontal
strips. The top bar contains these project-wide actions:

- **Close project** returns to the start screen. Unsaved work or an unaccepted
  capture requires confirmation.
- **Shortcuts** opens the in-app keyboard and pointer reference.
- **Undo / Redo** traverse accepted page and annotation changes.
- **Save project** writes an editable `.docscan`; later saves reuse its path.
- **Export PDF** writes every page as a flattened 300-DPI PDF.
- **Share** creates a PDF and opens the Windows share sheet for Mail and other
  installed share targets.

## Pages

- Select a page in the left rail to display it.
- Drag a page row to reorder it, or use the **↑ / ↓** buttons.
- **Add page** captures another page with the camera.
- **Import** adds a JPEG, PNG, or all pages of a PDF.
- **Edit page** reopens crop, rotation, color, and contrast controls without
  flattening annotations.
- **Delete page** removes the current page after confirmation.

A project can contain up to 20 pages. Import is rejected if all incoming PDF
pages do not fit.

## Annotation tools

[![Annotation tool palette and options][shot-tools]][shot-tools]

- **Navigate** (hand): pan with one pointer; pinch with two fingers to pan and
  zoom. The floating bar provides zoom out, zoom in, raster **1:1**, and
  **Full view**. Mouse-wheel zoom works while this tool is active.
- **Select** (pointer): select, move, resize, rotate, or multi-select objects.
- **Text**: tap to center new text at that point, or drag to set its maximum
  width. Text height and bounding box fit their contents automatically.
- **Pen**: draw freehand. **Fill** closes the first and last points and fills
  the resulting shape.
- **Highlighter**: draw a translucent freehand stroke.
- **Line / Arrow**: drag between endpoints. Their two handles move endpoints;
  rotation is intentionally unavailable.
- **Box / Oval**: drag any rectangular bounds. **Fill** switches between an
  outline and a solid shape.
- **Checkmark / X mark**: tap for a small centered mark or drag its size. Both
  keep a square aspect ratio.

[![Annotation examples and selection handles][shot-selection]][shot-selection]

Relevant options appear below the tool grid:

- Six colors: black, white, red, blue, green, and yellow.
- Stroke thickness from 1 to 20 for ink, marks, lines, and shapes.
- Fill for pen, box, and oval.
- Typeface, 6–144 point size, bold, italic, and underline for text.
- Smart snap for text, checkmarks, and X marks.

Color, thickness, fill, and text style are remembered independently for each
tool during the session. Selecting existing objects loads their options;
changing an option applies it to every compatible selected object.

## Smart snap and text

[![Inline text editing with smart snap][shot-text]][shot-text]

The magnet button is one shared smart-snap toggle for all compatible tools:

- With **Text**, tap near a printed solid, dashed, or dotted form line.
  FastFill positions the text above it and uses the detected line width.
- With **Checkmark** or **X mark**, tap near a printed box. FastFill centers
  and sizes the mark inside the box.

Smart snap is based on page coordinates, so its result does not depend on
display zoom. Tap rather than drag when requesting a snap. The magnet state is
preserved while changing tools and pages during the current session.

Text is edited directly on the page. The translucent blue frame follows the
rendered text as font or contents change. Press **Enter** or click elsewhere
to accept, **Shift+Enter** for a line break, or **Esc** to cancel. Double-click
or double-tap existing text with Select or Text active to edit it again.

Text has no corner resize handles. Change its point size in the right rail or
use the bottom **− / +** buttons. Press **Enter** in the font-size field to
apply the value and return focus to the page.

## Select and transform annotations

- Click or tap an object to select it. Drag anywhere inside its bounding box
  to move it.
- Drag a corner handle to resize freely. Line and arrow handles move only the
  corresponding endpoint.
- Drag the round handle above a supported object to rotate it.
- **Ctrl+click** toggles one object in a multi-selection. Drag an empty area
  to select intersecting objects; **Ctrl+drag** adds them to the selection.
- **Shift+drag** forces movement when a small object or nearby handle would be
  ambiguous. **Alt+drag** forces resize from the nearest handle.
- Hold **Ctrl** while rotating to snap to 45-degree increments.
- Hold **Ctrl** while drawing or resizing a box or oval to make a square or
  circle. Hold it while drawing or moving a line/arrow endpoint to snap its
  angle to 45-degree increments.

After creating an object, its handles remain usable even if its drawing tool
is still active. A press inside that selected bounding box edits the object
instead of creating another one.

The permanent controls at the bottom of the right rail:

[![Transform, layer, and delete controls][shot-actions]][shot-actions]

- **↻ 0°** resets supported selected objects to zero rotation.
- **− / +** scale selected shapes and ink by 10%; for text they decrease or
  increase font size by one point.
- **To back / To front** move selected objects behind or ahead of all other
  annotations.
- The single-bin button deletes selected objects. The double-bin button
  deletes every annotation on the current page after confirmation.

The status line shows applicable modifiers while a drag is active.

## Zoom and pan

Normal drawing and selection do not move the page. Use one of these explicit
navigation methods:

- Select **Navigate**, then left-drag or one-finger drag to pan, two-finger
  pinch to pan and zoom, or use the mouse wheel to zoom.
- Right-drag to pan temporarily without changing the current tool.
- Hold **Ctrl+Win** while dragging, pinching, or using the mouse wheel to
  navigate temporarily without changing tools.

Zoom ranges from full view to 8× full-view scale. Panning is limited so the
page cannot be lost outside the viewport.

## Keyboard and pointer reference

The same reference is available from **Shortcuts** in the top bar.

| Context | Shortcut | Function |
| --- | --- | --- |
| Global | **Ctrl+Z** | Undo the last accepted change. |
| Global | **Ctrl+Y** | Redo the last undone change. |
| Selection | **Delete** | Delete selected annotations. |
| Selection | **Ctrl+click** | Add or remove one object from selection. |
| Selection | **Ctrl+drag empty area** | Add objects to selection rectangle. |
| Selection | **Shift+drag** | Always move selected object. |
| Selection | **Alt+drag** | Resize from nearest handle. |
| Geometry | **Ctrl+rotate** | Snap rotation to 45-degree increments. |
| Geometry | **Ctrl+drag line endpoint** | Snap line or arrow to 45 degrees. |
| Geometry | **Ctrl+draw/resize box or oval** | Constrain to square/circle. |
| Page crop | **Ctrl+drag corner** | Snap to nearby image feature. |
| Navigation | **Mouse wheel** | Zoom while Navigate or Ctrl+Win is active. |
| Navigation | **Right-drag** | Pan without changing tool. |
| Navigation | **Ctrl+Win+drag/pinch** | Temporarily pan or zoom. |
| Text | **Enter** | Accept text edit. |
| Text | **Shift+Enter** | Insert a line break. |
| Text | **Esc** | Cancel text edit. |
| Text | **Double-click/tap** | Edit existing text. |

## Projects, autosave, and privacy

A `.docscan` file is a validated ZIP containing the edit model and original
JPEG or PNG page assets. Crops, filters, page order, and annotations remain
editable. PDF export flattens the corrected pages and annotations.

FastFill autosaves workspace changes locally after a short delay and again
when the app closes. **Recover autosave** opens the newest available workspace;
use **Save project** for a durable file at a chosen location.

FastFill has no cloud sync or built-in SMTP client. Sharing uses the Windows
share sheet. Projects are not encrypted at rest, so protect sensitive files
with Windows account and disk security.

## Current limits

- Maximum 20 pages per project.
- No OCR or searchable-PDF generation.
- No cloud synchronization or direct SMTP delivery.
- Development certificate has no public publisher reputation.

[shot-editor]: screenshots/editor-shapes.png
[shot-actions]: screenshots/annotation-actions.png
[shot-home]: screenshots/home.png
[shot-review]: screenshots/page-review.png
[shot-selection]: screenshots/selection-transform.png
[shot-text]: screenshots/text-smart-snap.png
[shot-tools]: screenshots/annotation-tools.png
[release]: https://github.com/dhanak/fastfill/releases/latest/download/FastFill-portable.zip
