#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"

# shellcheck source=scripts/kvaser-env.sh
source "$script_dir/kvaser-env.sh"

cd "$repo_root"
if [[ "$#" -eq 0 ]]; then
  set -- tui
fi

cargo run -p kvaser-tui -- "$@"
