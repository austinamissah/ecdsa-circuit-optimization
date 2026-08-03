#!/usr/bin/env bash
# lambda_TOTAL at TLM_SCHED_J2_DELTA=2, via the real harness.
#
# The screen is classical-channel only. The whole feasibility question now turns
# on whether the phase channel moved with it, and only eval_circuit can answer
# that. So: full ./benchmark.sh-equivalent runs (build_circuit + eval_circuit),
# no screen, over N nonces, recording classical AND phase per nonce.
#
# Worker trees are separate because eval_circuit writes results.tsv to the
# CARGO_MANIFEST_DIR baked in at compile time -- workers sharing a build would
# all append to one file (docs/lambda-measurement.md, "Reproducing").
set -uo pipefail
cd "$(dirname "$0")"
ROOT=$PWD; SRC=$ROOT/base; OUT=$ROOT/harness
NW=${NW:-6}; N=${N:-42}; DELTA=${DELTA:-2}
mkdir -p "$OUT"

# nonce list: the first N of the shared sweep set, so it overlaps the screen arms
head -n "$N" "$ROOT/sweep_nonces.txt" > "$OUT/nonces.txt"

if [ ! -d "$OUT/w0" ]; then
  echo "--- creating $NW worker trees $(date +%H:%M:%S) ---"
  for w in $(seq 0 $((NW-1))); do
    rm -rf "$OUT/w$w"; mkdir -p "$OUT/w$w"
    ( cd "$SRC" && tar -c --exclude=target --exclude=ops.bin . ) | tar -x -C "$OUT/w$w"
    cp -r "$SRC/target" "$OUT/w$w/target"
    ( cd "$OUT/w$w" && touch src/bin/build_circuit.rs src/bin/eval_circuit.rs \
        && cargo build --release --locked --offline --bin build_circuit --bin eval_circuit 2>&1 | tail -1 )
  done
fi

run_worker() {
  local w=$1
  local tree="$OUT/w$w"
  awk -v w="$w" -v k="$NW" 'NR%k==w' "$OUT/nonces.txt" | while read -r nonce; do
    ( cd "$tree" && rm -f ops.bin score.json
      SUB4_APPLY_STRIP=0 TLM_SCHED_J2_DELTA=$DELTA SUB4_TAIL_NONCE=$nonce \
        ./target/release/build_circuit > /dev/null 2>&1
      SUB4_APPLY_STRIP=0 TLM_SCHED_J2_DELTA=$DELTA SUB4_TAIL_NONCE=$nonce \
        ./target/release/eval_circuit > "$OUT/e_${w}_${nonce}.log" 2>&1
      MD5=$(md5sum ops.bin | cut -d' ' -f1)
      CM=$(grep -oP 'classical mismatches\s*:\s*\K\d+' "$OUT/e_${w}_${nonce}.log")
      PG=$(grep -oP 'phase-garbage batches\s*:\s*\K\d+' "$OUT/e_${w}_${nonce}.log")
      AG=$(grep -oP 'ancilla-garbage batches\s*:\s*\K\d+' "$OUT/e_${w}_${nonce}.log")
      echo -e "$nonce\t${CM:-NA}\t${PG:-NA}\t${AG:-NA}\t$MD5" >> "$OUT/results_$w.tsv" )
  done
}

echo "--- harness lambda, delta=$DELTA, N=$N, $NW workers $(date +%H:%M:%S) ---"
rm -f "$OUT"/results_*.tsv
for w in $(seq 0 $((NW-1))); do run_worker "$w" & done
wait
cat "$OUT"/results_*.tsv | awk -F'\t' '
  {c+=$2; cc+=$2*$2; p+=$3; pp+=$3*$3; cp+=$2*$3; a+=$4; n++; fp[$5]=1}
  END {
    mc=c/n; mp=p/n;
    sdc=sqrt((cc-n*mc*mc)/(n-1)); sdp=sqrt((pp-n*mp*mp)/(n-1));
    cov=(cp-n*mc*mp)/(n-1);
    printf "n=%d  classical %.3f +/- %.3f   phase %.3f +/- %.3f   ancilla %.1f\n",
           n, mc, sdc/sqrt(n), mp, sdp/sqrt(n), a/n;
    printf "cov(c,p)=%.3f  ->  lambda_total = c + p - cov = %.3f\n", cov, mc+mp-cov;
    printf "distinct md5 = %d / %d\n", length(fp), n;
  }'
echo "--- done $(date +%H:%M:%S) ---"
