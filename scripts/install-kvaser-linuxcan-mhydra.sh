#!/usr/bin/env bash
set -euo pipefail

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  echo "This installs kernel modules and blacklists only the in-kernel Kvaser SocketCAN driver."
  echo "SocketCAN core and other SocketCAN adapter drivers are left untouched."
  exec sudo -E bash "$0" "$@"
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
linuxcan_dir="$repo_root/third_party/kvaser/linuxcan"

"$script_dir/build-kvaser-linuxcan-modules.sh"

if ip link show can0 >/dev/null 2>&1; then
  ip link set can0 down || true
fi

if ip link show can1 >/dev/null 2>&1; then
  ip link set can1 down || true
fi

(
  cd "$linuxcan_dir/common"
  ./installscript.sh
)

(
  cd "$linuxcan_dir/mhydra"
  ./installscript.sh
)

modprobe kvcommon
modprobe mhydra

echo
echo "Installed Kvaser LinuxCAN mhydra driver."
echo "The installer blacklists kvaser_usb only, so SocketCAN still works for non-Kvaser adapters."
echo "If the Memorator was already plugged in, unplug/replug it if channels are not detected immediately."
