#!/usr/bin/env bash
# CI-only: package one universal, ad-hoc-signed macOS app and check it.
set -euo pipefail

: "${CARGO_TARGET_DIR:?Set CARGO_TARGET_DIR to the build output directory.}"
: "${RUNNER_TEMP:?Set RUNNER_TEMP to the CI temporary directory.}"
: "${GITHUB_SHA:?Set GITHUB_SHA to the source revision.}"
: "${GITHUB_REPOSITORY:?Set GITHUB_REPOSITORY to the repository.}"
: "${GITHUB_RUN_ID:?Set GITHUB_RUN_ID to the workflow run.}"

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
rust_root=$(cd -- "$script_dir/.." && pwd)
repo_root=$(cd -- "$rust_root/.." && pwd)
archive="$RUNNER_TEMP/FastFill-Rust-macos-universal.zip"
package_dir=$(mktemp -d "$RUNNER_TEMP/fastfill-macos-package.XXXXXX")
smoke_dir=$(mktemp -d "$RUNNER_TEMP/fastfill-macos-smoke.XXXXXX")
trap 'rm -rf -- "$package_dir" "$smoke_dir"' EXIT

package="$package_dir/FastFill-Rust"
app="$package/FastFill-Rust.app"
binary="$app/Contents/MacOS/FastFill-Rust"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$rust_root/assets/macos/Info.plist" "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"

x64="$CARGO_TARGET_DIR/x86_64-apple-darwin/release/fastfill-rust"
arm64="$CARGO_TARGET_DIR/aarch64-apple-darwin/release/fastfill-rust"
test -x "$x64"
test -x "$arm64"
lipo -create -output "$binary" "$x64" "$arm64"
lipo "$binary" -verify_arch x86_64 arm64
chmod 755 "$binary"

iconset="$package_dir/AppIcon.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$rust_root/assets/AppIcon.png" \
    --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$rust_root/assets/AppIcon.png" \
    --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns -o "$app/Contents/Resources/AppIcon.icns" "$iconset"

cp "$rust_root/docs/MACOS.txt" "$package/README.txt"
cp "$repo_root/LICENSE" "$package/LICENSE.txt"
cp "$rust_root/assets/icons/lucide/LICENSE" \
  "$package/LUCIDE-LICENSE.txt"
cp "$repo_root/docs/samples/fastfill-camera-sample-form.pdf" \
  "$package/sample-form.pdf"
printf 'Commit: %s\nBuild: https://github.com/%s/actions/runs/%s\n' \
  "$GITHUB_SHA" "$GITHUB_REPOSITORY" "$GITHUB_RUN_ID" \
  > "$package/BUILD.txt"

# Sign after merging architectures and adding bundle resources.
codesign --force --sign - --timestamp=none "$app"
codesign --verify --deep --strict --verbose=2 "$app"
ditto -c -k --sequesterRsrc --keepParent "$package" "$archive"

ditto -x -k "$archive" "$smoke_dir"
extracted="$smoke_dir/FastFill-Rust"
for file in README.txt LICENSE.txt LUCIDE-LICENSE.txt \
  sample-form.pdf BUILD.txt; do
  test -f "$extracted/$file"
done
test -x "$extracted/FastFill-Rust.app/Contents/MacOS/FastFill-Rust"
test -s "$extracted/FastFill-Rust.app/Contents/Resources/AppIcon.icns"
plutil -lint "$extracted/FastFill-Rust.app/Contents/Info.plist"
codesign --verify --deep --strict --verbose=2 \
  "$extracted/FastFill-Rust.app"
signature=$(codesign --display --verbose=2 \
  "$extracted/FastFill-Rust.app" 2>&1)
if ! grep -q '^Signature=adhoc$' <<< "$signature"; then
  printf 'Extracted app is not ad-hoc signed.\n' >&2
  exit 1
fi
executable="$extracted/FastFill-Rust.app/Contents/MacOS/FastFill-Rust"
lipo "$executable" -verify_arch x86_64 arm64

# Invalid helper arguments check startup without opening a GUI or camera.
set +e
"$executable" --fastfill-camera-query invalid \
  > "$smoke_dir/stdout" 2> "$smoke_dir/stderr"
status=$?
set -e
if test "$status" -ne 1; then
  printf 'Invalid camera query exited with %s instead of 1.\n' \
    "$status" >&2
  exit 1
fi
if ! grep -q 'Camera worker: Expected list or modes INDEX' \
  "$smoke_dir/stderr"; then
  printf 'Extracted executable failed its startup check.\n' >&2
  sed -n '1,20p' "$smoke_dir/stderr" >&2
  exit 1
fi
printf 'macOS universal extraction, signature and startup passed.\n'
