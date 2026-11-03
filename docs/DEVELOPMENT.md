# FastFill development guide

[← README](../README.md) · [User manual](MANUAL.md) · [Project plan](PLAN.md)

Run commands from the repository root. The application, library, tests and
packaging scripts belong to one Cargo package, `fastfill`.

## Setup and routine checks

Install the tools configured in `mise.toml`, then enable repository hooks:

```sh
mise install
git config core.hooksPath .githooks
mise exec -- rustup target add wasm32-unknown-unknown
```

Rust 1.99.0 is pinned. Node.js 18+ runs dependency-free browser and release
tests. The pre-push hook rejects unsigned commits; repository owners sign their
commits before pushing.

```sh
make check
mise exec -- cargo check --target wasm32-unknown-unknown
mise exec -- osv-scanner scan source .
```

`make check` runs formatting, strict Clippy, all-feature tests and the Node
adapter/release checks. `make check-core` runs library checks without the GUI or
camera dependencies. `make check-container` runs the same core checks in the
pinned compiler container. `make fmt` uses rustfmt and mdformat.

Agent builds must follow [build isolation](../AGENTS.md). Do not share an agent
target directory with the host developer or run `cargo clean` as a routine fix.
Developer dependency caches stay in `target/`; WASM output goes to `dist/`. Both
are ignored by Git and excluded from the Docker context.

## Run locally

```sh
mise exec -- cargo run
mise exec -- cargo run -- path/to/project.docscan
mise exec -- cargo run --release
```

Debug builds optimize application pixel loops at level 1 and dependencies at
level 2, keeping assertions and debug symbols. Assess performance with a release
build. Native startup accepts one `.docscan` argument and keeps any deferred
recovery separate from the explicitly opened file.

For a browser development server on LAN port 24800:

```sh
mise run web
```

Refresh the browser after a rebuild; automatic reload is disabled for remote
devices. The task sets `CARGO_INCREMENTAL=0` to avoid stale WASM objects while
retaining compiled dependencies. For a deployable build:

```sh
mise exec -- trunk build --release --locked --all-features \
  --public-url ./ --dist /tmp/fastfill-web-dist
```

Relative URLs support both site-root and subpath hosting. See [WEB.md](WEB.md)
for nginx and GitHub Pages. Camera and clipboard access require HTTPS or
localhost. Camera analysis, PDF rasterization and page correction run on the
browser UI thread; their native equivalents use background workers. Timed
browser profiling is deferred unless noticeable lag returns.

## Source layout and project limits

- `src/annotations/`: geometry, selection, editing, formatting and history.
- `src/app/`: controller, UI, page versions, file workflows and recovery.
- `src/camera/`: supervised native capture and device catalogs.
- `src/project/`: bounded archive I/O and legacy schema conversion.
- `src/page_canvas/`: common display/PDF drawing and pointer routing.
- `web/`: browser camera, clipboard and download adapters.
- `tests/samples/`, `tests/goldens/`, `docs/samples/`: shared fixtures.

Projects allow 20 pages, 50 MP per source image and 200 MP each across source
and corrected page images. Admission reserves the largest display size in each
page's retained history. Encoded images are limited to 100 MiB each; images plus
metadata to 600 MiB. Compact JSON manifests have a 5 MiB budget. Edits and
imports are checked before commit, including normalized image bytes and retained
undo/redo metadata capacity.

Each page allows 10,000 annotations; each text box 10,000 UTF-16 units,
including paste; each ink stroke 100,000 samples. Each page keeps a 64 MiB
undo/redo budget and drops oldest states first. These limits bound retained
project data, not every renderer allocation or temporary worker buffer.

Schema-v1 documents still load. Brightness uses schema-v2; unchanged legacy
annotation metadata survives a save. Unsupported font families and affine
shear/scaling are rejected rather than silently distorted. Soft-wrap fitting
hints load, but text editing uses natural width and explicit newlines.

Native recovery uses the existing per-user FastFill paths on each platform. Set
`FASTFILL_RECOVERY_DIR` to an absolute test directory to isolate recovery tests.
Browser recovery is not implemented; save downloaded copies explicitly. See the
user manual for editing, cancellation and privacy behavior.

## Diagnostics and fixtures

Keep original captures under `artifacts/reviews/`; they are local data, not
source-controlled build output. Choose fresh output filenames for diagnostics.
The examples refuse existing outputs and do not modify their inputs.

```sh
mkdir -p artifacts/previews
mise exec -- cargo run --no-default-features --example correct_page -- \
  tests/samples/sample-form.png artifacts/previews/corrected.png \
  examples/correction.json
mise exec -- cargo run --no-default-features --example form_probe -- \
  tests/samples/sample-form.png artifacts/previews/form-candidates.png
mise exec -- cargo run --no-default-features --example frame_probe -- \
  photo.jpg artifacts/previews/frame.png
mise exec -- cargo run --no-default-features --example frame_replay -- \
  artifacts/reviews/cam/frames artifacts/previews/replay
mise exec -- cargo run --no-default-features --example project_timing -- \
  form.docscan
```

Correction settings use normalized convex crop corners, quarter-turn rotation,
filter modes 0–3 (Original, Enhanced color, Grayscale, B&W), and brightness/
contrast from −100 to +100. See `examples/filter.json` and
`examples/brightness.json`. Geometry precedes filtering. Brightness and contrast
share one clamped lookup after filtering; originals stay intact.

