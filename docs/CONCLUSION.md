# Optimization campaign: conclusion

A write-up of an effort to lower the score of the reversible secp256k1 point-addition circuit in this
repository (metric: average executed Toffoli count times peak qubit width). The circuit itself is the
community-contributed frontier from the challenge repository; it was reproduced and validated locally.
The material here is the profiling and the analysis of which optimizations are available. The detailed
analyses live alongside this file in `docs/`.

## Verdict

The circuit runs at about 1.32M Toffoli times 1152 qubits (about 1.52e9) for one bare affine point
addition, which is the operation the ecdsa.fail harness scores. That is below both reference Pareto
points listed in the top-level README. The analysis found no available lever that lowers the score:
each lever tried was a dead end, a measured regression, or a correctness break. About 95% of the
Toffoli budget is two modular inversions, which reversible affine point addition requires. In the
literature surveyed in `quantum-inversion-frontier-research.md`, no reversible modular-inversion
implementation has a lower Toffoli count than the windowed binary GCD used here. The remaining option
is a ground-up rewrite of the inversion (a jump-k GCD engine) with an uncertain ceiling.

The published figures cited below are for different operations or scopes than one bare affine point
addition, so a direct score ranking is not like-for-like. Scopes are stated in "Comparison to
published figures".

## Where the cost is

Profiling (`profiling-notes.md`) attributes the about 1.32M executed Toffolis:

| bucket | share | what |
|---|---|---|
| Modular inversion (`tlm_inverse`) | ~47.6% | binary-GCD division `λ = dy/dx` |
| Modular "multiply" (`tlm_forward_multiply`) | ~47.6% | the reversible uncomputation of λ, a second division |
| Modular squaring | ~4.5% | Karatsuba plus symmetric partial products |
| Coordinate add/sub | <0.4% | small |

About 95% is two modular inversions. Reversible affine EC point addition needs two inversions per
addition (compute λ, then reversibly uncompute it). By the time λ is uncomputed the inputs have been
overwritten by the outputs, so λ can only be re-expressed as a division of the outputs. This was
checked against the literature and re-derived from the addition formula.

## Every lever, and its verdict

| # | lever | verdict | detail |
|---|---|---|---|
| 1 | GCD per-iteration width tightening | needs an unproven bit-growth bound | `schedule-widths.md`, `constprop-bitgrowth-feasibility.md` |
| 2 | apply-swap truncation to live width | unsafe; the swapped register is a full-width accumulator | `apply-swap-analysis.md` |
| 3 | extend structural-dead-gate skip tables | saturated; about 10.6K already skipped, CONSTPROP reaches a fixpoint at plus 269 | `dead-gate-analysis.md` |
| 4 | teach CONSTPROP a bit-growth invariant | about 0 net; the deadness is already captured by the tables and width schedule | `constprop-bitgrowth-feasibility.md` |
| 5 | enable disabled F-fold squaring schedule | measured regression: plus 3,458 Toffoli, peak 1153 above 1152, correctness FAILED | `squaring-analysis.md` §7 |
| 6 | in-place squaring rewrite (remove the unbuild) | about 6 to 9K ceiling (dual-use terms tie), not pursued | `squaring-analysis.md` |
| 7 | replace `tlm_forward_multiply` with a cheap multiply | not available; it is the second inversion, not a naive multiply | this file, "Where the cost is" |
| 8 | a cheaper inversion algorithm | none found; binary GCD has a lower reversible Toffoli count than Litinski/Qualtran/RNSL, and Bernstein-Yang divstep is rejected for reversible use | `quantum-inversion-frontier-research.md` |
| 9 | mine the disclosed frontier (Schrottenloher/Qarton) | the disclosed figures are for windowed additions and a full 28-addition attack, not one bare addition (see comparison below) | `quantum-inversion-frontier-research.md` §4 |
| 10 | windowed multiply / HJN swap-rounds | amortization-dependent; a loss for one addition | `quantum-inversion-frontier-research.md` §5 |
| 11 | lower the peak-qubit target (Pareto axis) | env knobs lose (1088: plus 11.9% Toffoli) or break correctness (1216: FAIL); peak is fixed to 1152 by the baked schedules | measured this campaign |

Both axes of the score were tested. At peak 1088 emitted Toffoli rose 11.9%; at peak 1216 emitted
Toffoli fell 0.87% and correctness FAILED. The per-step vent schedules are tuned to 1152.

