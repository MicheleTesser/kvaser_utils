#!/usr/bin/env bash
set -euo pipefail

dest="${1:-third_party/kvaser}"
mkdir -p "$dest"

wget --content-disposition \
  "https://www.kvaser.com/downloads-kvaser/?utm_source=software&utm_ean=7330130980754&utm_status=latest" \
  -P "$dest"

wget --content-disposition \
  "https://www.kvaser.com/downloads-kvaser/?utm_source=software&utm_ean=7330130981966&utm_status=latest" \
  -P "$dest"

echo "Kvaser SDK archives downloaded in $dest"
