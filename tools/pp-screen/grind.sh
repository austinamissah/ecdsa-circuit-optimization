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

# The width schedule is read from the builder, never recomputed by the screener:
# it has moved three times (sampled table, greedy table in its own file, then
# the embedded table with a rescale on by default and a repair set off), and a
# screener running a stale table produces confident nonsense. The dump also
# carries the resolved depth and the per-round chunk geometry.
geom="${GEOM:-geom.tsv}"

if ! grep -q "mod geom {" src/point_add/pingpong_div.rs 2>/dev/null; then
  echo "!! src/point_add/pingpong_div.rs is not instrumented." >&2
  echo "   Every sync takes src/point_add from upstream and drops it. Re-apply with:" >&2
  echo "     python3 tools/pp-screen/instrument.py" >&2
  exit 1
fi

echo "== building ops.bin at ROUNDS=$rounds ROUNDS_MUL=$rounds_mul =="
before=""
[ -f ops.bin ] && before="$(md5sum ops.bin | cut -d' ' -f1)"
PP_GEOMETRY="$geom" SUB4_PP_ROUNDS="$rounds" SUB4_PP_ROUNDS_MUL="$rounds_mul" \
  ./target/release/build_circuit 2>&1 | grep -E "emitted ops|OK"
after="$(md5sum ops.bin | cut -d' ' -f1)"
echo "   ops.bin md5 $before -> $after"

# Verify the knobs actually took, by asking the builder what depth it resolved
# rather than inferring it from whether ops.bin moved. The md5 heuristic that
# used to live here compared against a hard-coded "default" config, so it
# false-alarmed the moment upstream changed the default, and would have stayed
# silent for any config that happened to collide. This check is exact and does
# not care what the default is. It is the same class of trap as issue #23:
# benchmark.sh's sudo path drops SUB4_ overrides, so a knob can silently no-op.
got="$(grep '^#rounds' "$geom" | cut -f2-3)"
want="$(printf '%s\t%s' "$rounds" "$rounds_mul")"
if [ "$got" != "$want" ]; then
  echo "!! depth knobs did not apply: asked $rounds/$rounds_mul, builder resolved ${got}" >&2
  echo "   (tab-separated; check SUB4_PP_ROUNDS / SUB4_PP_ROUNDS_MUL reached build_circuit)" >&2
  exit 1
fi

echo "   geometry -> $geom ($(grep -c '^#width' "$geom") rounds, $(grep '^#rounds' "$geom" | cut -f2-3 | tr '\t' '/'))"

args=(--ops ops.bin --geometry "$geom" --from "$from" --count "$count" --threads "$threads"
      --rounds "$rounds" --rounds-mul "$rounds_mul")
[ -n "$out" ] && args+=(--out "$out")

echo "== screening $count nonces from $from =="
exec ./target/release/pp_screen "${args[@]}"
