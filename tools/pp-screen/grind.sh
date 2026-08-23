#!/usr/bin/env bash
# Screen a nonce block at one walk depth.
#
# THE TRAP THIS EXISTS TO CLOSE: the Fiat-Shamir seed is a hash of the whole op
# stream, and changing the walk depth changes the op count (12,912,890 at
# 698/696 against 12,890,758 at 696/694). So ops.bin must be rebuilt for the
# exact config being screened, or every seed the screener derives belongs to a
# different circuit and every result is noise that looks like data. The tail
# nonce itself only rewrites q_target on 96 existing ops, so one build per
# config covers the whole block.
#
# build_circuit is run directly, never through benchmark.sh: that script prefers
# the `sudo -n bwrap` path whose env_reset drops SUB4_ overrides, so the depth
# knobs would be silently ignored and every arm would measure the default.
#
# Usage: grind.sh <rounds> <rounds-mul> <from> <count> [threads] [out]
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo"

rounds="${1:?usage: grind.sh <rounds> <rounds-mul> <from> <count> [threads] [out]}"
rounds_mul="${2:?}"
from="${3:?}"
count="${4:?}"
threads="${5:-15}"
out="${6:-}"

echo "== building ops.bin at ROUNDS=$rounds ROUNDS_MUL=$rounds_mul =="
before=""
[ -f ops.bin ] && before="$(md5sum ops.bin | cut -d' ' -f1)"
SUB4_PP_ROUNDS="$rounds" SUB4_PP_ROUNDS_MUL="$rounds_mul" \
  ./target/release/build_circuit 2>&1 | grep -E "emitted ops|OK"
after="$(md5sum ops.bin | cut -d' ' -f1)"
echo "   ops.bin md5 $before -> $after"
if [ -n "$before" ] && [ "$before" = "$after" ] && [ "$rounds/$rounds_mul" != "698/696" ]; then
  echo "!! ops.bin did not move; the depth knobs were not applied" >&2
  exit 1
fi

args=(--ops ops.bin --from "$from" --count "$count" --threads "$threads"
      --rounds "$rounds" --rounds-mul "$rounds_mul")
[ -n "$out" ] && args+=(--out "$out")

echo "== screening $count nonces from $from =="
exec ./target/release/pp_screen "${args[@]}"
