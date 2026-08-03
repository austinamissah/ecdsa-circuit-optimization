#!/usr/bin/env bash
# lambda_classical for each (ITERS:strip) config, over a fixed 400-nonce set.
#
# ITERS is a compile-time const (schedule.rs:4), so each value needs its own
# build; the tree is rebuilt in place, one config at a time. Note the nonce set
# being shared across configs buys NO variance reduction: the Fiat-Shamir seed
# is a hash of the whole op stream, so changing ITERS re-rolls all 9,024 test
# inputs at every nonce. It is a fixed set for protocol hygiene, not a pairing.
set -uo pipefail
cd "$(dirname "$0")"
ROOT=$PWD; TREE=$ROOT/base; OUT=$ROOT/iters
NONCES=${NONCES:-$ROOT/sweep_nonces.txt}
NWORK=${NWORK:-10}; LANES=${LANES:-16}
mkdir -p "$OUT"

for spec in "$@"; do
  IT=${spec%%:*}; STRIP=${spec##*:}
  echo "=== lambda ITERS=$IT strip=$STRIP  $(date +%H:%M:%S) ==="
  sed -i "s/^pub const ITERS: usize = .*/pub const ITERS: usize = $IT;/" \
      "$TREE/src/point_add/trailmix_ludicrous/schedule.rs"
  ( cd "$TREE" && cargo build --release --locked --offline --bin lamscreen 2>&1 \
      | grep -E "^error" -A5 | head -20 )
  rm -f "$OUT"/L_${IT}_${STRIP}_*.tsv
  for w in $(seq 0 $((NWORK-1))); do
    awk -v w=$w -v k=$NWORK 'NR%k==w' "$NONCES" > "$OUT/nn_${IT}_${STRIP}_$w.txt"
  done
  T0=$(date +%s)
  for w in $(seq 0 $((NWORK-1))); do
    SUB4_APPLY_STRIP=$STRIP "$TREE/target/release/lamscreen" \
      --nonces "$OUT/nn_${IT}_${STRIP}_$w.txt" --mode count --lanes "$LANES" \
      --tag "i${IT}s${STRIP}" --out "$OUT/L_${IT}_${STRIP}_$w.tsv" 2>/dev/null &
  done
  wait
  T1=$(date +%s)
  cat "$OUT"/L_${IT}_${STRIP}_*.tsv | awk -F'\t' -v it=$IT -v st=$STRIP '
    FNR>1 {s+=$3; ss+=$3*$3; n++; fp[$9]=1}
    END {m=s/n; sd=sqrt((ss-n*m*m)/(n-1));
         printf "  ITERS=%s strip=%s  n=%d  mean=%.3f  sd=%.3f  sem=%.3f  distinct_fp=%d\n",
                it, st, n, m, sd, sd/sqrt(n), length(fp)}'
  echo "  wall $((T1-T0)) s"
done
echo "=== lambda sweep done $(date +%H:%M:%S) ==="
