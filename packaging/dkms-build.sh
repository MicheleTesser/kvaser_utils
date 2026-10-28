#!/usr/bin/env bash
set -euo pipefail
kernel_version="${1:?Missing target kernel version}"
kernel_dir="${2:-/usr/lib/modules/$kernel_version/build}"
make_args=("KDIR=$kernel_dir" "KV_KERNEL_VERSION=$kernel_version")
# Inspect the target headers: pacman usually builds for a kernel not yet booted.
if grep -q '^CONFIG_CC_IS_CLANG=y' "$kernel_dir/.config"; then
  make_args+=(LLVM=/usr/bin/)
fi
exec make common mhydra "${make_args[@]}"
