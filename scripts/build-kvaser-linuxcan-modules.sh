#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
linuxcan_dir="$repo_root/third_party/kvaser/linuxcan"

if [[ ! -d "$linuxcan_dir" ]]; then
  kvaser_dir="$repo_root/third_party/kvaser"
  tar -xf "$kvaser_dir"/linuxcan_*.tar.gz -C "$kvaser_dir"
fi

make_args=()
if grep -qi clang /proc/version; then
  make_args+=(CC=clang LLVM=1)
fi

make -C "$linuxcan_dir" common mhydra "${make_args[@]}"

echo "Built LinuxCAN kernel modules:"
echo "  $linuxcan_dir/common/kvcommon.ko"
echo "  $linuxcan_dir/mhydra/mhydra.ko"
