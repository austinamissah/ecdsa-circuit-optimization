#!/usr/bin/env bash
# Continuous nonce hunt: screen block after block at a fixed configuration.
#
# `hunt.sh` does one block and exits. This runs until stopped, which is what a
# real search needs. Pair it with `hunt-worker.sh` to confirm survivors in
# parallel while screening continues.
#
# ops.bin and the geometry are built ONCE up front and copied aside. pp_screen
# reads them at block start and holds the transcript prefix in memory, so the
# eval workers may rebuild ./ops.bin in their own directories without
# corrupting a block in flight.
#
# BEFORE RUNNING ONE OF THESE, READ THIS: a clean nonce averages e^lambda draws,
# which on one workstation is days. The leaderboard drifts around 1%/day, so a
# target must be worth more than roughly 5% to survive its own grind. A seven
# hour run on 2026-08-23 covered 6.2% of its search before the frontier moved
# past a -0.038% target. Price the target first; the pipeline is not the
# constraint, the economics are.
#
# Usage: hunt-loop.sh <r1> <r2> <from> [block] [threads] [dir]
set -uo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo"

r1="${1:?usage: hunt-loop.sh <r1> <r2> <from> [block] [threads] [dir]}"
r2="${2:?}"
from="${3:?}"
block="${4:-1000000}"
threads="${5:-12}"
D="${6:-/tmp/pp-hunt}"

mkdir -p "$D"
geom="$D/target.geom.tsv"; ops="$D/target.ops.bin"
surv="$D/survivors.txt";   log="$D/loop.log"

if ! grep -q "mod geom {" src/point_add/pingpong_div.rs; then
  echo "!! builder not instrumented; run: python3 tools/pp-screen/instrument.py" >&2
  exit 1
fi

PP_GEOMETRY="$geom" SUB4_PP_R1="$r1" SUB4_PP_R2="$r2" \
  ./target/release/build_circuit >/dev/null 2>&1
cp ops.bin "$ops"

resolved="$(grep '^#rounds' "$geom" | cut -f2-3 | tr '\t' '/')"
{
  echo "hunt start $(date -u +%FT%TZ)  R1=$r1 R2=$r2"
  echo "  rounds $resolved  md5 $(md5sum "$ops" | cut -d' ' -f1)"
} >> "$log"

while :; do
  s=$(date +%s)
  ./target/release/pp_screen --ops "$ops" --geometry "$geom" \
      --from "$from" --count "$block" --threads "$threads" 2>>"$log" \
    | awk -F'\t' '/^SURVIVOR/{print $2; fflush()}' >> "$surv"
  e=$(date +%s)
  echo "block from=$from done in $((e-s))s  survivors_total=$(wc -l < "$surv")  $(date -u +%FT%TZ)" >> "$log"
  from=$((from + block))
done