Form diagnostics mark checkboxes and solid/dotted/dashed rules, including
individual table cells and full rows. They are not OCR. Framing diagnostics show
accepted and rejected candidates with normalized corners and heuristic scores.
Sharp background edges can mislead framing; manual review remains necessary.
Camera analysis uses a 640-pixel edge, detailed framing 1280 pixels, and
form/corner analysis 1600 pixels. Keep real-hardware calibration intact.

The replay viewer opens directly from its generated `index.html`. Normal tests
skip the expensive 224-frame recording; run it explicitly when needed:

```sh
mise exec -- cargo test --no-default-features --test document_detection \
  recorded_laptop_sequence -- --ignored --nocapture
```

Regenerate the printable sample form with `make sample-form`. The Python
generator uses `uv` script metadata; it needs no workspace virtual environment.
`node tools/source-size.mjs` reports indicative production source size.

## Native camera diagnostics

With other camera apps closed:

```sh
mise exec -- cargo run --no-default-features --features native-camera \
  --example camera_probe -- list
mise exec -- cargo run --no-default-features --features native-camera \
  --example camera_probe -- formats 0
mise exec -- cargo run --no-default-features --features native-camera \
  --example camera_probe -- capture 0 camera-test.png
```

The default probe request is exactly 640×480, 30 FPS, MJPG. Supply an advertised
mode as `WIDTH HEIGHT FPS ENCODING` after the output filename when needed.
Supported encodings: MJPG, YUYV, NV12, RGB3, BGR3, GREY. The probe allows a
two-second exposure warm-up; the live UI has no forced capture delay. Requests
run in supervised child processes. No automated test opens a camera.

Linux uses V4L2 through rscam without libv4l2 or Clang. Windows uses Media
Foundation. macOS capture is deferred; browser capture uses browser APIs. For
SP6 touch diagnostics, launch the app from PowerShell with an opt-in log:

```powershell
$env:FASTFILL_TOUCH_LOG = Join-Path $env:TEMP 'FastFill-touch.log'
.\FastFill.exe
```

The log records pointer routing, not typed text. Remove the variable afterward.

## CI, signing and releases

The [FastFill workflow](../.github/workflows/build.yml) runs from the root.

| Event                       | Publication                         | GitHub Pages      |
| --------------------------- | ----------------------------------- | ----------------- |
| Pull request / other branch | Checked Actions artifacts           | No                |
| Successful `master` build   | Update the shared `dev` prerelease  | No                |
| Version tag `v…`            | Publish all four versioned archives | After publication |

Windows/Linux native tests and all four platform packages gate publication.
macOS tests on Intel, cross-builds both architectures and ad-hoc signs the
universal app. Windows release builds statically link the C runtime and are
signed/timestamped on `master` and version tags. Browser archives contain a
complete static site. Packages are extracted and smoke-tested in CI; these
checks do not replace the pending physical Mac/SP6 tests in [PLAN.md](PLAN.md).

The Windows signing secrets remain `FASTFILL_CERTIFICATE_BASE64` and
`FASTFILL_CERTIFICATE_PASSWORD`. The public signer certificate is exported as
`FastFill.cer` and included in signed ZIPs. No certificate rotation is needed.
See the [README](../README.md) for certificate trust instructions. Never commit
private signing material. macOS uses ad-hoc signing, not Developer ID or
notarization.

Development releases advance the `dev` tag only after all uploads succeed.
Version tags never move. Rerunning a version workflow replaces that version's
assets. Versions with a prerelease suffix do not become GitHub's latest release;
stable versions do. Tag versions must match the package version in `Cargo.toml`.

To release, update `Cargo.toml` and its package entry in `Cargo.lock`, commit
and sign the change, then create and push the matching signed tag. For example,
with the current package version:

```sh
git tag -s v1.0.0-alpha.1 -m "FastFill 1.0.0-alpha.1"
git push origin v1.0.0-alpha.1
```

GitHub Pages uses the existing `github-pages` environment and account domain.
Its environment rules must allow version tags (`v*`), not only the master
branch. Keep Pages configured for GitHub Actions and leave the repository's
custom-domain override empty when inheriting the account domain. Update branch
protection if required checks still name a retired workflow.

After the first root-layout build, update any local launch shortcuts or file
associations to the new executable path. Existing certificate trust and native
recovery paths remain valid. Old host build output is deliberately untouched:
move the previous application target directory to root `target/` only if that
destination does not exist, and move any local mise overrides separately. Remove
stale browser output when it is no longer needed.

After verifying the replacement downloads, remove the retired development
prerelease and obsolete portable assets from the shared `dev` release if
desired. Historical version releases can stay available.

Release automation and cache tests require no GitHub access:

```sh
node tools/publish-release.test.mjs
node tools/cache.test.mjs
```

Caches include crate downloads and compiled outputs per platform. Keys include
the compiler version and manifest/lockfile hashes. Compatible restore prefixes
allow dependency reuse, and tests always run after a cache hit. Source moves or
toolchain changes can require rebuilding. CI omits debug symbols and incremental
state from caches; it keeps optimization and test assertions.
