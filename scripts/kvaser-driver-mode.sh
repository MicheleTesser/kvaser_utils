#!/usr/bin/env bash
set -euo pipefail

MODE_FILE="/etc/modprobe.d/kvaser-linuxcan-mode.conf"

usage() {
  cat <<'USAGE'
Usage:
  scripts/kvaser-driver-mode.sh linuxcan
  scripts/kvaser-driver-mode.sh socketcan
  scripts/kvaser-driver-mode.sh status

Modes:
  linuxcan   -> KVASER -> LinuxCAN (mhydra)
  socketcan  -> KVASER -> SocketCAN (kvaser_usb)
  status     -> shows current runtime mode
USAGE
}

if [[ "${1:-}" == "" ]]; then
  usage
  exit 1
fi

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  exec sudo -E bash "$0" "$@"
fi

bring_can_down() {
  for iface in can0 can1; do
    if ip link show "$iface" >/dev/null 2>&1; then
      ip link set "$iface" down || true
    fi
  done
}

set_linuxcan_persistent() {
  cat >"$MODE_FILE" <<'CONF'
# Managed by scripts/kvaser-driver-mode.sh
# LinuxCAN mode for Kvaser hardware
blacklist kvaser_usb
CONF
}

clear_linuxcan_persistent() {
  cat >"$MODE_FILE" <<'CONF'
# Managed by scripts/kvaser-driver-mode.sh; overrides the packaged default.
blacklist mhydra
CONF
}

runtime_status() {
  if lsmod | grep -q '^mhydra\b'; then
    echo "runtime: linuxcan (mhydra)"
  elif lsmod | grep -q '^kvaser_usb\b'; then
    echo "runtime: socketcan (kvaser_usb)"
  else
    echo "runtime: unknown (neither mhydra nor kvaser_usb loaded)"
  fi

  if ! grep -qs "^blacklist mhydra$" "$MODE_FILE" && { [[ -f "$MODE_FILE" ]] || [[ -f /usr/lib/modprobe.d/kvaser-linuxcan-mode.conf ]]; }; then
    echo "boot default: linuxcan (kvaser_usb blacklisted by $MODE_FILE)"
  else
    echo "boot default: socketcan (kvaser_usb allowed)"
  fi
}

case "$1" in
  linuxcan)
    bring_can_down
    modprobe -r kvaser_usb || true
    modprobe kvcommon
    modprobe mhydra
    set_linuxcan_persistent
    echo "KVASER -> LinuxCAN (mhydra)"
    echo "If channels do not appear immediately, unplug/replug the Kvaser adapter."
    ;;
  socketcan)
    bring_can_down
    modprobe -r mhydra || true
    modprobe -r kvcommon || true
    clear_linuxcan_persistent
    modprobe can || true
    modprobe can_raw || true
    modprobe can_dev || true
    modprobe kvaser_usb
    echo "KVASER -> SocketCAN (kvaser_usb)"
    echo "If channels do not appear immediately, unplug/replug the Kvaser adapter."
    ;;
  status)
    runtime_status
    ;;
  *)
    usage
    exit 1
    ;;
esac
