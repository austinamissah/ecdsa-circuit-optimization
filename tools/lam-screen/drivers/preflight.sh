#!/usr/bin/env bash
# Build + full-harness eval for each (ITERS:strip) config given as an argument.
#
# eval_circuit exits before printing metrics when a shot mismatches, but it
# appends avgT / qubits / ops to results.tsv on the FAIL path too, so the price
# of a lever is readable even when the shipped nonce is not clean under it.
# md5 ops.bin is recorded because a null result is only a result if it moved
# (memory/04-traps.md section 1).
set -uo pipefail
cd "$(dirname "$0")"
ROOT=$PWD; TREE=$ROOT/base; OUT=$ROOT/iters
mkdir -p "$OUT"
[ -f "$OUT/preflight.tsv" ] || echo -e "iters\tstrip\tops\tqubits\tavgT\tscore\tclassical\tphase\tstale_keys\tmd5" > "$OUT/preflight.tsv"

for spec in "$@"; do
  IT=${spec%%:*}; STRIP=${spec##*:}
  echo "=== ITERS=$IT strip=$STRIP  $(date +%H:%M:%S) ==="
  sed -i "s/^pub const ITERS: usize = .*/pub const ITERS: usize = $IT;/" \
      "$TREE/src/point_add/trailmix_ludicrous/schedule.rs"
  ( cd "$TREE" && cargo build --release --locked --offline \
      --bin lamscreen --bin build_circuit --bin eval_circuit 2>&1 | grep -E "^error" -A5 | head -20 )
  ( cd "$TREE" && rm -f ops.bin score.json
    SUB4_APPLY_STRIP=$STRIP ./target/release/build_circuit > "$OUT/b_${IT}_$STRIP.log" 2>&1
    SUB4_APPLY_STRIP=$STRIP ./target/release/eval_circuit  > "$OUT/e_${IT}_$STRIP.log" 2>&1 )
  MD5=$(md5sum "$TREE/ops.bin" | cut -d' ' -f1)
  CM=$(grep -oP 'classical mismatches\s*:\s*\K\d+' "$OUT/e_${IT}_$STRIP.log")
  PG=$(grep -oP 'phase-garbage batches\s*:\s*\K\d+' "$OUT/e_${IT}_$STRIP.log")
  STALE=$(grep -oP 'stale keys skipped' -c "$OUT/b_${IT}_$STRIP.log" >/dev/null && grep -oP ';\s*\K\d+(?= stale keys skipped)' "$OUT/b_${IT}_$STRIP.log" || echo 0)
  read -r AVGT QB OPS < <(tail -1 "$TREE/results.tsv" | awk -F'\t' '{print $3, $5, $6}')
  SC=$(python3 -c "print(round(float('$AVGT'))*int('$QB'))")
  echo -e "$IT\t$STRIP\t$OPS\t$QB\t$AVGT\t$SC\t$CM\t$PG\t$STALE\t$MD5" >> "$OUT/preflight.tsv"
  echo "  ops=$OPS qubits=$QB avgT=$AVGT score=$SC classical=$CM phase=$PG stale=$STALE md5=$MD5"
done
echo; column -t -s$'\t' "$OUT/preflight.tsv"
