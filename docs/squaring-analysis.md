# Modular squaring: is it at its floor?

Read-only. No source changed. Target: modular squaring, ~4.5% of the Toffoli budget (~62,081 CCX
across its phases), via `mod_square_sub_pm_secp256k1_symmetric` (square.rs:471) and the live backend
`square_addsub_vented` (arith/multiply.rs:2087). Question: untaken slack, or already at its floor?

## Bottom line

The squaring is **much closer to its floor than "a deprioritized target with untaken slack" would
suggest.** It already uses all the standard heavy optimizations: **Karatsuba** (3 half-size squarings
instead of one full 256-bit), **full symmetry** exploitation (strict upper-triangle, n(n−1)/2 cross
terms, each once), **measurement-vented** cross-term uncompute (0-Toffoli AND clears), and a
**NAF-minimal** F-fold reduction (weight-5, the minimum). Symmetry, diagonal, and reduction are all
at or near their floors, there is **no provably-removable slack** (no dead gates, no un-exploited
symmetry, no redundant reduction pass). Two rewrite ideas were investigated and **both fell through**
(see §7): (a) enabling the env-disabled F-fold schedule **regressed by +3,458 Toffoli and failed
correctness** when measured; and (b) the ~28K compute/uncompute round-trip turns out to be **mostly
not harvestable**, only the single-use `c` term benefits from going in-place, so the realistic
ceiling is **~6–9K, not ~28K** (the earlier ~28K figure wrongly lumped all three unbuilds; the two
dual-use terms tie, per the §7 algebra). **Honest harvestable estimate: effectively 0 at low risk;
~6–9K only via a speculative in-place rewrite of the `c` term that may not net positive after
inline-fold overhead.**

## 1. What it computes, step by step

`λ = hi·2^128 + lo`. Karatsuba: `λ² = a + (c−a−b)·2^128 + b·2^256` where `a=lo²`, `b=hi²`,
`c=(hi+lo)²`, and `2^256 ≡ F = 2^32+977 (mod p)`. The routine subtracts `λ² mod p` into `output_reg`
via signs `−c·2^128 − a + a·2^128 + b·2^128 − b·F` (square.rs:471-510). It keeps **one partial-product
register alive at a time** (build → apply → unbuild per prod, square.rs:479-507), a deliberate
peak-qubit cap (1152) that shapes the whole cost structure.

Per half-square, `square_addsub_vented` (multiply.rs:2087):
- **Cross terms (symmetry):** `for i in 0..n { controlled_add_subtract_vented_borrowed(x[i+1..n], prod@2i+1, ctrl=x[i], borrowed carries) }` (multiply.rs:2090-2101). Each unordered pair `x_i·x_j` (j>i) is formed **exactly once** as a controlled add at weight `2^{i+j}`, no explicit CCX products; the Toffolis are the adder carry chains (`cuccaro_add_fast_borrowed_carries`, one CCX/bit forward).
- **Diagonal + doubling:** `square_corr_forward` (multiply.rs:1932), three full Cuccaro add/subs placing `x_i` at bit `2i` (the `a_i²=a_i` diagonal) and correcting the ×2 of the cross terms. Cost `10n−6` CCX, linear.
- **Venting:** cross-term carry ancillas cleared by `hmr` + `cz_if` (adder.rs:104-112), **0 Toffoli** uncompute (halves cost vs the non-vented `square_addsub_local`).

Build cost per half-square: **n(n−1)/2 + 10n − 6** ⇒ **9,402** (n=128: lo, hi) / **9,540** (n=129: sum).

Phase-cost map (from the TRACE_TLM_CCX measurement):

| phase | CCX | what |
|---|---|---|
| `square_c_sum_build` / `_unbuild` | 9,540 / 9,540 | `c=(hi+lo)²` build + full-inverse unbuild |
| `square_a_lo_build` / `_unbuild` | 9,402 / 9,402 | `a=lo²` build + unbuild |
| `square_b_hi_build` / `_unbuild` | 9,402 / 9,402 | `b=hi²` build + unbuild |
| `square_b_hi_apply_f_times_sub` | 4,329 | `−b·F` reduction (5 NAF passes; incl. 32-bit wrap) |
| `square_c_sum_apply_shifted_128_sub` | 1,064 | `−c·2^128` combine |
| (other applies / sum build) | tail | `±a`, `+a·2^128`, `+b·2^128`, `sum=hi+lo` |
| **total** | **~62,081** | |

**Build + unbuild ≈ 56,688 CCX ≈ 91% of the squaring budget.** The combine/reduction is only ~5–6K.

## 2. Symmetry, fully exploited (at floor)

