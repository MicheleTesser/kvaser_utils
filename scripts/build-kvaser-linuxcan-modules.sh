#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
linuxcan_dir="$repo_root/third_party/kvaser/linuxcan"
compat_patch="$script_dir/patches/linuxcan-5.51-modern-kernel.patch"

if [[ ! -d "$linuxcan_dir" ]]; then
  kvaser_dir="$repo_root/third_party/kvaser"
  tar -xf "$kvaser_dir"/linuxcan_*.tar.gz -C "$kvaser_dir"
fi

if patch --dry-run --forward --batch --silent -d "$linuxcan_dir" -p1 \
    <"$compat_patch" >/dev/null 2>&1; then
  patch --forward --batch --silent -d "$linuxcan_dir" -p1 <"$compat_patch"
elif ! patch --dry-run --reverse --batch --silent -d "$linuxcan_dir" -p1 \
    <"$compat_patch" >/dev/null 2>&1; then
  echo "Unable to apply LinuxCAN compatibility patch: $compat_patch" >&2
  exit 1
fi

make_args=()
if grep -qi clang /proc/version; then
  if [[ -x /usr/bin/clang ]]; then
    make_args+=(LLVM=/usr/bin/)
  else
    clang_path="$(command -v clang)"
    make_args+=("LLVM=$(dirname -- "$clang_path")/")
  fi
fi

make -C "$linuxcan_dir" common mhydra "${make_args[@]}"

echo "Built LinuxCAN kernel modules:"
echo "  $linuxcan_dir/common/kvcommon.ko"
echo "  $linuxcan_dir/mhydra/mhydra.ko"
