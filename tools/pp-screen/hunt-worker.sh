#!/usr/bin/env bash
# One eval worker of N, draining its shard of a running hunt's survivor list.
#
# Confirmation has to keep pace with survivor generation, because a clean nonce
# is only *found* when it is evaluated, so a backlog is direct latency on the
# result. One worker alongside 14 screening threads fell 1.8x behind on
# 2026-08-23; three workers alongside 12 screening threads kept up.
#
# Each worker runs in its OWN directory: build_circuit writes ./ops.bin and
# eval_circuit reads it, both relative to cwd, so workers sharing a directory
# would race. results.tsv and score.json are written to CARGO_MANIFEST_DIR, a
# compile-time constant, so ALL workers write to the same two files regardless
# of cwd. Those are measurement records: save them before starting a hunt and
# restore them after, and treat anything written during the run as scratch.
#
# Usage: hunt-worker.sh <index> <of-n> <r1> <r2> [target-round-avgT] [dir]
#   index counts from 0. Run one instance per shard.
set -uo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
bin="$repo/target/release"

w="${1:?usage: hunt-worker.sh <index> <of-n> <r1> <r2> [target] [dir]}"
n="${2:?}"
r1="${3:?}"
r2="${4:?}"
target="${5:-0}"
D="${6:-/tmp/pp-hunt}"

dir="$D/w$w"; mkdir -p "$dir"; cd "$dir"
done_f="$D/w$w.done"; res="$D/w$w.results"
touch "$done_f" "$res"

while :; do
  # Shard by line number so workers never collide, and skip anything already done.
  awk -v w="$w" -v n="$n" 'NR%n==w' "$D/survivors.txt" 2>/dev/null | sort -u > "$dir/mine"
  sort -u "$done_f" > "$dir/seen"
  comm -23 "$dir/mine" "$dir/seen" > "$dir/todo"

  while read -r nonce; do
    [ -z "$nonce" ] && continue
    SUB4_PINGPONG_TAIL_NONCE="$nonce" SUB4_PP_R1="$r1" SUB4_PP_R2="$r2" \
      "$bin/build_circuit" >/dev/null 2>&1 || { echo "$nonce" >> "$done_f"; continue; }
    out="$("$bin/eval_circuit" 2>&1)"
    cls="$(printf '%s' "$out" | awk '/classical mismatches/{print $NF}')"
    ph="$(printf '%s'  "$out" | awk '/phase-garbage batches/{print $NF}')"
    an="$(printf '%s'  "$out" | awk '/ancilla-garbage batches/{print $NF}')"
    tof="$(printf '%s' "$out" | awk '/avg executed Toffoli/{print $NF}')"
    q="$(printf '%s'   "$out" | awk '/^  qubits/{print $NF}')"

    if [ "${cls:-x}" = "0" ] && [ "${ph:-x}" = "0" ] && [ "${an:-x}" = "0" ]; then
      v=$(awk -v t="$tof" -v g="$target" \
            'BEGIN{print (g != 0 && int(t+0.5) <= g) ? "WINNER" : "clean"}')
      printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$nonce" "$cls" "$ph" "$an" "$tof" "$q" "$v" >> "$res"
    else
      printf '%s\t%s\t%s\t%s\t%s\t%s\tdirty\n' \
        "$nonce" "${cls:-?}" "${ph:-?}" "${an:-?}" "${tof:-?}" "${q:-?}" >> "$res"
    fi
    echo "$nonce" >> "$done_f"
  done < "$dir/todo"
  sleep 20
done