The loop operand is `x[i+1..n]` (strictly j>i) and each pair is added once (multiply.rs:2090-2101).
Leading Toffoli term is exactly **n(n−1)/2**, the theoretical symmetric cross-term count. Measured
build 9,402 vs the 8,128 cross floor for n=128; the +1,274 gap is the `10n−6` linear diagonal/doubling
correction, not redundant products. **No slack: symmetry is fully realized, and it's done via vented
controlled-adds (~1 CCX/pair) rather than a raw CCX per pair.**

## 3. Reduction (F-fold), NAF-minimal, one sub-floor wrinkle

`F = 2^32 + 977` is folded via `F_NAF_TERMS` (square.rs:14-20) = **5 signed terms**
`{(0,−),(4,−),(6,+),(10,−),(32,−)}`, i.e. `977 = 2^10 − 2^6 + 2^4 + 2^0` (NAF weight 4, below the
binary weight 6) plus `2^32`. **This is the minimum-weight representation, the F-multiply uses no
redundant or repeated passes, and no fold control is provably zero** (a,b,c high halves genuinely
reach bits 255–257, e.g. `(2^128−1)² = 2^256−2^129+1`, so nothing is structurally skippable).

The one spot **above** the absolute floor: in the `−b·F` term, `apply_shifted_hi_term` (square.rs:43)
folds the 32 overflow bits of the `shift=32` NAF term **one at a time** with 32 separate F-windows
(square.rs:65-71), the dominant contributor to the 4,329-CCX `square_b_hi_apply_f_times_sub` phase.
**Cheaper schedules for exactly this already exist in the code** (`TLM_SQUARE_F_RAMP10_DIRECT32`,
`TLM_SQUARE_F_DIRECT_TAGS`, `apply_shifted_value_direct`, square.rs:302-375) **but are disabled by
default.** This is the single most accessible slack (see §6).

## 4. Diagonal terms, not wasteful, near floor

`a_i² = a_i` is never computed as a CCX product. In the live backend it is merged into the O(n)
`square_corr_forward` correction adders (multiply.rs:1932-1966, `10n−6` CCX), linear, not quadratic.
(The free-CX diagonal `cx(x[i], row[0])` exists only in the dead schoolbook fallback, square.rs:117.)
So the diagonal is handled at linear cost, not as `n` wasted Toffoli products. **No meaningful slack.**

## 5. Distance from the theoretical floor

A correct symmetric modular square of a 256-bit value has an irreducible cross-term core of
~n(n−1)/2 AND-equivalents; Karatsuba trades that for 3 half-squares ≈ 3·(128·127/2) ≈ **24,384**
cross-term Toffolis, plus O(n) diagonal correction and the mod-p reduction. The **build** side
(~28,344 CCX) sits right at that Karatsuba floor (24,384 + 3·(10·128−6) ≈ 28,206). The reduction/
combine (~5–6K) is near its NAF floor. **The one component with no theoretical justification is the
unbuild (~28,344 CCX)**, it exists purely to return the temporary `prod` registers to |0⟩, i.e. it
is the reversibility "compute–copy–uncompute" tax, not arithmetic. So the implementation is
**~2× its arithmetic floor**, and essentially all of the excess is the uncompute round-trip.

## 6. Honest yield estimate

**Provably removable right now (skip/tune/table): ~0.** No dead gates, no un-exploited symmetry, no
redundant reduction pass, no zero-control that isn't already forced. The algorithmic optimizations are
all present. This is *not* a soft target sitting at 2× waste for lack of attention.

**~~Accessible, low-risk (a schedule flip): ~1–3K CCX.~~, RETRACTED, MEASURED AS A REGRESSION.**
The env-gated F-fold alternatives (`TLM_SQUARE_F_RAMP10_DIRECT32` / `TLM_SQUARE_F_DIRECT_TAGS`,
square.rs:302-375) were *believed* to be a cheaper "untaken" lever. **Measurement disproved this**
(see §7): enabling `TLM_SQUARE_F_RAMP10_DIRECT32=1` raised the emitted Toffoli count by **+3,458**,
pushed peak to **1153 > 1152**, and **failed correctness** (9,013/9,024 shots wrong). The guard
`value.len()+32 <= N` also excludes the full-width `−b·F` term, so it never reached the intended wrap
anyway. The default `SHIFTED_LOW` schedule is correct and cheaper. **This lever is a dead end.**

**Combine fusion (would need a small rewrite): ~2–4K CCX, width-traded.** The middle coefficient is
applied as three separate shifted combines (`−c·2^128`, `+a·2^128`, `+b·2^128`, square.rs:483/494/503)
instead of forming `m = c−a−b` once and applying `m·2^128`, which would cut those combines (and their
high-F-folds) from 3 to 1. But it needs a live 256-bit scratch + adder, and peak is already at the
1152 cap, so this is a Toffoli/width trade, not a free win.

