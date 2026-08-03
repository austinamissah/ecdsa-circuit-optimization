#!/usr/bin/env bash
# End-to-end seed grind at TLM_SCHED_J2_DELTA=2.
#
# Stage 1: lam-screen in ladder mode over a fresh nonce range, 10 workers.
#          A hit is classical == 0 at the full 9,024-shot rung.
# Stage 2: every hit goes to the REAL harness, because the screen is
#          classical-only and a hit is a candidate, never a seed
#          (upstream-search-economics.md). At lambda_phase_only = 1.90 expect
#          ~7 candidates per true seed.
#
# A seed is only claimed on 0 classical / 0 phase / 0 ancilla from eval_circuit.
set -uo pipefail
cd "$(dirname "$0")"
ROOT=$PWD; TREE=$ROOT/base; OUT=$ROOT/grind
NW=${NW:-10}; BLOCK=${BLOCK:-4000}; ROUNDS=${ROUNDS:-8}
mkdir -p "$OUT"
: > "$OUT/hits.txt"; : > "$OUT/confirmed.txt"

BASE=${BASE:-170000000000000}

for r in $(seq 1 "$ROUNDS"); do
  LO=$((BASE + (r-1)*BLOCK))
  echo "=== round $r: nonces $LO .. $((LO+BLOCK-1))  $(date +%H:%M:%S) ==="
  for w in $(seq 0 $((NW-1))); do
    awk -v lo="$LO" -v n="$BLOCK" -v w="$w" -v k="$NW" \
        'BEGIN{for(i=w;i<n;i+=k) print lo+i}' > "$OUT/n_$w.txt"
  done
  T0=$(date +%s)
  for w in $(seq 0 $((NW-1))); do
    SUB4_APPLY_STRIP=0 TLM_SCHED_J2_DELTA=2 "$TREE/target/release/lamscreen" \
      --nonces "$OUT/n_$w.txt" --mode ladder --lanes 16 --tag grind \
      --out "$OUT/s_$w.tsv" 2>/dev/null &
  done
  wait
  T1=$(date +%s)
  # a hit: classical==0 AND it reached the full rung
  NEW=$(cat "$OUT"/s_*.tsv | awk -F'\t' '$1!="tag" && $3==0 && $4>=9000 {print $2}')
  NH=$(echo "$NEW" | grep -c . )
  echo "  screened $BLOCK in $((T1-T0)) s ($(echo "scale=0; $BLOCK*3600/($T1-$T0)" | bc)/hour), $NH classical-clean candidates"
  [ -n "$NEW" ] && echo "$NEW" >> "$OUT/hits.txt"

  # stage 2: confirm each candidate on the real harness
  for nonce in $NEW; do
    [ -z "$nonce" ] && continue
    ( cd "$TREE" && rm -f ops.bin score.json
      SUB4_APPLY_STRIP=0 TLM_SCHED_J2_DELTA=2 SUB4_TAIL_NONCE=$nonce ./target/release/build_circuit >/dev/null 2>&1
      SUB4_APPLY_STRIP=0 TLM_SCHED_J2_DELTA=2 SUB4_TAIL_NONCE=$nonce ./target/release/eval_circuit > "$OUT/e_$nonce.log" 2>&1 )
    CM=$(grep -oP 'classical mismatches\s*:\s*\K\d+' "$OUT/e_$nonce.log")
    PG=$(grep -oP 'phase-garbage batches\s*:\s*\K\d+' "$OUT/e_$nonce.log")
    AG=$(grep -oP 'ancilla-garbage batches\s*:\s*\K\d+' "$OUT/e_$nonce.log")
    echo "  harness $nonce -> classical=$CM phase=$PG ancilla=$AG"
    echo -e "$nonce\t$CM\t$PG\t$AG" >> "$OUT/confirmed.txt"
    if [ "${CM:-1}" = "0" ] && [ "${PG:-1}" = "0" ] && [ "${AG:-1}" = "0" ]; then
      echo "*** CLEAN SEED FOUND: $nonce ***"
      grep -E "score|toffoli|qubits" "$OUT/e_$nonce.log" | head -6
      cp "$TREE/score.json" "$OUT/score_$nonce.json" 2>/dev/null
      exit 0
    fi
  done
done
echo "=== no clean seed in $((ROUNDS*BLOCK)) nonces $(date +%H:%M:%S) ==="
