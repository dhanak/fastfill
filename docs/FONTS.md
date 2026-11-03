# Browser fonts

Browser builds embed Liberation Sans, Serif and Mono in regular, bold, italic
and bold italic styles. Native builds keep using installed system fonts. Page
rendering, text measurement, editing and PDF export share these faces.

Unmodified fonts come from the [Liberation Fonts 2.1.5 release][release]. The
downloaded archive is `liberation-fonts-ttf-2.1.5.tar.gz`, SHA-256:

```text
7191c669bf38899f73a2094ed00f7b800553364f90e2637010a69c0e268f25d0
```

The twelve TTF files total 4,359,164 bytes before WASM compression. Their
copyright notices and SIL Open Font License 1.1 are preserved in
[the bundled license](../assets/fonts/license/LICENSE). Trunk copies that
license to `licenses/liberation/LICENSE` in the browser distribution.

[release]: https://github.com/liberationfonts/liberation-fonts/releases/tag/2.1.5