**~~The big fat (~10–22K CCX)~~, REVISED DOWN to ~6–9K.** The ~28,344-CCX unbuild is the
reversibility round-trip, but **only ~9.5K of it is harvestable**, because the benefit of going
in-place depends on how many times each square is *used* in the combine (see §7 algebra):
- **c = (hi+lo)²** is used **once** (`−c·2^128`, square.rs:483) → in-place eliminates its unbuild
  (~9,540) → the only real win.
- **a = lo²** (used twice, square.rs:492/494) and **b = hi²** (used twice, square.rs:503/505) →
  in-place **ties** materialize-and-reuse, no saving (a dual-use materialized square costs
  build+2·apply+unbuild = 2× the square; the in-place alternative recomputes it per position = also
  2×). Even folding the two uses into one `a·(2^128−1)` pass places each cross term twice.

So the realistic ceiling is **c's unbuild ~9,540, minus the inline-F-fold overhead** the direct path
adds to fold sum²'s bits ≥ 256 → net **~6–9K (~0.5–0.7% of the 1.32M budget)**. It is still **not**
removable by measurement-uncompute (HMR fixups on a multi-bit *arithmetic* register cost ~the same;
free only for single ANDs, already used). Requires a new reversible primitive
(`controlled_square_accumulate_shifted_folded`) with **no dedicated unit self-test** (`TLM_SQ_SELFTEST`
only covers the materialized backend), validated only by the ~4-min build+eval cycle. Genuine
correctness risk; net could still come out ≤0 after fold overhead.

### Verdict

Squaring is **not** at a hard floor, but the low-hanging algorithmic fruit is already taken, and the
two rewrite ideas were investigated and did not pan out:
1. **F-fold schedule flip, REJECTED (measured):** +3,458 Toffoli, peak 1153 > 1152, correctness
   FAILED. Dead end (§7).
2. **In-place c-term square, not attempted, ceiling revised to ~6–9K** (down from ~28K, since the two
   dual-use terms tie). A risky reversible-arithmetic rewrite with no fast validation loop for ~0.6% of
   budget, poor effort/reward; deprioritized.

Everything else (symmetry, diagonal, NAF reduction) is already at its floor; don't spend math there.
The GCD engine (~95% of the budget) is where any real leverage would be, and it is already at its
frontier.

## 7. Experiment log & corrected algebra

**F-fold schedule experiment (`TLM_SQUARE_F_RAMP10_DIRECT32=1`, run 2026-07-11).** Self-test passed
(`SQ_SELFTEST` bit-exact vs classical x², inverse drains clean), but it covers only the
partial-product backend, not the F-fold schedule. Full build + eval vs baseline (1,320,763 avg
Toffoli, 1,152 qubits):

| metric | baseline | flag ON | Δ |
|---|---|---|---|
| emitted CCX (pre-CONSTPROP) | 1,398,456 | 1,402,219 | **+3,763** |
| emitted CCX (post-CONSTPROP) | 1,398,187 | 1,401,645 | **+3,458** |
| peak qubits | 1,152 | **1,153** | +1 (over cap) |
| eval correctness | OK | **FAILED** (9,013/9,024 mismatch) | n/a |

The `RAMP10_DIRECT32` path routed the `a_hi`/`c_hi` folds through `mod_double + apply_full_width`,
which is *more* expensive than `SHIFTED_LOW` (`square_a_lo_apply_shifted_128_add` and
`square_b_hi_apply_shifted_128_add` each jumped to ~2,480; `square_c_sum_apply_shifted_128_sub`
1,064→1,783), while barely touching the intended `square_b_hi_apply_f_times_sub` (4,329→4,272,
because the 256-bit `b` value fails the `value.len()+32 <= N` guard). Net: a regression *and*
incorrect. The alternative schedules are env-disabled because they are wrong/costlier in this
configuration, not an untaken win.

**Dual-use algebra (why the unbuild ceiling is ~9.5K, not ~28K).** For a square used `k` times in the
combine: materialize = build + k·apply + unbuild = (2 + small)·(square cost); in-place = k·(direct
compute) = k·(square cost). In-place wins **only when k = 1** (1 vs 2). c is k=1 (win ~9,540); a and b
are k=2 (2 vs 2, tie). Hence only c's unbuild is harvestable. My original §6 lumped all three unbuilds
(~28,344) into the "prize," which was wrong, the honest ceiling is c's ~9,540 minus inline-fold
overhead.
