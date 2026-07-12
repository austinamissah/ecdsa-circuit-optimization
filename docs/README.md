# Docs index

Analyses from the optimization campaign on this reversible secp256k1 point-addition circuit
(metric: **average executed Toffoli × peak qubit width**).

**Start here:** **[`CONCLUSION.md`](CONCLUSION.md)** — the campaign verdict, every lever tried, the
frontier comparison, and the one remaining research-scale lever.

## Frontier & literature research
- [`quantum-inversion-frontier-research.md`](quantum-inversion-frontier-research.md) — multi-source,
  adversarially-verified survey of reversible modular-inversion circuits and quantum-ECDLP resource
  estimates, plus a direct mining of Schrottenloher 2026's disclosed circuit. Establishes that this
  design leads every disclosed single-point-addition circuit.

## Per-component analyses
- [`profiling-notes.md`](profiling-notes.md) — where the Toffolis go (per-phase breakdown; ~95% is
  two modular inversions).
- [`gcd-engine-study.md`](gcd-engine-study.md) — the binary-GCD inversion engine: inner loop,
  reversibility structure, self-tests, schedule tables, and optimization candidates.
- [`schedule-widths.md`](schedule-widths.md) — the per-iteration register-width schedules
  (`SCHED_J2`, `GAP_J2`) and what they control.
- [`apply-swap-analysis.md`](apply-swap-analysis.md) — why the apply-swap cannot be truncated (the
  swapped register is a full-width accumulator).
- [`dead-gate-analysis.md`](dead-gate-analysis.md) — the structural-dead-gate skip tables: safety
  model, saturation, and the CONSTPROP relationship.
- [`constprop-bitgrowth-feasibility.md`](constprop-bitgrowth-feasibility.md) — feasibility of teaching
  CONSTPROP a GCD bit-growth invariant (≈0 net after existing coverage).
- [`squaring-analysis.md`](squaring-analysis.md) — the modular squaring: Karatsuba + symmetry + NAF
  reduction, the compute/uncompute round-trip, and a measured F-fold regression.

## Reference
- [`submission-process.md`](submission-process.md) — how submission to the ecdsa.fail platform works
  (CLI, auth, artifact), from the repo's own files.
