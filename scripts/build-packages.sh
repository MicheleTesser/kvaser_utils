#!/usr/bin/env bash
set -euo pipefail
repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
"$repo_root/scripts/create-release-archive.sh"
cp "$repo_root/PKGBUILD" "$repo_root/dist/PKGBUILD"
mkdir -p "$repo_root/dist/packaging"
cp "$repo_root/packaging/kvaser-utils.install" "$repo_root/dist/packaging/"
cd "$repo_root/dist"
# Pin the generated archive for this build, without modifying the source PKGBUILD.
archive="$(bash -c 'source ./PKGBUILD; printf "%s" "${source[0]}"')"
checksum="$(sha256sum "$archive")"
sed -i "s/sha256sums=('SKIP')/sha256sums=('${checksum%% *}')/" PKGBUILD
exec makepkg "$@"
