# FastFill browser build

This archive contains a complete static site. It needs a web server, not a Rust
toolchain or application backend. Projects and exports stay in the browser until
you download them. The browser needs WebGPU or WebGL2.

## Install

Extract into an empty web directory. For example:

```sh
mkdir -p /var/www/html/fastfill
tar -xzf FastFill-Rust-web.tar.gz -C /var/www/html/fastfill
```

Serve that directory at `https://example.org/fastfill/`. The archive puts
`index.html`, JavaScript, WASM, and `snippets/` directly in the directory; its
asset URLs are relative, so it also works at a site's root. Keep the trailing
slash when using a subpath.

For nginx, load its usual `mime.types` and ensure `.wasm` is served as
`application/wasm` and `.js`/`.mjs` as JavaScript. Use HTTPS for remote access.
No rewrite or application server is needed for the editor's main page.

## GitHub Pages

The `master` workflow deploys the tested browser archive to GitHub Pages after
native tests and the browser package check pass. If `dev.vidga.hu` is the
account's Pages domain, the project site is at
`https://dev.vidga.hu/fastfill/`. Keep the repository's custom-domain setting
empty so the account domain applies to the project site.

The browser build has no camera capture. Open and save projects with the app's
file controls. Back up important `.docscan` files outside the browser.
