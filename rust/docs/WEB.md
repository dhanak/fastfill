# FastFill browser build

This archive contains a complete static site. It needs a web server, not a Rust
toolchain or application backend. Projects and exports stay in the browser until
you download them.

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

The browser build has no camera capture. Open and save projects with the app's
file controls. Back up important `.docscan` files outside the browser.
