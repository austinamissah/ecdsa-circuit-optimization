#!/usr/bin/env bash
# Diversified SAT portfolio, no wall cap. First solver to return sat/unsat writes
# RESULT and the rest are killed. Every run's log persists so a restart loses
# only the in-flight work.
#
#   portfolio.sh <cnf> <outdir> [tag]
set -uo pipefail
# Solver binaries are not committed (see README). Point these at your own builds,
# or put kissat/cadical on PATH and leave them alone.
K="${KISSAT:-$(command -v kissat || echo kissat)}"
C="${CADICAL:-$(command -v cadical || echo cadical)}"

CNF=$1; OUT=$2; TAG=${3:-base}
mkdir -p "$OUT"
rm -f "$OUT/RESULT" "$OUT/STOP"

run() {  # name, binary, args...
  local name=$1; shift
  local log="$OUT/$name.log"
  "$@" "$CNF" > "$log" 2>&1
  local rc=$?
  local st="indeterminate"
  [ $rc -eq 10 ] && st="SAT"
  [ $rc -eq 20 ] && st="UNSAT"
  echo -e "$name\t$rc\t$st\t$(date +%s)" >> "$OUT/status.tsv"
  if [ $rc -eq 10 ] || [ $rc -eq 20 ]; then
    if [ ! -f "$OUT/RESULT" ]; then
      echo -e "$TAG\t$name\t$st\t$rc" > "$OUT/RESULT"
      touch "$OUT/STOP"
    fi
  fi
}

echo "portfolio start $(date -Is) cnf=$CNF" > "$OUT/portfolio.log"
: > "$OUT/status.tsv"

# kissat: 9 arms
run k_sat_s1     "$K" --sat     --seed=1        &
run k_sat_s2     "$K" --sat     --seed=2        &
run k_sat_s3     "$K" --sat     --seed=3        &
run k_unsat_s4   "$K" --unsat   --seed=4        &
run k_unsat_s5   "$K" --unsat   --seed=5        &
run k_def_s6     "$K"           --seed=6        &
run k_def_s7     "$K"           --seed=7        &
run k_plain_s8   "$K" --plain   --seed=8        &
run k_basic_s9   "$K" --basic   --seed=9        &
# cadical: 5 arms
run c_sat_s1     "$C" --sat     --seed=1        &
run c_sat_s2     "$C" --sat     --seed=2        &
run c_unsat_s3   "$C" --unsat   --seed=3        &
run c_def_s4     "$C"           --seed=4        &
run c_plain_s5   "$C" --plain   --seed=5        &

# watchdog: kill the rest as soon as one arm resolves
( while :; do
    if [ -f "$OUT/STOP" ]; then pkill -P $$ -f kissat; pkill -P $$ -f cadical; break; fi
    if ! pgrep -P $$ >/dev/null 2>&1; then break; fi
    sleep 20
  done ) &

wait
echo "portfolio end $(date -Is)" >> "$OUT/portfolio.log"
cat "$OUT/status.tsv" >> "$OUT/portfolio.log"
