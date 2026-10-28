#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"

pkgname="kvaser-utils"
pkgver="$(awk -F'"' '/^version = "/ { print $2; exit }' "$repo_root/crates/kvaser-tui/Cargo.toml")"

if [[ -z "$pkgver" ]]; then
  echo "Cannot determine package version from crates/kvaser-tui/Cargo.toml" >&2
  exit 1
fi

release_root="$pkgname-$pkgver"
dist_dir="$repo_root/dist"
staging_dir="$(mktemp -d)"
trap 'rm -rf "$staging_dir"' EXIT

mkdir -p "$dist_dir" "$staging_dir/$release_root"

copy_into_release() {
  local path="$1"
  cp -R "$repo_root/$path" "$staging_dir/$release_root/$path"
}

mkdir -p \
  "$staging_dir/$release_root/crates" \
  "$staging_dir/$release_root/scripts" \
  "$staging_dir/$release_root/third_party/kvaser"

copy_into_release Cargo.toml
copy_into_release Cargo.lock
copy_into_release PKGBUILD
copy_into_release packaging
copy_into_release scripts/patches
copy_into_release scripts/build-packages.sh
copy_into_release README.md
copy_into_release rust-toolchain.toml
copy_into_release export_csv.py
copy_into_release crates/kvaser-api
copy_into_release crates/kvaser-core
copy_into_release crates/kvaser-tui
copy_into_release scripts/build-kvaser-local.sh
copy_into_release scripts/build-kvaser-linuxcan-modules.sh
copy_into_release scripts/create-release-archive.sh
copy_into_release scripts/disable-kvaser-linuxcan-enable-socketcan.sh
copy_into_release scripts/download-kvaser-sdk.sh
copy_into_release scripts/install-kvaser-linuxcan-mhydra.sh
copy_into_release scripts/kvaser-driver-mode.sh
copy_into_release scripts/kvaser-env.sh
copy_into_release scripts/kvaser_mode
copy_into_release scripts/run-local.sh
copy_into_release scripts/setup-socketcan-tunnel-ifaces.sh
copy_into_release third_party/kvaser/kvlibsdk_5_51_461.tar.gz
copy_into_release third_party/kvaser/linuxcan_5_51_461.tar.gz

archive_path="$dist_dir/$release_root.tar.gz"
tar -C "$staging_dir" -czf "$archive_path" "$release_root"

echo "$archive_path"
