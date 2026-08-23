#!/usr/bin/env bash
# Long-running nonce hunt at a fixed configuration.
#
# Screens a nonce block with the classical pre-filter and records survivors.
# `pp_screen` reads ops.bin once at startup and holds the transcript prefix in
# memory, so once it is running the file is free and survivors can be verified
# with real builds in the same tree without racing it. Verify separately with
# `confirm.sh` rather than inline, so a slow eval never stalls screening.
#
# Usage: hunt.sh <r1> <r2> <from> <count> [threads] [tag]
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo"

r1="${1:?usage: hunt.sh <r1> <r2> <from> <count> [threads] [tag]}"
r2="${2:?}"
from="${3:?}"
count="${4:?}"
threads="${5:-15}"
tag="${6:-hunt}"

out="${HUNT_DIR:-/tmp/pp-hunt}"
mkdir -p "$out"
geom="$out/$tag.geom.tsv"
log="$out/$tag.log"
surv="$out/$tag.survivors"

if ! grep -q "mod geom {" src/point_add/pingpong_div.rs; then
  echo "!! builder not instrumented; run: python3 tools/pp-screen/instrument.py" >&2
  exit 1
fi

echo "== building ops.bin at R1=$r1 R2=$r2 ==" | tee -a "$log"
PP_GEOMETRY="$geom" SUB4_PP_R1="$r1" SUB4_PP_R2="$r2" \
  ./target/release/build_circuit 2>&1 | grep -E "emitted ops" | tee -a "$log"

# The geometry dump states the depth the builder actually resolved. Screening
# against a different depth than the one ops.bin was built at is the trap this
# whole pipeline exists to avoid, so check rather than assume.
rounds="$(grep '^#rounds' "$geom" | cut -f2-3 | tr '\t' '/')"
echo "   resolved rounds: $rounds" | tee -a "$log"
echo "   ops.bin md5: $(md5sum ops.bin | cut -d' ' -f1)" | tee -a "$log"

echo "== screening $count nonces from $from on $threads threads ==" | tee -a "$log"
echo "   survivors -> $surv" | tee -a "$log"

# Survivors are printed as they are found, so tail the log to watch progress.
exec ./target/release/pp_screen \
  --ops ops.bin --geometry "$geom" \
  --from "$from" --count "$count" --threads "$threads" \
  --out "$surv" 2>&1 | tee -a "$log"