## Comparison to published figures

Published figures for related work, with their scopes. These are different operations or scopes than
one bare affine point addition, so they are not a direct score ranking.

- This circuit: about 1.32M Toffoli, 1152 qubits, for one bare affine point addition (the ecdsa.fail
  metric).
- Schrottenloher 2026 (arXiv 2606.02235): the disclosed per-windowed-addition figures are 2^21.19
  (about 2.34M) Toffoli at 1192 qubits (space-optimized) and 2^20.83 (about 1.82M) Toffoli at 1446
  qubits (gate-optimized). A windowed addition selects one of 2^w = 2^16 precomputed multiples (window
  w = 16) and uses table lookups; the paper states a lookup of 2^w values costs 2^w Toffoli, and the
  per-addition formula includes 3 times 2^16 Toffoli of lookup, so it is a heavier operation than one
  bare addition.
  The widely cited 2^25.78 Toffoli / 1462 qubit figure is the full Shor attack on secp256k1 (28
  windowed additions), not one addition. Its per-addition claim is about 6.5 to 10% fewer Toffoli and
  about 1.5% more qubits than Babbush et al. Each addition performs two full modular inversions with
  no cross-addition amortization.
- Google/Babbush et al. (2026): the about 2.6M Toffoli / 1175 qubit and about 2.1M / 1425 qubit
  figures are resource estimates; the circuits are withheld behind a zero-knowledge proof, not
  disclosed.

Because Schrottenloher's figures are for windowed additions or the full attack, and Babbush's circuits
are not disclosed, none of these is a bare single-addition circuit directly comparable to the 1.32M /
1152 figure. The README's "about 3x lower" target (about 5e8) corresponds to no disclosed standalone
single-addition circuit; it is below the two-inversion cost and would require cross-addition windowing
that a single-addition benchmark does not use.

## Approaches considered and why each did not apply

- QFT / phase-gradient arithmetic (carry-free addition via Fourier rotations): the gate set is
  Clifford plus Toffoli only, with no rotation or T gate, so Fourier-basis arithmetic is not
  expressible.
- Clifford-is-free exploitation (only CCX/CCZ are counted): the measurement-vent trick already
  converts every ventable AND into Clifford gates, which is why this inversion is about 629K (measured;
  see `profiling-notes.md`) rather than the roughly 1.7M of the surveyed Kaliski implementations
  (Litinski, Qualtran; see `quantum-inversion-frontier-research.md`). It is already applied.
- Cheap-multiply uncompute of the second inversion: does not work because the inputs are overwritten
  by the outputs, so λ can only be re-derived as a division of the outputs.
- Inversion-free projective/Jacobian coordinates: cost more Toffoli than two affine inversions, and do
  not give the unique point representation Shor's algorithm requires.
- Fermat / addition-chain inversion: x^(p-2) is about 256 squarings, about 16M Toffoli, roughly 25
  times more.

## The remaining option (research-scale)

Jump-4 or adaptive-jump binary GCD (from k-ary GCD and Bernstein-Yang batched divstep). Larger jumps
cut the iteration count from 258 toward about 130, which reduces the per-iteration overhead (compare,
tape, control; about 10 to 13% of the two GCDs). The core add-work is roughly conserved, so the
estimated ceiling is about 5 to 10% and uncertain. It is a ground-up rewrite (`JUMP=2` is asserted and
the codec and schedules are jump-2-specific), with no env knob and a build-plus-eval loop of a few
minutes. Everything cheaper than this was tried and did not lower the score.

## Bottom line

The circuit sits at the two-inversion cost of reversible affine point addition, in a gate model
(Clifford plus Toffoli, no rotations) that does not admit the Fourier-arithmetic shortcut. In the
literature surveyed in `quantum-inversion-frontier-research.md`, no reversible modular-inversion
implementation has a lower Toffoli count than the binary GCD used here. Lowering the score further
would require a research-scale change, most plausibly a jump-k GCD engine, rather than tuning of the
current circuit.

---

*Detailed analyses: `profiling-notes.md`, `gcd-engine-study.md`, `schedule-widths.md`,
`apply-swap-analysis.md`, `dead-gate-analysis.md`, `constprop-bitgrowth-feasibility.md`,
`squaring-analysis.md`, `quantum-inversion-frontier-research.md`.*
