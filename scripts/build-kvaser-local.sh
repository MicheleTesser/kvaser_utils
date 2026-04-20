#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
kvaser_dir="$repo_root/third_party/kvaser"

if [[ ! -d "$kvaser_dir/linuxcan" ]]; then
  tar -xf "$kvaser_dir"/linuxcan_*.tar.gz -C "$kvaser_dir"
fi

if [[ ! -d "$kvaser_dir/kvlibsdk" ]]; then
  tar -xf "$kvaser_dir"/kvlibsdk_*.tar.gz -C "$kvaser_dir"
fi

make -C "$kvaser_dir/linuxcan" canlib
make -C "$kvaser_dir/kvlibsdk" kvamemolibxml XTRA_CFLAGS="-Wno-error=overloaded-virtual -Wno-error=deprecated-declarations"
make -C "$kvaser_dir/kvlibsdk" kvmlib

# Kvaser SDK 5.51.461 trips Clang's header-guard warning in kvlclib while the
# upstream Makefile uses -Werror. Keep the source untouched and only relax that
# specific warning for the local build.
make -C "$kvaser_dir/kvlibsdk" kvlclib XTRA_CFLAGS=-Wno-error=header-guard

echo "Local Kvaser user-space libraries built."
echo "Use scripts/run-local.sh <command> to run with LD_LIBRARY_PATH configured."
