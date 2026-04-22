# Research notes — quantum_ecc inversion algorithm space

Session: 2026-04-22 (continued, iterative moonshot work).
Author: autoresearch agent.

Baseline state: 4,394,546 avg executed Toffoli @ 2729 qubits.
Kaliski modular inversion contributes ~81% of the circuit budget
(3.55M Toffoli across the two inversion passes).

This document surveys published modular-inversion algorithms that could
plausibly replace or augment Kaliski, with their iteration structure,
per-iteration reversible cost, and publication status.

## Deliverable 1 result (classical B-Y empirical survey)

Implemented `divstep2` (Bernstein–Yang 2019/266, §8) in
`src/classical_by.rs` and ran it on 10,000 random secp256k1 inputs
seeded by SHAKE128.

| metric | value |
|---|---|
| theoretical safegcd bound `⌈(49·256 + 57)/17⌉` | 742 |
| observed minimum iters | 502 |
| observed maximum iters | **567** |
| observed mean iters | 531 |
| max |δ| during execution | 20 |
| modinv matches (vs Fermat) | 10,000 / 10,000 |

**Key implication**: safegcd's upper bound overestimates real-world
iteration count on secp256k1 by **24%**. Also `|δ| ≤ 20` always, so
the δ register needs only ~7 bits, not a full-width integer.

## Additional empirical result: jumpdivstep matrix entries are MUCH smaller than 2^w

A key pessimism in the earlier B-Y analysis was assuming jumpdivstep's
2×2 matrix entries reach the BY worst-case magnitude 2^w often enough
that reversible matrix-apply must budget full w-bit classical constants.

I added `jumpdivstep_matrix_entry_survey` to `src/classical_by.rs` and
sampled 100,000 random low-word states for several widths w.

| w | max observed |entry| | max log2 |entry| | mean log2 |entry| | theoretical max log2 |
|---|---:|---:|---:|---:|
| 4  | 8    | 3.00  | 0.43 | 4  |
| 8  | 55   | 5.78  | 0.94 | 8  |
| 12 | 233  | 7.86  | 1.62 | 12 |
| 16 | 1364 | 10.41 | 2.48 | 16 |

This matters a lot. If the matrix entries are *typically* only 8–10 bits
wide even when w=16, then the per-jump reversible matrix-apply cost is
closer to ~10n than to ~16n or ~31n. The classical jumpdivstep advantage
may survive reversibly after all.

That reopens B-Y as a real moonshot candidate, but only in the **jumped**
form. Plain w=1 still loses.

## Deliverable 2: algorithm space survey

All costs are for **n = 256** (secp256k1). Reversible costs are
**measured** (for Kaliski, from our instrumented build) or
**estimated conservatively** (per-iteration op counts × naive
register sizes). Where a later empirical correction changes an earlier
upper bound, I note both.

### 1. Kaliski almost-inverse  — baseline (used by our circuit)

- **Classical ref**: Kaliski 1995, *"The Montgomery inverse and its
  applications"*, IEEE Trans. Computers 44(8), 1064–1065.
  DOI: 10.1109/12.403725.
- **Reversible refs**:
  - Roetteler et al. 2017 (RNSL), arXiv:1706.06752.
  - Häner et al. 2020 (HRSL), arXiv:2001.09580, eprint 2020/077.
- **Iteration count**: classically 2n = 512 iters for deterministic
  convergence; our code truncates to 399 (tuned against 9024-shot
  deterministic corpus).
- **Per-iter reversible cost (measured)**: **~2180 CCX**.
- **Per-pass (forward + backward)**: **1.81M CCX**.
- **Structural notes**:
  - Binary-GCD style. Each iter: parity/equality check, 2n-bit comparator,
    two cswaps of two register pairs, fused cond-sub/cond-add, halve.
  - Reversibility via `m_hist` branch log (one qubit per iter).

### 2. Bernstein–Yang divstep2 (`w = 1`)  — no reversible impl published

- **Classical ref**: Bernstein, Yang 2019, *"Fast constant-time gcd
  computation and modular inversion"*, eprint 2019/266, TCHES 2019(3).
- **Reversible ref**: **unpublished / would be novel research**.
- **Iteration count**:
  - Theoretical bound: **742**.
  - Empirical worst case (10k secp256k1 samples): **567**.
  - Empirical mean: 531.
