#!/usr/bin/env bash
set -euo pipefail

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  echo "This disables Kvaser LinuxCAN (mhydra) and re-enables kvaser_usb SocketCAN."
  exec sudo -E bash "$0" "$@"
fi

for iface in can0 can1; do
  if ip link show "$iface" >/dev/null 2>&1; then
    ip link set "$iface" down || true
  fi
done

modprobe -r mhydra || true
modprobe -r kvcommon || true

changed=0
for conf in /etc/modprobe.d/*.conf; do
  [[ -e "$conf" ]] || continue
  if grep -Eq '^[[:space:]]*blacklist[[:space:]]+kvaser_usb([[:space:]]|$)' "$conf"; then
    sed -i -E '/^[[:space:]]*blacklist[[:space:]]+kvaser_usb([[:space:]]|$)/d' "$conf"
    changed=1
  fi
done

if [[ "$changed" -eq 1 ]]; then
  echo "Removed kvaser_usb blacklist lines from /etc/modprobe.d/*.conf"
fi

modprobe can || true
modprobe can_raw || true
modprobe can_dev || true
modprobe kvaser_usb || true

echo
echo "LinuxCAN mhydra disabled; kvaser_usb requested."
echo "If can0/can1 do not appear immediately, unplug/replug the Kvaser adapter."
