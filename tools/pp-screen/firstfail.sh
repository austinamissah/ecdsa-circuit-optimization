#!/usr/bin/env bash
# Compare the model's first failing shot against the simulator's, per nonce.
#
# This is the sharp test of whether the walk model is faithful. A walk failure
# is a hard failure in the circuit, so if the model says the first walk failure
# is at shot X, the simulator's first failure Y must satisfy Y <= X. Two
# outcomes matter:
#
#   Y == X   the walk failure IS the first failure: the model is catching the
#            dominant mode and nothing earlier is being missed.
#   Y <  X   something the model does not cover (the truncated-comparison
#            repairs) failed earlier. Fine for correctness, but it bounds how
#            much filtering power the model can have.
#   Y >  X   the model rejected a shot the simulator accepts. That is a false
#            rejection and means the model is WRONG: it would discard valid
#            nonces during a grind.
#
# Usage: firstfail.sh <nonce-file> [rounds] [rounds-mul]
set -uo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo"

nonces="${1:?usage: firstfail.sh <nonce-file> [rounds] [rounds-mul]}"
rounds="${2:-698}"
rounds_mul="${3:-696}"

results_backup="$(mktemp)"; cp results.tsv "$results_backup"
score_backup="$(mktemp)";   cp score.json "$score_backup"
trap 'cp "$results_backup" results.tsv; cp "$score_backup" score.json; rm -f "$results_backup" "$score_backup"' EXIT

# One build at this config seeds every nonce below: the tail only rewrites
# q_target on 96 existing ops, so the transcript prefix is shared.
SUB4_PP_ROUNDS="$rounds" SUB4_PP_ROUNDS_MUL="$rounds_mul" \
  ./target/release/build_circuit >/dev/null 2>&1

model="$(./target/release/pp_screen --ops ops.bin --threads 4 --verbose \
          --rounds "$rounds" --rounds-mul "$rounds_mul" \
          $(while read -r n; do [ -n "$n" ] && printf ' --nonce %s' "$n"; done < "$nonces") \
          2>/dev/null | awk -F'\t' '$1=="FIRSTFAIL"{print $2"\t"$3} $1=="SURVIVOR"{print $2"\tnone"}')"

printf 'nonce\tmodel_first_fail\tsim_first_fail\tverdict\n'
while read -r n; do
  [ -z "$n" ] && continue
  mshot="$(printf '%s\n' "$model" | awk -F'\t' -v n="$n" '$1==n{print $2}')"

  SUB4_PINGPONG_TAIL_NONCE="$n" SUB4_PP_ROUNDS="$rounds" SUB4_PP_ROUNDS_MUL="$rounds_mul" \
    ./target/release/build_circuit >/dev/null 2>&1
  sshot="$(./target/release/eval_circuit 2>&1 \
            | awk 'match($0, /MISMATCH shot [0-9]+/){print substr($0, RSTART+14, RLENGTH-14); exit}')"
  [ -z "$sshot" ] && sshot=none

  if [ "$mshot" = none ] || [ "$sshot" = none ]; then
    verdict=n/a
  elif [ "$sshot" -eq "$mshot" ]; then
    verdict=EXACT
  elif [ "$sshot" -lt "$mshot" ]; then
    verdict=sim_earlier
  else
    verdict=FALSE_REJECTION
  fi
  printf '%s\t%s\t%s\t%s\n' "$n" "${mshot:-?}" "$sshot" "$verdict"
done < "$nonces"