- **Per-iter reversible cost (est.)**:
  - Branch predicate `(δ > 0) ∧ (g odd)`: 1 CCX.
  - Cswap of 3-4 register pairs and sign handling: 7n.
  - Cond add/sub g ± f and coeff updates: 3–5n.
  - Halve g: 0.
  - **Upper-bound estimate: 10–12n ≈ 2560–3072 CCX/iter**.
- **Per-pass cost**:
  - Using empirical max 567 and optimistic 10n: 567 × 2560 × 2 ≈ 2.90M.
  - Using conservative 12n: 3.48M.
- **Verdict vs Kaliski**: Worse by ~1.6–1.9× per pass. `w=1` is not a bet.

### 3. Bernstein–Yang jumpdivstep (`w ≫ 1`)  — no reversible impl published

- **Same paper, §9 of eprint 2019/266**.
- **Classical speedup**: batches `w` divsteps into one 2×2 matrix `M`.
  Widely used in classical constant-time crypto (e.g. libsecp256k1).
- **Reversible ref**: **unpublished / would be novel research**.
- **Naive earlier estimate (pessimistic)**:
  - Assumed |M[i][j]| ≈ 2^w typically.
  - Per-jump cost scales like w·n for each matrix-entry multiply.
  - This cancelled most of the 1/w iteration-count reduction.
- **Empirical correction (new)**:
  - Matrix entries are much smaller than 2^w in practice on random low-word
    states: at w=16, max observed |entry| was 1364 (< 2^11) and mean log2
    magnitude only 2.48.
  - So a faithful reversible implementation should cost closer to
    `entry_bits · n`, where `entry_bits` is ~8–11 for w up to 16, not 16.
- **Updated rough cost model**:
  - For w=12: empirical max entry bits ≈ 8. Let matrix-apply cost be
    ~8n on (f,g) and another ~8n on coeffs = **16n ≈ 4096 CCX/jump**.
    Iterations: 567 / 12 ≈ 48 jumps.
    Per pass: 48 × 4096 ≈ **197k CCX forward**, doubled ≈ **394k/pass**.
  - For w=16: empirical max entry bits ≈ 11. Let cost ~22n ≈ 5632/jump.
    Iterations: ~36 jumps.
    Per pass: 36 × 5632 × 2 ≈ **405k/pass**.
- **New verdict**: **BY at some jumped w (probably 12–16) is worth prototyping.**
  This completely reverses the previous pessimistic conclusion.

### 4. Montgomery inverse (Savaş–Koç)  — Kaliski variant

- **Classical ref**: Savaş, Koç 2000, *"The Montgomery modular
  inverse — revisited"*, IEEE Trans. Computers 49(7), 763–766.
  DOI: 10.1109/12.863048.
- **Reversible ref**: used by RNSL 2017 (arXiv:1706.06752) and HRSL
  2020 (arXiv:2001.09580) as the inversion primitive. Our Kaliski is
  structurally equivalent.
- **Iteration count**: same as Kaliski.
- **Per-iter reversible cost**: essentially identical to Kaliski.
- **Verdict**: not a different reversible algorithm in practice.

### 5. Lehmer-style GCD  — no reversible impl published

- **Classical refs**:
  - Lehmer 1938, *"Euclid's algorithm for large numbers"*, Amer. Math. Monthly 45(4).
  - Jebelean 1993, *"A double-digit Lehmer–Euclid algorithm for finding the GCD of long integers."*
- **Reversible ref**: **unpublished / would be novel research**.
- **Idea**: approximate (u, v) by top-k bits, compute a small 2×2 matrix
  classically, apply it to the full registers.
- **Iteration count**: ~2n/k. For k=12, ~43 steps worst-case, but with
  jump-style batching maybe less.
- **Per-step reversible cost**:
  - If a select-swap QROM can read the matrix in O(√(2^{2k})) = O(2^k)
    rather than O(2^{2k}) and matrix entries stay around 12 bits,
    matrix-apply cost could be competitive.
- **Verdict**: still plausible, but **higher research risk** than jumped
  B-Y because the classical step-selection logic is more irregular.

### 6. Fermat's little theorem (`a^{p−2}`) via addition chain

- **Classical ref**: addition-chain literature; standard FLT inversion.
- **Reversible refs**: discussed in RNSL; not used in prime-field ECC.
- **Cost**: ~255 squarings + ~8–15 general muls.
  At ~70–80k Toffoli/mul, this is ~20M CCX.
- **Verdict**: much worse than Kaliski.

### 7. Itoh–Tsujii inversion (GF(2^n) only)

