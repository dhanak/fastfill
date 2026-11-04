# FastFill project plan

FastFill is the sole application in this repository. Promotion was authorized on
2026-11-03; remaining Mac/SP6 checks stay explicit and do not postpone the
repository transition. Earlier implementation history remains in Git.

## Accepted implementation

- [x] Image/PDF import, adaptive PDF DPI, image orientation and page reordering.
- [x] Shapes, text, ink, layered/mixed selection, transforms and bounded
  history.
- [x] Font controls, keyboard formatting, snapping and image-anchored
  correction.
- [x] Legacy project loading, atomic saving, PDF export and native recovery.
- [x] Native Windows/Linux camera preview, framing, capture and orientation.
- [x] Browser camera preview, review, framing, automatic capture and downloads.
- [x] Desktop/touch UI polish, animated rail, command palette and shortcuts.
- [x] Browser text/clipboard/font fixes and resize acceptance manually
  confirmed.
- [x] Linux portable app, Windows download/run, legacy project open and SP6
  front/rear camera selection, resolution, framing and mirroring confirmed.
- [x] SP6 sidebar touch taps, annotation gestures and native icon/signature
  fixes.
- [x] Browser archives tested at site root and subpaths, desktop and mobile.

## Primary project transition

- [x] Move application sources, tests, assets, examples and docs to the root.
- [x] Remove the retired application and its framework/build/release files.
- [x] Keep shared image/PDF fixtures, goldens and the sample-form generator.
- [x] Use FastFill naming for the crate, binaries, bundles, archives and UI.
- [x] Make the root README standalone, including Windows certificate trust.
- [x] Use one workflow and publication gate for all four platform archives.
- [x] Publish development builds from master and releases from version tags.
- [x] Deploy GitHub Pages only after successful tagged publication.
- [x] Run the full native suite, strict native/browser Clippy, formatting,
  browser-adapter/release/cache tests and workflow dependency checks.
- [x] Build from the root and verify extracted Linux and browser archives.
- [x] Confirm the renamed development release, `v1.0.0-alpha.1` release and
  tagged Pages deployment. User confirms success after allowing `v*` tags in the
  `github-pages` environment on 2026-11-03.

## Remaining hardware validation

- [ ] SP6 save/export, reopen and inspect the saved/exported output (7A).
- [ ] SP6 touch and Surface Pen at 150% and 200% display scaling.
- [ ] Physical system sleep/resume and camera-permission denial calibration. An
  open preview must stay paused until Resume and require fresh frames.
- [ ] A 20-page project and repeated scans: memory, latency and thermal
  behavior.
- [ ] Real Mac: Gatekeeper approval, launch, file dialogs, open/save/export and
  display scaling. Ad-hoc signing is not Developer ID/notarization.
- [ ] Physical Mac Cmd-key checks; browser-host and automated mappings passed.
- [ ] Visual edge-sliver stroke thinning and thick small Check/Cross marks.

The native camera path already passed laptop and SP6 front/rear checks. Native
macOS camera support remains deferred at the user's request. Browser capture is
separate and already accepted. Timed browser profiling remains deferred unless
visible lag returns.

## Known limits and future work

- Browser image/PDF processing runs on the UI thread; large jobs can pause it.
- Browser recovery and a close-tab unsaved-project guard are not implemented.
- Browser clipboard requires HTTPS/localhost and browser permission.
- Dense ink can be rejected at release; damaged PDF content arrays can lose
  undecodable streams. Both limitations were explicitly accepted.
- Sharp, contrasted backgrounds can mislead automatic framing. Keep manual
  review and adjustment; preserve existing real-device detector calibration.
- Legacy soft-wrap hints load, but text uses explicit newlines and natural
  width. Unmapped font families and affine scaling/shear are rejected
  explicitly.
- PDF export is flattened; keep the editable project. Physical paper sizes
  follow the current 612-point-width convention rather than original PDF size.
- Canvas editing requires a pointing device; screen-reader support is limited.

Do not close hardware checkpoints based only on software tests. Record manual
results here as they arrive. See [development](DEVELOPMENT.md) and the
[manual](MANUAL.md) for reproducible workflows and diagnostics.
