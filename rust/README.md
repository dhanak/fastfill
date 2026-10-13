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

Checkpoint 3D adds bounded-shape drawing, editing, and history to the viewer:

- **Rectangle:** drag with the left mouse button or one finger. The blue
  preview becomes a black outline on release. Hold Ctrl to draw a square.
- **Oval:** drag like a rectangle. Hold Ctrl to draw a circle.
- **Select:** click or touch anywhere inside a shape's box, then drag to move
  it. Drag one of its four handles to resize freely. Hold Ctrl for a square or
  circle, Shift to force movement, or Alt to force nearest-corner resizing.
- Rectangle and Oval modes can move or resize the selected object from its box
  or handles. Starting elsewhere draws a new shape of the active type.
- **Delete:** remove the selection with the button or Delete key.
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
- Opening another image asks before discarding rectangles. Canceling the
  picker or choosing an invalid image preserves the current page.

Annotations and history are temporary. Other shapes, projects, and saving are
not implemented yet. Android browser rendering is currently slow; native
touchscreen performance still needs hardware testing.
