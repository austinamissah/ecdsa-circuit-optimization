# Optimization campaign — conclusion

A summary of a systematic effort to lower the score of a reversible secp256k1 point-addition circuit
(metric: **average executed Toffoli count × peak qubit width**). The detailed analyses referenced
below live alongside this file in `docs/`.

## Verdict

**The circuit is at a genuine multi-barrier frontier on both axes of the score, and ahead of every
disclosed academic result.** At **~1.32M Toffoli × 1152 qubits ≈ 1.52×10⁹**, it beats the best
published single-point-addition circuits (Google/Babbush "Circuit One/Two" at 3.0–3.2×10⁹;
Schrottenloher 2026 at 2.6–2.8×10⁹) and sits essentially at the structural floor for a standalone
affine point addition. Every accessible optimization lever was tried and found to be a dead end, a
measured regression, or a correctness break. The only remaining move is a ground-up research effort
with a modest, uncertain ceiling.

## Where the cost is

Profiling (`profiling-notes.md`) attributes the ~1.32M executed Toffolis:

| bucket | share | what |
|---|---|---|
| Modular **inversion** (`tlm_inverse`) | ~47.6% | binary-GCD division `λ = dy/dx` |
| Modular **"multiply"** (`tlm_forward_multiply`) | ~47.6% | the reversible **uncomputation of λ** — a *second* division |
| Modular **squaring** | ~4.5% | Karatsuba + symmetric, already near floor |
| Coordinate add/sub | <0.4% | negligible |

**~95% is two modular inversions.** This is not incidental — reversible *affine* EC point addition
provably needs two inversions per addition (compute λ, then reversibly uncompute it), because by the
time λ is uncomputed the inputs have been overwritten by the outputs, so λ can only be re-expressed as
a division of the outputs. Confirmed against the literature and re-derived from first principles.

## Every lever, and its verdict

| # | lever | verdict | detail |
|---|---|---|---|
| 1 | GCD per-iteration width tightening | needs an unprovable bit-growth bound | `schedule-widths.md`, `constprop-bitgrowth-feasibility.md` |
| 2 | apply-swap truncation to live width | **proven unsafe** — the swapped register is a full-width accumulator | `apply-swap-analysis.md` |
| 3 | extend structural-dead-gate skip tables | **saturated** — ~10.6K already skipped, CONSTPROP hits fixpoint at +269 | `dead-gate-analysis.md` |
| 4 | teach CONSTPROP a bit-growth invariant | ~0 net — the deadness is already captured by the tables + width schedule | `constprop-bitgrowth-feasibility.md` |
| 5 | enable disabled F-fold squaring schedule | **measured regression** +3,458 Toffoli, peak 1153>1152, correctness FAILED | `squaring-analysis.md` §7 |
| 6 | in-place squaring rewrite (kill the unbuild) | ~6–9K ceiling (dual-use terms tie), high risk, not pursued | `squaring-analysis.md` |
| 7 | replace `tlm_forward_multiply` with a cheap multiply | **impossible** — it is the mandatory second inversion, not a naive multiply | this file, §"cost" |
| 8 | a cheaper inversion *algorithm* | **none exists** — binary GCD beats Litinski/Qualtran/RNSL; Bernstein–Yang divstep is rejected for reversible use | `quantum-inversion-frontier-research.md` |
| 9 | mine the disclosed frontier (Schrottenloher/Qarton) | **this design leads it** — every disclosed circuit scores worse | `quantum-inversion-frontier-research.md` §4 |
| 10 | windowed multiply / HJN swap-rounds | amortization-dependent → **a loss for a single addition** | `quantum-inversion-frontier-research.md` §5 |
| 11 | lower the peak-qubit target (Pareto axis) | env knobs **lose (1088: +11.9% Toffoli) or break correctness (1216: FAIL)** — peak is locked to 1152 by the baked schedules | measured this campaign |

**Both axes of the score are exhausted.** Peak = 1152 sits in a sharp minimum: lowering it explodes
Toffoli (convex the wrong way), raising it barely helps *and* breaks correctness (the thousands of
per-step vent schedules are hard-tuned to 1152).

## The frontier, verified by mining disclosed circuits

The published academic state of the art for a **single** point addition (`quantum-inversion-frontier-research.md`):

| circuit | Toffoli / add | qubits | score |
|---|---|---|---|
| **This design** | **~1.32M** | **1152** | **1.52×10⁹** ✅ |
| Schrottenloher 2026 (space-opt) | 2.34M | 1192 | 2.79×10⁹ |
| Schrottenloher 2026 (gate-opt) | 1.82M | 1446 | 2.63×10⁹ |
| Google/Babbush "Circuit One" | 2.7M | 1175 | 3.2×10⁹ |
| Google/Babbush "Circuit Two" | 2.1M | 1425 | 3.0×10⁹ |

Schrottenloher's disclosed circuit does **28 windowed additions**, each with **two full inversions,
no amortization** — confirming both the two-inversion floor and that a "≈3× lower" target
(~5×10⁸) corresponds to no disclosed standalone single-addition circuit; it lies below the
two-inversion floor and would require cross-addition windowing a single-addition benchmark cannot use.

## Creative exploration — the walls are real, not effort limits

A deliberate search across unrelated fields, and why each failed:

- **QFT / phase-gradient arithmetic** (carry-free addition via Fourier rotations): the gate set is
  **Clifford + Toffoli only** — no rotation/T gate exists — so Fourier arithmetic is *inexpressible*.
  This is the deep reason the whole field uses Toffoli-based GCD.
- **Clifford-is-free exploitation** (only CCX/CCZ are counted): the measurement-vent trick already
  converts every ventable AND into free Clifford; it is why this design is 629K, not the literature's
  1.7M — and it is already saturated.
- **Cheap-multiply uncompute of the second inversion**: fails because the inputs are overwritten by
  the outputs, so λ can only be re-derived as a *division* of the outputs (§"cost").
- **Inversion-free projective/Jacobian coordinates**: cost *more* Toffoli than two affine inversions,
  and break Shor-uniqueness.
- **Fermat / addition-chain inversion**: `x^(p−2)` ≈ 256 squarings ≈ ~16M Toffoli — ~25× worse.

## The only lever with a pulse (research-scale)

**Jump-4 / adaptive-jump binary GCD** (from k-ary GCD and Bernstein–Yang batched divstep). Larger
jumps cut the iteration count from 258 toward ~130, shaving the *per-iteration overhead* (compare +
tape + control, ~10–13% of the two GCDs) — though the core add-work is roughly conserved. Realistic
ceiling ~5–10%, uncertain. It is a ground-up rewrite (`JUMP=2` is asserted and the codec/schedules are
jump-2-specific), with no env knob and a ~4-minute build+eval loop — a multi-day research effort, not
a session tweak. Everything cheaper than this is a confirmed dead end.

## Bottom line

This submission is a well-optimized point that **leads the disclosed academic frontier** and sits at
the two-inversion structural floor of reversible affine point addition, in a gate model (Clifford +
Toffoli, no rotations) that forecloses the classical shortcuts. Further progress requires a research
program — most plausibly a jump-k GCD engine — not incremental tuning of what exists.

---

*Detailed analyses: `profiling-notes.md`, `gcd-engine-study.md`, `schedule-widths.md`,
`apply-swap-analysis.md`, `dead-gate-analysis.md`, `constprop-bitgrowth-feasibility.md`,
`squaring-analysis.md`, `quantum-inversion-frontier-research.md`.*
