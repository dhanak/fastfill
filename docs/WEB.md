# FastFill browser build

This archive contains a complete static site. It needs a web server, without a
compiler or application backend. Projects and exports stay in the browser until
you download them. The browser needs WebGPU or WebGL2.

## Install

Extract into an empty web directory. For example:

```sh
mkdir -p /var/www/html/fastfill
tar -xzf FastFill-web.tar.gz -C /var/www/html/fastfill
```

Serve that directory at `https://example.org/fastfill/`. The archive puts
`index.html`, JavaScript, WASM, and `snippets/` directly in the directory; its
asset URLs are relative, so it also works at a site's root. Keep the trailing
slash when using a subpath.

For nginx, load its usual `mime.types` and ensure `.wasm` is served as
`application/wasm` and `.js`/`.mjs` as JavaScript. Use HTTPS for remote access.
No rewrite or application server is needed for the editor's main page.

## GitHub Pages

The workflow deploys the tested browser archive only after a version tag's
release has been published. Master builds update the development downloads,
without deploying Pages. The `github-pages` environment must allow `v*` tags. If
`dev.vidga.hu` is the account's Pages domain, the project site is at
`https://dev.vidga.hu/fastfill/`. Keep the repository's custom-domain setting
empty so the account domain applies to the project site.

Camera capture needs HTTPS or localhost and browser permission. Use Camera in
the page rail. The floating controls match the desktop camera layout; browser
resolution is automatic. Hold the document steady for automatic capture, or use
manual Capture at any time. Review the photo, then choose Use photo. Automatic
capture suggests its detected frame in the page editor. Open and save projects
with the app's file controls. Use **Rename project** in the project-name menu to
set the filename for future project and PDF downloads. Back up important
`.docscan` files outside the browser.
