#!/usr/bin/env bash
# CI-only: ship one Linux executable and check it after extraction.
set -euo pipefail

: "${CARGO_TARGET_DIR:?Set CARGO_TARGET_DIR to the build output directory.}"
: "${RUNNER_TEMP:?Set RUNNER_TEMP to the CI temporary directory.}"
: "${GITHUB_SHA:?Set GITHUB_SHA to the source revision.}"
: "${GITHUB_REPOSITORY:?Set GITHUB_REPOSITORY to the repository.}"
: "${GITHUB_RUN_ID:?Set GITHUB_RUN_ID to the workflow run.}"

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
rust_root=$(cd -- "$script_dir/.." && pwd)
repo_root=$(cd -- "$rust_root/.." && pwd)
source="$CARGO_TARGET_DIR/release/fastfill-rust"
archive="$RUNNER_TEMP/FastFill-Rust-linux-x64.tar.gz"
package_dir=$(mktemp -d "$RUNNER_TEMP/fastfill-linux-package.XXXXXX")
smoke_dir=$(mktemp -d "$RUNNER_TEMP/fastfill-linux-smoke.XXXXXX")
trap 'rm -rf -- "$package_dir" "$smoke_dir"' EXIT

mkdir "$package_dir/FastFill-Rust"
package="$package_dir/FastFill-Rust"
install -m 755 "$source" "$package/FastFill-Rust"
cp "$rust_root/LINUX.txt" "$package/README.txt"
cp "$repo_root/LICENSE" "$package/LICENSE.txt"
cp "$rust_root/assets/icons/lucide/LICENSE" \
  "$package/LUCIDE-LICENSE.txt"
cp "$repo_root/docs/samples/fastfill-camera-sample-form.pdf" \
  "$package/sample-form.pdf"
printf 'Commit: %s\nBuild: https://github.com/%s/actions/runs/%s\n' \
  "$GITHUB_SHA" "$GITHUB_REPOSITORY" "$GITHUB_RUN_ID" \
  > "$package/BUILD.txt"
tar -czf "$archive" -C "$package_dir" FastFill-Rust

tar -xzf "$archive" -C "$smoke_dir"
extracted="$smoke_dir/FastFill-Rust"
for file in FastFill-Rust README.txt LICENSE.txt LUCIDE-LICENSE.txt \
  sample-form.pdf BUILD.txt; do
  test -f "$extracted/$file"
done
test -x "$extracted/FastFill-Rust"

# ldd checks linked libraries; GUI drivers and portals can load at runtime.
ldd "$extracted/FastFill-Rust" > "$smoke_dir/linked-libraries.txt"
if grep -q 'not found' "$smoke_dir/linked-libraries.txt"; then
  printf 'Missing linked library:\n' >&2
  sed -n '/not found/p' "$smoke_dir/linked-libraries.txt" >&2
  exit 1
fi

# Invalid helper arguments test extracted-binary startup without a display.
set +e
timeout 10s "$extracted/FastFill-Rust" \
  --fastfill-camera-query invalid > "$smoke_dir/stdout" \
  2> "$smoke_dir/stderr"
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
printf 'Linux extraction, linked libraries and startup check passed.\n'
