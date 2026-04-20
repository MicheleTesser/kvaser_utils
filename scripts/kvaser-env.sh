#!/usr/bin/env bash

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
kvaser_dir="$repo_root/third_party/kvaser"

kvaser_library_path="$kvaser_dir/linuxcan/canlib"
kvaser_library_path="$kvaser_library_path:$kvaser_dir/kvlibsdk/kvadblib/so.ndb"
kvaser_library_path="$kvaser_library_path:$kvaser_dir/kvlibsdk/kvamemolibxml/so.ndb"
kvaser_library_path="$kvaser_library_path:$kvaser_dir/kvlibsdk/kvmlib/memolib_fd/so.ndb"
kvaser_library_path="$kvaser_library_path:$kvaser_dir/kvlibsdk/kvmlib/memolib_wrap/so.ndb"
kvaser_library_path="$kvaser_library_path:$kvaser_dir/kvlibsdk/kvmlib/kvmlib/so.ndb"
kvaser_library_path="$kvaser_library_path:$kvaser_dir/kvlibsdk/kvlclib/so.ndb"

export LD_LIBRARY_PATH="$kvaser_library_path${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export KVASER_UTILS_LD_LIBRARY_PATH="$kvaser_library_path"

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  echo "This script is meant to be sourced:"
  echo "  source scripts/kvaser-env.sh"
  echo
  echo "Current local Kvaser library path:"
  echo "$kvaser_library_path"
fi
