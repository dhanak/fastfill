#!/usr/bin/env bash
# Publish only checked archives; never move a version tag.
set -euo pipefail

: "${GITHUB_REF:?Set GITHUB_REF to the branch or version tag.}"
: "${GITHUB_SHA:?Set GITHUB_SHA to the source revision.}"
: "${GITHUB_REPOSITORY:?Set GITHUB_REPOSITORY to the repository.}"

case "$GITHUB_REF" in
  refs/heads/master)
    tag=dev
    title="FastFill development build"
    prerelease=true
    latest=false
    ;;
  refs/tags/v[0-9]*)
    tag=${GITHUB_REF#refs/tags/}
    version=$(awk -F '"' '/^version[[:space:]]*=/ {print $2; exit}' Cargo.toml)
    if test "${tag#v}" != "$version"; then
      printf '::error::Tag %s does not match Cargo version %s.\n' \
        "$tag" "$version" >&2
      exit 1
    fi
    title="FastFill $version"
    prerelease=false
    latest=true
    if [[ "${version%%+*}" == *-* ]]; then
      prerelease=true
      latest=false
    fi
    ;;
  *)
    printf '::error::Only master and version tags publish releases.\n' >&2
    exit 1
    ;;
esac

archives=(FastFill-windows-x64.zip FastFill-linux-x64.tar.gz
  FastFill-macos-universal.zip FastFill-web.tar.gz)
for archive in "${archives[@]}"; do
  test -s "$archive" || {
    printf '::error::Missing or empty archive: %s\n' "$archive" >&2
    exit 1
  }
done

if gh release view "$tag" >/dev/null 2>&1; then
  gh release upload "$tag" "${archives[@]}" --clobber
  if test "$tag" = dev; then
    # Advance the development pointer only after every upload succeeds.
    gh api --method PATCH \
      "repos/${GITHUB_REPOSITORY}/git/refs/tags/dev" \
      -f "sha=${GITHUB_SHA}" -F force=true >/dev/null
    gh release edit dev --prerelease --latest=false \
      --title "$title" --notes "Latest successful master build: ${GITHUB_SHA}."
  else
    gh release edit "$tag" --draft=false --title "$title" \
      "--prerelease=$prerelease" "--latest=$latest"
  fi
else
  flags=(--title "$title" "--prerelease=$prerelease" "--latest=$latest")
  if test "$tag" = dev; then
    flags+=(--target "$GITHUB_SHA"
      --notes "Latest successful master build: ${GITHUB_SHA}.")
  else
    flags+=(--verify-tag --generate-notes)
  fi
  gh release create "$tag" "${archives[@]}" "${flags[@]}"
fi
