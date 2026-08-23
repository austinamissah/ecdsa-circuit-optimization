#!/usr/bin/env bash
# Confirm prefilter survivors against the real simulator.
#
# The prefilter only models the walk. Survivors still carry the residual
# failure rate of the truncated-comparison repairs, so each one has to be run
# through the actual trusted scorer. This drives build_circuit and eval_circuit
# directly rather than through benchmark.sh, for two reasons:
#
#   * benchmark.sh prefers the `sudo -n bwrap` path, whose `env_reset` silently
#     drops SUB4_/TLM_ overrides, so every trial would measure the default.
#   * eval_circuit appends a row to results.tsv and rewrites score.json on every
#     run. Those are measurement records, so they are saved and restored here
#     instead of being polluted with screening traffic.
#
# Usage: verify.sh <survivors-file> [rounds] [rounds-mul]
set -uo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo"

survivors="${1:?usage: verify.sh <survivors-file> [rounds] [rounds-mul]}"
rounds="${2:-698}"
rounds_mul="${3:-696}"

results_backup="$(mktemp)"
score_backup="$(mktemp)"
cp results.tsv "$results_backup"
cp score.json "$score_backup"
restore() {
  cp "$results_backup" results.tsv
  cp "$score_backup" score.json
  rm -f "$results_backup" "$score_backup"
}
trap restore EXIT

printf 'nonce\tclassical\tphase\tancilla\tavg_toffoli\tqubits\tscore\tverdict\n'

while read -r nonce; do
  [ -z "$nonce" ] && continue

  SUB4_PINGPONG_TAIL_NONCE="$nonce" \
  SUB4_PP_ROUNDS="$rounds" \
  SUB4_PP_ROUNDS_MUL="$rounds_mul" \
    ./target/release/build_circuit >/dev/null 2>&1 || { echo "$nonce	build failed" >&2; continue; }

  out="$(./target/release/eval_circuit 2>&1)"

  cls="$(printf '%s' "$out" | awk '/classical mismatches/{print $NF}')"
  phase="$(printf '%s' "$out" | awk '/phase-garbage batches/{print $NF}')"
  anc="$(printf '%s' "$out" | awk '/ancilla-garbage batches/{print $NF}')"
  tof="$(printf '%s' "$out" | awk '/avg executed Toffoli/{print $NF}')"
  qub="$(printf '%s' "$out" | awk '/^  qubits/{print $NF}')"

  if [ "${cls:-x}" = "0" ] && [ "${phase:-x}" = "0" ] && [ "${anc:-x}" = "0" ]; then
    verdict=CLEAN
    score="$(awk -v t="$tof" -v q="$qub" 'BEGIN{printf "%d", int(t+0.5)*q}')"
  else
    verdict=dirty
    score=n/a
  fi
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$nonce" "${cls:-?}" "${phase:-?}" "${anc:-?}" "${tof:-?}" "${qub:-?}" "$score" "$verdict"
done < "$survivors"
