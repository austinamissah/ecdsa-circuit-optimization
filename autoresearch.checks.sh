#!/bin/bash
set -euo pipefail

tmp=$(mktemp)
cleanup() {
  rm -f "$tmp"
}
trap cleanup EXIT

cargo run --release -- --note "autoresearch-checks" >"$tmp" 2>&1 || {
  tail -80 "$tmp"
  exit 1
}
