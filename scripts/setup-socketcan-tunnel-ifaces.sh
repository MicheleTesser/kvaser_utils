#!/usr/bin/env bash
set -euo pipefail

if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  echo "This creates can0/can1 as vcan interfaces for live LinuxCAN -> SocketCAN tunnel."
  exec sudo -E bash "$0" "$@"
fi

ensure_vcan_iface() {
  local iface="$1"

  if ip link show "$iface" >/dev/null 2>&1; then
    local kind
    kind="$(ip -d link show "$iface" | sed -n 's/^[[:space:]]*link\/\([^[:space:]]*\).*/\1/p' | head -n1)"
    if [[ "$kind" != "can" ]]; then
      echo "Interface $iface exists and is not CAN/vcan (kind=$kind). Leaving it unchanged."
      return 0
    fi
    ip link set "$iface" up
    echo "Interface $iface already exists; set UP."
    return 0
  fi

  ip link add dev "$iface" type vcan
  ip link set "$iface" up
  echo "Created $iface (vcan) and set UP."
}

modprobe vcan
ensure_vcan_iface can0
ensure_vcan_iface can1

echo
echo "Ready: can0 and can1 are available for SocketCAN tools (candump, cansend, etc.)."
