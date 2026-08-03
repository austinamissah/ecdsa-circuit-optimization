# Docs index

Analyses from the optimization campaign on this reversible secp256k1 point-addition circuit
(metric: **average executed Toffoli × peak qubit width**).

**New here?** The top-level [README](../README.md#what-this-fork-did-and-what-it-found) has the
short version of what this fork measured and what it got wrong. Project write-up:
[amissah.net](https://amissah.net). The three things worth knowing:

1. The first-pass conclusion — *no lever available* — **was wrong**, and is kept and marked rather
   than deleted. See the [lever verdict audit](CONCLUSION.md#lever-verdict-audit).
2. The score is not the binding constraint. **λ**, the intrinsic error rate, is —
   [`lambda-measurement.md`](lambda-measurement.md).
3. The leaderboard leader is an autonomous agent whose method is readable from its own submissions
   — [`upstream-search-economics.md`](upstream-search-economics.md).

> **Provenance.** Everything under "Frontier & literature research" and "Per-component analyses"
> was written 2026-07-11 against commit `422f21d`. The fork was rebased onto upstream `8af8a6f`
> on 2026-08-02, 25 accepted submissions later, and the circuit changed materially (`ITERS`
> 258 → 261, `SCHED_J2` rewritten, a new occupancy tripwire, peak 1152 → 1154). Those documents
> are accurate for the circuit they describe and are kept as a record; check any specific number
> against the current source before relying on it. The 2026-08-02 documents are current.

**Start here:** **[`CONCLUSION.md`](CONCLUSION.md)**, the campaign verdict, every lever tried, the
frontier comparison — with its wrong verdicts marked in place and a
[lever verdict audit](CONCLUSION.md#lever-verdict-audit) of what refuted each one.

## Current (2026-08-02)
- [`lambda-measurement.md`](lambda-measurement.md), λ_total ≈ 20.0 measured on the rebased head
  over 199 nonces: the third axis that is not in the score but decides what can ship, why a clean
  seed costs ~5e8 trials, and the λ targets that would make a grind feasible.
- [`upstream-search-economics.md`](upstream-search-economics.md), how upstream lands a submission
  every 0.88 days: the DGM controller, its 512/2,048/8,192/9,024 shot ladder, why λ is absent from
  its selection function, and what a screen can and cannot tell you.
- [`data/`](data/), raw per-trial measurements, with the integrity checks that make them citable.
- [`../tools/nonce-screen/`](../tools/nonce-screen/) — an **unbuilt, unvalidated draft** of a fast
  nonce screen, kept for its design only. Never compiled, never checked against the harness. Read
  its README before touching it.

## Frontier & literature research
- [`quantum-inversion-frontier-research.md`](quantum-inversion-frontier-research.md), multi-source,
  adversarially-verified survey of reversible modular-inversion circuits and quantum-ECDLP resource
  estimates, plus a direct mining of Schrottenloher 2026's disclosed circuit. Records the published
  figures and their scopes (per-windowed-addition and full-attack figures for Schrottenloher;
  resource estimates with withheld circuits for Google/Babbush), which are not directly comparable to
  one bare affine point addition.

## Per-component analyses
- [`profiling-notes.md`](profiling-notes.md), where the Toffolis go (per-phase breakdown; ~95% is
  two modular inversions).
- [`gcd-engine-study.md`](gcd-engine-study.md), the binary-GCD inversion engine: inner loop,
  reversibility structure, self-tests, schedule tables, and optimization candidates.
- [`schedule-widths.md`](schedule-widths.md), the per-iteration register-width schedules
  (`SCHED_J2`, `GAP_J2`) and what they control.
- [`apply-swap-analysis.md`](apply-swap-analysis.md), why the apply-swap cannot be truncated (the
  swapped register is a full-width accumulator).
- [`dead-gate-analysis.md`](dead-gate-analysis.md), the structural-dead-gate skip tables: safety
  model, saturation, and the CONSTPROP relationship.
- [`constprop-bitgrowth-feasibility.md`](constprop-bitgrowth-feasibility.md), feasibility of teaching
  CONSTPROP a GCD bit-growth invariant (≈0 net after existing coverage).
- [`squaring-analysis.md`](squaring-analysis.md), the modular squaring: Karatsuba + symmetry + NAF
  reduction, the compute/uncompute round-trip, and a measured F-fold regression.

## Reference
- [`submission-process.md`](submission-process.md), how submission to the ecdsa.fail platform works
  (CLI, auth, artifact), from the repo's own files.
