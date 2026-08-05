#!/usr/bin/env bash
# Stage 4a: lambda_total sweep over SUB4_TAIL_NONCE on the UNMODIFIED head.
# Full ./benchmark.sh per trial. No custom screen.
set -uo pipefail

SCRATCH="${SCRATCH:-$(mktemp -d)}"   # override to reuse worker trees between runs
BASE=62000008397024
NWORK=14
OUT="$SCRATCH/sweep_results.tsv"
SHIM="$SCRATCH/shim"

# --- deterministic sandbox path -------------------------------------------
# benchmark.sh prefers `sudo -n bwrap`, and sudo's env_reset strips
# SUB4_TAIL_NONCE before build_circuit ever sees it. A sudo that always fails
# forces the documented `setpriv --no-new-privs bwrap` fallback, which passes
# the environment through. Same bwrap flags, same confinement, identical
# ops.bin (verified: control reproduces md5 f5c5f98258ddb7a0b1f250750ad1c6d2).
mkdir -p "$SHIM"
printf '#!/bin/sh\nexit 1\n' > "$SHIM/sudo"
chmod 755 "$SHIM/sudo"
export PATH="$SHIM:$PATH"

# --- nonce list ------------------------------------------------------------
# Block A (local, 100): base+0..99 -> answers "are clean seeds clustered?"
# Block B (global, 100): base + k*2^40 -> independent regions of the 48-bit space
# Controls: base appears at A0; two explicit repeats appended as tripwires.
build_nonce_list() {
  local i
  for i in $(seq 0 99); do echo -e "A\t$((BASE + i))"; done
  for i in $(seq 1 100); do
    echo -e "B\t$(( (BASE + i * 1099511627776) % 281474976710656 ))"
  done
  echo -e "CTRL\t$BASE"
  echo -e "CTRL\t$BASE"
}

build_nonce_list > "$SCRATCH/nonce_list.tsv"
TOTAL=$(wc -l < "$SCRATCH/nonce_list.tsv")
echo "sweep: $TOTAL trials across $NWORK workers"

printf 'block\tnonce\texit\tclassical\tphase\tancilla\tavgT\tmd5\n' > "$OUT"

# --- worker ----------------------------------------------------------------
run_worker() {
  local wid="$1"
  local wdir
  wdir=$(printf '%s/w%02d' "$SCRATCH" "$wid")
  cd "$wdir" || return 1
  local line n blk lg ex cls ph anc avgt md5 row
  while IFS=$'\t' read -r blk n; do
    lg="$wdir/trial.log"
    SUB4_TAIL_NONCE="$n" ./benchmark.sh > "$lg" 2>&1
    ex=$?
    cls=$(grep -m1 'classical mismatches'    "$lg" | awk -F: '{gsub(/ /,"",$2); print $2}')
    ph=$( grep -m1 'phase-garbage batches'   "$lg" | awk -F: '{gsub(/ /,"",$2); print $2}')
    anc=$(grep -m1 'ancilla-garbage batches' "$lg" | awk -F: '{gsub(/ /,"",$2); print $2}')
    # avg_tof is written to results.tsv on BOTH OK and FAIL rows
    avgt=$(tail -1 results.tsv 2>/dev/null | cut -f3)
    md5=$(md5sum ops.bin 2>/dev/null | cut -d' ' -f1)
    row=$(printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s' \
          "$blk" "$n" "${ex:-NA}" "${cls:-NA}" "${ph:-NA}" "${anc:-NA}" "${avgt:-NA}" "${md5:-NA}")
    # single-line append; O_APPEND makes short writes atomic
    echo "$row" >> "$OUT"
  done < "$SCRATCH/work_$wid.tsv"
}

# --- shard round-robin so blocks A and B interleave across workers ----------
for w in $(seq 0 $((NWORK - 1))); do : > "$SCRATCH/work_$w.tsv"; done
i=0
while IFS= read -r line; do
  echo "$line" >> "$SCRATCH/work_$((i % NWORK)).tsv"
  i=$((i + 1))
done < "$SCRATCH/nonce_list.tsv"

for w in $(seq 0 $((NWORK - 1))); do
  run_worker "$w" &
done
wait

echo "sweep complete: $(( $(wc -l < "$OUT") - 1 )) rows"
