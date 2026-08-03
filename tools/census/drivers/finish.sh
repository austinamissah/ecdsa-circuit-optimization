#!/usr/bin/env bash
# After mining: emit re-mined keys, price them, verify on the FULL harness,
# and measure the lambda they cost. All at TLM_SCHED_J2_DELTA=2.
set -uo pipefail
cd "$(dirname "$0")"
T=$PWD/t
E="SUB4_APPLY_STRIP=0 TLM_SCHED_J2_DELTA=2"

score_of() { python3 -c "
import sys
a,q=sys.argv[1],sys.argv[2]
print(round(float(a))*int(q))" "$1" "$2"; }

row() { # label strip -> ops qubits avgT score classical phase md5
  local label=$1 strip=$2
  ( cd "$T" && rm -f ops.bin score.json
    env SUB4_APPLY_STRIP=$strip TLM_SCHED_J2_DELTA=2 ./target/release/build_circuit > "/tmp/b_$label.log" 2>&1
    env SUB4_APPLY_STRIP=$strip TLM_SCHED_J2_DELTA=2 ./target/release/eval_circuit  > "/tmp/e_$label.log" 2>&1 )
  local md5 cm pg ag avgt qb ops sc
  md5=$(md5sum "$T/ops.bin" | cut -d' ' -f1)
  cm=$(grep -oP 'classical mismatches\s*:\s*\K\d+' "/tmp/e_$label.log")
  pg=$(grep -oP 'phase-garbage batches\s*:\s*\K\d+' "/tmp/e_$label.log")
  ag=$(grep -oP 'ancilla-garbage batches\s*:\s*\K\d+' "/tmp/e_$label.log")
  read -r avgt qb ops < <(tail -1 "$T/results.tsv" | awk -F'\t' '{print $3, $5, $6}')
  sc=$(score_of "$avgt" "$qb")
  echo -e "$label\tstrip=$strip\tops=$ops\tq=$qb\tavgT=$avgt\tscore=$sc\tclassical=$cm\tphase=$pg\tancilla=$ag\tmd5=$md5"
  grep -E "stale keys skipped" "/tmp/b_$label.log" | tail -1
}

echo "=== 1. baseline: delta 2, strip OFF  $(date +%H:%M:%S) ==="
row d2_stripoff 0

echo "=== 2. emit from the 12 MINING shards only (324M)  $(date +%H:%M:%S) ==="
MSH=$(for f in shards/s[0-9].bin shards/s10.bin; do printf "%s:27000000," "$f"; done | sed 's/,$//')
env $E "$T/target/release/census" --mode emit --shards "$MSH" \
    --out "$PWD/deep_strip_keys.mining.rs" 2>&1 | grep -E "merged|wrote"

echo "=== 3. re-emit including the 2 HELD-OUT shards (378M)  $(date +%H:%M:%S) ==="
ASH="$MSH,$(for f in shards/s10[12].bin; do printf "%s:27000000," "$f"; done | sed 's/,$//')"
env $E "$T/target/release/census" --mode emit --shards "$ASH" \
    --out "$PWD/deep_strip_keys.validated.rs" 2>&1 | grep -E "merged|wrote"

echo "--- held-out result: keys the mining census got WRONG ---"
python3 - <<'PYEOF'
import re
def load(p):
    s = open(p).read()
    d = set(re.findall(r'^    \((\d+, [\d, ]+?)\),$', s, re.M))
    dead = set(x for x in d if x.count(',') == 6)
    down = set(x for x in d if x.count(',') == 7)
    return dead, down
md, mw = load('deep_strip_keys.mining.rs')
vd, vw = load('deep_strip_keys.validated.rs')
print(f"  mining-only (324M): {len(md)} dead, {len(mw)} downgrade")
print(f"  with held-out (378M): {len(vd)} dead, {len(vw)} downgrade")
print(f"  caught by 54M of held-out data: {len(md-vd)} dead keys fired, {len(mw-vw)} downgrades violated")
n_false, N_v = len(md-vd)+len(mw-vw), 54e6
print(f"  -> false-key rate {n_false} per 54M samples; each false key costs ~9024/N lambda,")
print(f"     so the mining set carried roughly {n_false*9024/N_v:.3f} lambda of undetected error.")
PYEOF

echo "=== 4. install re-mined keys and rebuild  $(date +%H:%M:%S) ==="
cp "$T/src/point_add/deep_strip_keys.rs" deep_strip_keys.orig.rs
cp deep_strip_keys.validated.rs "$T/src/point_add/deep_strip_keys.rs"
( cd "$T" && cargo build --release --locked --offline \
    --bin build_circuit --bin eval_circuit --bin lamscreen 2>&1 | grep -E "^error" -A5 | head -20 )

echo "=== 5. delta 2, re-mined strip ON, FULL HARNESS  $(date +%H:%M:%S) ==="
row d2_remined 1

echo "=== 6. lambda_classical, re-mined strip ON, n=400  $(date +%H:%M:%S) ==="
for w in $(seq 0 9); do awk -v w=$w 'NR%10==w' nonces400.txt > "shards/n_$w.txt"; done
for w in $(seq 0 9); do
  env SUB4_APPLY_STRIP=1 TLM_SCHED_J2_DELTA=2 "$T/target/release/lamscreen" \
    --nonces "shards/n_$w.txt" --mode count --lanes 16 --tag remined \
    --out "shards/L_$w.tsv" 2>/dev/null &
done
wait
cat shards/L_*.tsv | awk -F'\t' '$1!="tag"{v[n++]=$3; s+=$3; fp[$9]=1}
  END{m=s/n; for(i=0;i<n;i++) d+=(v[i]-m)^2; sd=sqrt(d/(n-1));
      printf "remined-strip-on: n=%d lambda_c=%.3f sd=%.3f sem=%.3f distinct_fp=%d\n", n, m, sd, sd/sqrt(n), length(fp)}'
echo "  (compare: delta 2 strip OFF = 5.787 +/- 0.125, n=400, measured 2026-08-03)"
echo "=== finish done $(date +%H:%M:%S) ==="
