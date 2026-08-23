#!/usr/bin/env bash
# Confirm hunt survivors against the real scorer, and report any that beat a target.
#
# The pre-filter models the walk and the pseudo-Mersenne fold; it does not model
# the truncated comparisons, so survivors still have a residual failure rate and
# have to be run through `eval_circuit`. A clean nonce is necessary but not
# sufficient: its avg executed Toffoli also has to round below the target.
#
# `results.tsv` and `score.json` are measurement records; eval_circuit appends to
# one and rewrites the other on every run, so both are saved and restored here.
#
# Usage: confirm.sh <survivor-file> <r1> <r2> [target-round-avgT]
set -uo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo"

surv="${1:?usage: confirm.sh <survivor-file> <r1> <r2> [target-round-avgT]}"
r1="${2:?}"
r2="${3:?}"
target="${4:-0}"

rb="$(mktemp)"; cp results.tsv "$rb"
sb="$(mktemp)"; cp score.json "$sb"
trap 'cp "$rb" results.tsv; cp "$sb" score.json; rm -f "$rb" "$sb"' EXIT

printf 'nonce\tclassical\tphase\tancilla\tavg_toffoli\tqubits\tscore\tverdict\n'

while read -r n; do
  [ -z "$n" ] && continue

  SUB4_PINGPONG_TAIL_NONCE="$n" SUB4_PP_R1="$r1" SUB4_PP_R2="$r2" \
    ./target/release/build_circuit >/dev/null 2>&1 || { echo "$n	build failed" >&2; continue; }

  out="$(./target/release/eval_circuit 2>&1)"
  cls="$(printf '%s' "$out" | awk '/classical mismatches/{print $NF}')"
  ph="$(printf '%s' "$out"  | awk '/phase-garbage batches/{print $NF}')"
  an="$(printf '%s' "$out"  | awk '/ancilla-garbage batches/{print $NF}')"
  tof="$(printf '%s' "$out" | awk '/avg executed Toffoli/{print $NF}')"
  q="$(printf '%s' "$out"   | awk '/^  qubits/{print $NF}')"

  if [ "${cls:-x}" = "0" ] && [ "${ph:-x}" = "0" ] && [ "${an:-x}" = "0" ]; then
    score="$(awk -v t="$tof" -v q="$q" 'BEGIN{printf "%d", int(t+0.5)*q}')"
    if [ "$target" != "0" ] && [ "$(awk -v t="$tof" -v g="$target" 'BEGIN{print (int(t+0.5) <= g) ? 1 : 0}')" = "1" ]; then
      verdict=WINNER
    else
      verdict=clean
    fi
  else
    score=n/a
    verdict=dirty
  fi
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$n" "${cls:-?}" "${ph:-?}" "${an:-?}" "${tof:-?}" "${q:-?}" "$score" "$verdict"
done < "$surv"
