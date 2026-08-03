#!/usr/bin/env bash
# 10 mining shards + 2 held-out validation shards at TLM_SCHED_J2_DELTA=2.
# Shards are separate seeds so any SUBSET can be merged -- that is what gives
# the lambda-vs-census-depth curve for free.
set -uo pipefail
cd "$(dirname "$0")"
PER=${PER:-10000000}
echo "=== census start $(date +%H:%M:%S)  10 mining + 2 held-out, $PER each ==="
for s in 1 2 3 4 5 6 7 8 9 10 101 102; do
  SUB4_APPLY_STRIP=0 TLM_SCHED_J2_DELTA=2 ./t/target/release/census \
    --mode shard --samples "$PER" --seed "$s" --lanes 64 \
    --out "shards/s$s.bin" > "shards/s$s.log" 2>&1 &
done
wait
echo "=== census done $(date +%H:%M:%S) ==="
grep -h "never-fired" shards/*.log
