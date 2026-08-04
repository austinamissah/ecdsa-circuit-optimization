# `sat` — exact-eight joint synthesis attack

`src/point_add/memory/06-research-status.md` leaves *unrestricted exact-eight joint
synthesis* open: 11,416 variables / 54,051 clauses, no witness and **no UNSAT proof**,
stopped at a preregistered two-CPU-hour cap rather than at a wall. It names four
reopening conditions; the first is "a machine-checkable symmetry reduction".

## Known-answer test — run this first

The encoder is `../../src/point_add/memory/repro/y5_joint_codec_synth.py`, which needs
`kissat` and `cadical` on `PATH`. Reproducing their CNF is the gate: without it we would
be attacking a different problem.

```bash
python3 src/point_add/memory/repro/y5_joint_codec_synth.py --timeout-seconds 20 --max-local-seconds 90
python3 -c "print(open('.autoresearch/measurements/y5-joint-codec-synth-v1/cnf/joint-codec-exact-8.cnf').readlines()[4])"
# must print: p cnf 11416 54051
```

Confirmed 2026-08-03 at both levels — the report's `searches[0].cnf` and the DIMACS
header both read 11,416 / 54,051.

## `symbreak.py` — the symmetry reduction

Rebuilds the instance through the same encoder and appends a **conditional
lexicographic gate-order break**. Two shears commute unconditionally when four GF(2)
dot products vanish (`l2.d1 = r2.d1 = l1.d2 = r1.d2 = 0`); for each adjacent pair it
emits `commute(i,i+1) -> params(i) <=_lex params(i+1)`. Conditioning on `commute` is
what keeps it sound — non-commuting neighbours retain both orders.

Left/right control commutativity is already broken by
`y5_normalizer_synth.constrain_gate_shape`. **Wire permutation is not a symmetry here**
and no break is emitted for it: the 25 inputs are pinned to literal bit patterns and the
target table is fixed, so conjugating by a wire map changes the required inputs.

```bash
python3 tools/sat/symbreak.py --selftest      # REQUIRED before trusting any UNSAT
python3 tools/sat/symbreak.py --gates 8 --out /tmp/exact8-broken.cnf
```

`--selftest` checks two things: that the unbroken exact-8 rebuild is exactly
11,416 / 54,051, and that **exact-9 — known SAT, since the nine-CCX reference is a
witness — stays SAT with the break applied.** A symmetry break that loses a solution
would turn this instance's open question into a false UNSAT, so this is not optional.

## `portfolio.sh` — diversified, no wall cap

14 arms: kissat `--sat`/`--unsat`/`--default`/`--plain`/`--basic` and cadical
`--sat`/`--unsat`/`--default`/`--plain`, each with a distinct `--seed`. The first arm to
return 10/20 writes `RESULT` and the watchdog kills the rest. Every arm's log persists,
so a restart loses only in-flight work.

```bash
tools/sat/portfolio.sh /path/to.cnf /path/to/outdir tag
```

**A timeout is a timeout, not UNSAT.** Nothing here may be recorded as an UNSAT proof
unless an arm exits 20 and `symbreak.py --selftest` passed on the same build.