- **Classical ref**: Itoh, Tsujii 1988.
- **Applicability**: GF(2^n), not GF(p). secp256k1 uses a prime field.
- **Verdict**: not applicable.

## Summary table

| algo | iters (n=256) | per-iter/step CCX | per-pass CCX | pub reversible? | competitive? |
|---|---:|---:|---:|---|---|
| Kaliski | 399 | 2180 | 1810k | yes | baseline |
| B-Y divstep w=1 | 567 (emp max) | 2560–3072 | 2900–3480k | no | no |
| B-Y jumpdivstep w≈12 | ~48 jumps | ~4096/jump | **~394k** | no | **yes (promising)** |
| B-Y jumpdivstep w≈16 | ~36 jumps | ~5632/jump | **~405k** | no | **yes (promising)** |
| Montgomery inv | ≈399 | ≈2200 | ≈1810k | yes | same as Kaliski |
| Lehmer + QROM | unknown | unknown | maybe 500–1000k | no | maybe |
| Fermat chain | ~270 muls | 75k/mul | 20M | yes | no |
| Itoh–Tsujii | N/A | N/A | N/A | N/A | not applicable |

## Deliverable 3: the actual research bet

**Conclusion: `BY at some w is worth prototyping`.**

More specifically: **jumped Bernstein–Yang divsteps with w in the 12–16 range**
now looks like the strongest algorithmic moonshot.

### Why the conclusion changed

The previous B-Y analysis assumed matrix-entry magnitudes near the BY
worst-case bound 2^w. The new empirical matrix-entry survey shows that on
secp256k1-like random low-word states they are much smaller:
- at w=12: max observed entry bits ≈ 8,
- at w=16: max observed entry bits ≈ 11,
with mean much lower.

That changes the reversible cost model entirely. Jumped B-Y no longer pays
`w · n` per matrix coefficient in practice; it pays something closer to
`(8..11) · n`, which **does not scale with w** nearly as badly.

### Estimated cost

If we can implement a reversible jumpdivstep primitive with:
- QROM lookup of the 2×2 matrix from low-w bits,
- matrix apply to (f, g) and coeff trackers,
- exact divide-by-2^w (just k right-shifts because divisibility is guaranteed),

then one inversion pass may cost **~400k CCX** instead of Kaliski's 1.81M.
Two inversion passes would then cost ~800k instead of 3.62M, saving
**~2.8M Toffoli** and taking the full circuit from 4.39M to about **1.6M**.
That would beat Google's quoted 2.1M low-gate point.

### Is this published?

No. A faithful reversible implementation of jumped B-Y would be **novel
research**. The classical algorithm is standard; the reversible realization
is not in the literature.

## Proposed next quantum tasks (deferred proposals only)

Per instruction, I did **not** write quantum code in this session. The
following are proposals for future sessions.

### Proposal P1: reversible jumped B-Y divsteps (w=12)

Primitive sketch:
```
fn by_jump_w12_step(
    b: &mut B,
    delta: &[QubitId],
    f: &[QubitId],
    g: &[QubitId],
    u: &[QubitId],
    v: &[QubitId],
    q: &[QubitId],
    r: &[QubitId],
    hist: &[QubitId],
) {
    // 1. Form address from low 12 bits of (f, g) and sign(delta).
    // 2. Select-swap QROM lookup of precomputed 2×2 matrix M.
    // 3. Apply M to (f, g) and to (u, v, q, r), all modulo p where needed.
    // 4. Right-shift g by 12.
    // 5. Update delta.
    // 6. Uncompute QROM address.
}
```

Open problems:
- exact address format from BY §9,
- reversible select-swap QROM primitive,
- exact sign handling for matrix entries,
- coefficient tracking with guaranteed divisibility.

### Proposal P2: reversible jumped B-Y divsteps (w=16)

Same as P1 but w=16. Fewer jumps, slightly larger matrices. The empirical
entry-size result suggests w=16 may be the sweet spot.

### Proposal P3: hybrid Kaliski-jump

If full B-Y is too risky, adapt only the **jump batching** idea to Kaliski's
state machine: batch 8–12 parity/compare/cswap/sub/halve steps using a matrix
on (u, v_w) while reusing the existing (r, s, m_hist) machinery. Smaller
expected gain (perhaps 10–20%), but less novel than full B-Y.

## Bottom line

After Deliverables 1–3, the recommendation changes from
"B-Y is dead" to **"jumped B-Y is the one remaining believable algorithmic
path to SOTA."**

Not proven, but now clearly worth a real prototype.
