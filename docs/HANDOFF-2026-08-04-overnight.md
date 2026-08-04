# Handoff — overnight queue, 2026-08-03/04

Baseline throughout: **head `6909d15` = 1,486,468,554** (avgT 1,288,101.386 × 1154, 0/0/0,
md5 `ef30945f3afcb369192ea32897232d2f`). `RIG.md` (1,489,216,228) and `mod.rs:2379`
(1,487,599,474) both quote stale figures — see
[`rebase-2026-08-03-upstream-ed4b529.md`](rebase-2026-08-03-upstream-ed4b529.md).

**The strict-beat bar is 0.886 avgT.** `round(avgT) ≤ 1,288,100` wins by 1,154 points. That
is 7,995 shot-charges out of 11,623,826,906 — worth keeping in mind, because it means almost
any real structural saving clears it.

## Item 1 — fire-versus-charge cross-census: **DONE, lever closed**

Full writeup: [`fire-vs-charge-cross-census.md`](fire-vs-charge-cross-census.md).

`hotness.rs` extended to accumulate each gate's per-shot effect mask alongside its charge,
still gated on `attributed == toffoli_gates == 11,623,826,906` and `avgT == 1288101.386`.

| | shot-charges | avgT | share |
|---|---:|---:|---:|
| total charge | 11,623,826,906 | 1,288,101.4 | 100% |
| actually fires | 2,708,896,591 | 300,188.0 | 23.30% |
| **cooling headroom** | **8,914,930,315** | **987,913.4** | **76.70%** |

**No cooling candidate exists, and the reason is structural rather than a failed search.**
The fire-rate mode is 0.25 — exactly the rate a Toffoli with two independent uniform controls
fires — with 60.1% of unconditional gates in [0.20, 0.30). `PushCondition` takes a *classical*
bit; whether a gate fires is a function of its *quantum* controls; the classical bits here are
`Hmr` measurement outcomes, independent of them. `P(a gate's ~2,256 firing shots all fall
inside a fair coin's 4,512) = 2^-2256`.

The census did surface **46,134 gates (3.35% of score) that fire on none of the 9,024 official
shots** — one such gate would clear the 0.886 bar. Not harvestable: that is a certificate about
*one draw*, and deleting a gate re-rolls all 9,024 inputs (`04-traps.md` §1). They survived
upstream's census, so their fire rate is small but nonzero, and deletion costs ≈ `9024·p` λ each.

**Nothing from item 1 scores below the head, so the item-3 grind trigger did not fire.**

## Item 2 — exact-eight joint synthesis: **IN FLIGHT, no verdict**

Full tooling: [`../tools/sat/`](../tools/sat/).

**Known-answer test PASSED at both levels.** The encoder's report gives
`searches[0].cnf = 11,416 variables / 54,051 clauses` and the emitted DIMACS header reads
`p cnf 11416 54051`, matching `06-research-status.md`. We are attacking the same instance.

No SAT solver was present on this machine; **kissat 4.0.4** and **cadical 3.0.1** were built
from source into
`/tmp/claude-1000/.../scratchpad/solvers/{kissat,cadical}/build/` — rebuild them before
resuming (`./configure && make -j8` in each clone).

`tools/sat/symbreak.py` implements reopening condition 1, a conditional lexicographic
**gate-order** break: two shears commute unconditionally when four GF(2) dot products vanish,
and for each adjacent pair it emits `commute(i,i+1) → params(i) ≤_lex params(i+1)`.
Conditioning on `commute` is what makes it sound. Left/right control commutativity was already
broken upstream in `constrain_gate_shape`. **Wire permutation is not a symmetry of this
instance** — the 25 inputs are pinned to literal bit patterns — so no break is emitted for it,
and reopening condition 1 is only partially discharged as a result.

### State at handoff

- **Baseline portfolio: running, no result.** 14 diversified arms (kissat
  `--sat`/`--unsat`/`--default`/`--plain`/`--basic`, cadical
  `--sat`/`--unsat`/`--default`/`--plain`, distinct seeds) on the unmodified CNF, launched
  ~21:57, no wall cap. `RESULT` absent, `status.tsv` empty — every arm still searching, which
  already exceeds the two-CPU-hour cap the prior run stopped at.
- **Symmetry-break selftest: did not return in the window.** It builds exact-9 (known SAT via
  the nine-CCX reference) and requires SAT both with and without the break, at 900 s per solve,
  on a machine already running 15 solvers. **Until it passes, the broken CNF must not be used
  and no UNSAT from it may be believed** — a break that loses a solution turns an open question
  into a false UNSAT.

### What to do next, in order

1. Re-run `python3 tools/sat/symbreak.py --selftest --kissat <path> --timeout 1800` **on an
   idle machine.** If exact-9 times out rather than returning SAT, that is a failure of the
   *test*, not of the break — lower the load and retry before concluding anything.
2. If the selftest passes: `symbreak.py --gates 8 --out exact8-broken.cnf`, then
   `tools/sat/portfolio.sh exact8-broken.cnf <outdir> broken`, split with the baseline arms so
   the machine is not oversubscribed.
3. **A timeout is a timeout.** Do not record UNSAT unless an arm exits 20 *and* the selftest
   passed on the same build.
4. Reopening conditions 2–4 (stronger encoding, distinct representation, compiled witness) are
   untouched.

**Even a SAT verdict is not a score.** It would be an 8-CCX joint codec, and converting that
into a scored circuit is a substantial implementation. Item 2 could not have fired the item-3
trigger tonight.

## Items 3 and 4 — grind and submission prep: **did not fire, correctly**

The trigger is "any configuration from items 1 or 2 scores below 1,486,468,554 on the full
harness". Item 1 produced no configuration (the lever is closed, not deferred); item 2 has no
verdict and would not produce a scored circuit even with one. **No grind was started and no
submission branch was created.** Doing either would have meant grinding a circuit that is not
below the frontier, which is the exact failure the rejected submission already demonstrated.

## Standing facts re-confirmed tonight

- Every arm in `docs/data/arms-newbase-2026-08-03.tsv` has a distinct `md5 ops.bin`; the
  dead-knob trap did not recur.
- Both new instruments reproduce a known quantity before their output is used: `hotness.rs`
  reconstructs the scorer's Toffoli total exactly, and the SAT work reproduces 11,416 / 54,051.
  The hotness gate caught a 9,216-vs-9,024 shot-count error that looked entirely plausible in
  the output.
- Delta arms remain dead against the new head: d1 +1.68% (upstream strip) / +1.92% (strip off),
  d2 +2.30% / +2.52%.

## Scratch that matters

```
/tmp/claude-1000/-home-anna-ecdsa-circuit-optimization/8942013c-.../scratchpad/
  d1/t/                     built tree (build_circuit, eval_circuit, census, lamscreen, hotness)
  d1/arms.tsv               the six arms, also committed to docs/data/
  d1/hot/head.hot.tsv       per-gate charge/fire dump, 1.34 M rows (regenerate: 53 s)
  solvers/{kissat,cadical}  built solver binaries
  sat/run/base/             baseline portfolio logs and status.tsv
```

`/tmp` is not durable. Everything load-bearing is committed; the dumps and solver binaries are
cheap to rebuild and deliberately not.
