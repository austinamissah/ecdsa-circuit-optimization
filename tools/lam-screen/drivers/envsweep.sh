#!/usr/bin/env bash
# lambda + score for pure-env circuit variants, at a fixed ITERS and strip state.
#
# Each spec is  name|ENV=v,ENV=v  -- e.g.  addskip0|TLM_APPLY_ADD_SKIP_LASTK=0
# Everything runs at whatever ITERS the tree is currently built for, with
# SUB4_APPLY_STRIP forced to $STRIP, so an arm differs from the baseline in
# exactly the named knobs.
#
# The knobs here reach the stream: `set_default_env` only writes when absent and
# `install_q1153_submission_defaults` skips its five names when they are already
# set, so an explicit value wins. That is asserted, not assumed -- an arm whose
# md5 ops.bin equals the baseline's is reported as DEAD (04-traps.md section 1).
set -uo pipefail
cd "$(dirname "$0")"
ROOT=$PWD; TREE=$ROOT/base; OUT=$ROOT/env
NONCES=${NONCES:-$ROOT/sweep_nonces.txt}
NWORK=${NWORK:-10}; LANES=${LANES:-16}; STRIP=${STRIP:-0}
mkdir -p "$OUT"
[ -f "$OUT/preflight.tsv" ] || echo -e "name\tenv\tops\tqubits\tavgT\tscore\tclassical\tphase\tmd5" > "$OUT/preflight.tsv"

for spec in "$@"; do
  NAME=${spec%%|*}; ENVS=${spec#*|}
  echo "=== $NAME  [$ENVS]  $(date +%H:%M:%S) ==="
  ENVARGS=(SUB4_APPLY_STRIP=$STRIP)
  if [ "$ENVS" != "$NAME" ]; then
    IFS=',' read -ra KV <<< "$ENVS"
    for kv in "${KV[@]}"; do ENVARGS+=("$kv"); done
  fi

  ( cd "$TREE" && rm -f ops.bin score.json
    env "${ENVARGS[@]}" ./target/release/build_circuit > "$OUT/b_$NAME.log" 2>&1
    env "${ENVARGS[@]}" ./target/release/eval_circuit  > "$OUT/e_$NAME.log" 2>&1 )
  MD5=$(md5sum "$TREE/ops.bin" | cut -d' ' -f1)
  CM=$(grep -oP 'classical mismatches\s*:\s*\K\d+' "$OUT/e_$NAME.log")
  PG=$(grep -oP 'phase-garbage batches\s*:\s*\K\d+' "$OUT/e_$NAME.log")
  read -r AVGT QB OPS < <(tail -1 "$TREE/results.tsv" | awk -F'\t' '{print $3, $5, $6}')
  SC=$(python3 -c "print(round(float('$AVGT'))*int('$QB'))")
  echo -e "$NAME\t$ENVS\t$OPS\t$QB\t$AVGT\t$SC\t$CM\t$PG\t$MD5" >> "$OUT/preflight.tsv"
  echo "  ops=$OPS qubits=$QB avgT=$AVGT score=$SC classical=$CM phase=$PG md5=$MD5"

  if [ "$(grep -c "	$MD5\$" "$OUT/preflight.tsv")" -gt 1 ]; then
    echo "  !! DEAD KNOB: md5 matches another arm -- skipping the lambda run"
    continue
  fi
  if [ "${CM:-0}" -gt 200 ]; then
    echo "  !! BROKEN ARM: classical=$CM is far outside the intrinsic band -- skipping the lambda run"
    continue
  fi

  rm -f "$OUT"/L_${NAME}_*.tsv
  for w in $(seq 0 $((NWORK-1))); do
    awk -v w=$w -v k=$NWORK 'NR%k==w' "$NONCES" > "$OUT/nn_${NAME}_$w.txt"
  done
  T0=$(date +%s)
  for w in $(seq 0 $((NWORK-1))); do
    env "${ENVARGS[@]}" "$TREE/target/release/lamscreen" \
      --nonces "$OUT/nn_${NAME}_$w.txt" --mode count --lanes "$LANES" \
      --tag "$NAME" --out "$OUT/L_${NAME}_$w.tsv" 2>/dev/null &
  done
  wait
  T1=$(date +%s)
  cat "$OUT"/L_${NAME}_*.tsv | awk -F'\t' -v nm="$NAME" '
    $1 != "tag" {s+=$3; ss+=$3*$3; n++; fp[$9]=1}
    END {m=s/n; sd=sqrt((ss-n*m*m)/(n-1));
         printf "  %s  n=%d  mean=%.3f  sd=%.3f  sem=%.3f  distinct_fp=%d\n", nm, n, m, sd, sd/sqrt(n), length(fp)}'
  echo "  wall $((T1-T0)) s"
done
echo "=== env sweep done $(date +%H:%M:%S) ==="
