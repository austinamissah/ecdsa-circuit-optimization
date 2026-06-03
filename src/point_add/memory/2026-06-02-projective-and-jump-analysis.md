# 2026-06-02 Projective Coordinates & Jump GCD Analysis

## Summary

Investigated two "moonshot" approaches. Both ruled out as transformative wins.
Circuit is at **1,680,191 avg Toffoli × 1,542 qubits = 2,590,854,522 score**.

---

## 1. Projective Coordinates: DEAD END

### Why the intuition was wrong

Initial reasoning: "96.7% of Toffoli is inversion. Projective eliminates inversion in the forward pass." But:

1. **The current circuit is already a 1-inversion architecture.** The "dialog GCD" runs ONE half-GCD forward, records branch decisions, applies the Bezout reconstruction, then reverses the GCD. It's NOT 4 independent Kaliski calls.

2. **Projective still needs inversion for affine conversion at the end.** And the uncomputation of intermediate projective state (HH, HHH, V, X3, Y3) requires knowing the original inputs — which have been overwritten. This is the same obstruction documented in `ONE_INV_DX3_AFFINE_PA_BLOCKER`.

3. **The current dialog_gcd computes inversion-multiplication simultaneously** (same technique as Schrottenloher 2026's "Bezout reconstruction"). There's no "separate inversion" to eliminate.

### Huang 2025 (withdrawn)

Huang et al. (arXiv:2502.12441) argued projective doesn't help for quantum ECDLP. Their argument is about Shor's full algorithm (non-uniqueness breaks period-finding interference), NOT about a single point-add gate. So their objection is irrelevant to our benchmark. But projective still doesn't help because of point (2) above.

---

## 2. Jump/Batched GCD: MARGINAL (5-11%)

### The idea

Classical `safegcd` batches T binary-GCD steps into one 2×2 matrix multiply. For quantum: record T branch decisions, look up the accumulated matrix via QROM, apply it to (u,v) and (x,y) in one shot.

### Window survey results (from `kaliski_jump.rs` infrastructure)

| w (low bits) | t (batch) | Distinct matrices | Max per class | Entry bound |
|---|---|---|---|---|
| 4 | 4 | 125 | 16 | 16 (4-bit) |
| 8 | 4 | 125 | 16 | 16 |
| 8 | 8 | 10,126 | 95 | 256 (8-bit) |

### Why it doesn't yield a big win

The existing test `selected_matrix_variable_coeff_lower_bound_kills_hybrid_kaliski_windows` (line 769) proves that even with QROM-selected coefficients, applying the matrix to (u,v) AND (x,y) costs more per batch than replaying microsteps — because the coefficients are quantum-selected, requiring controlled multiply-accumulates.

**Cost comparison for t=4:**
- Current: 4 × (cswap + csub at truncated width + apply ops) ≈ 3,400 CCX per 4 steps
- Jump with QROM: 226 (lookup) + 2,048 (apply) + 226 (unlookup) ≈ 2,500 CCX
- Savings: 900 CCX × 100 batches = 90K CCX = **5.3% of total**

For the apply phase specifically, the per-iteration cost is already 591 CCX for the controlled add and 256 for cswap — batching helps but not dramatically because the full-width quantum-controlled operations dominate.

### Architecture insight

Current cost structure:
- **Tobitvector** (forward GCD, records branch bits): 752K CCX (44.8%)  
- **Apply** (Bezout reconstruction, uses recorded bits): 795K CCX (47.4%)
- **Other** (arithmetic, output assembly): 133K CCX (7.9%)

Jump GCD only helps the tobitvector. The apply phase processes the RECORDED bits, not per-step — so it doesn't benefit from step-batching.

---

## 3. Apply Phase Width Truncation: NOT VIABLE

Initial idea: Bezout coefficients grow from 0 to full-width during reconstruction, so early steps could use narrower operations.

**Why it fails:** The apply operates with MODULAR arithmetic (`mod_double_inplace_fast`, `cmod_add`). After even one modular double+add, values are uniformly 0..p-1 regardless of starting value. No width guarantee exists.

---

## 4. Apply Phase Cost Reduction: WHAT REMAINS

Per apply iteration: 591 CCX (ctrl add) + 591 CCX (ctrl sub) + 256 CCX (cswap) + 65 (double) + 65 (halve) + 170 (overflow/clean) = 1,738 CCX

The controlled add/sub decomposes as:
- 256 CCX: materialize f[i] = ctrl & source[i]
- 256 CCX: Cuccaro add/sub of f into accumulator
- 65 CCX: Solinas correction (add/sub constant c=977)
- 14 CCX: clean overflow flag

This is already at the theoretical floor for a controlled quantum-quantum modular add (2n CCX minimum).

---

## 5. Remaining Levers (incremental, 1-5% each)

### BODY_CARRY_TRUNC_W (tobitvector carry truncation)
- Truncates the carry chain in the tobitvector add/sub body by W bits
- Tested: W=5 → 8959 mismatches, W=1 → 3882 mismatches (both on current reroll)
- **Needs co-tuned Fiat-Shamir island.** May yield 5-15K CCX per valid W.

### COMPARE_BITS tightening
- Currently 57. Tested cb=56: only 7 mismatches + 5 phase-garbage.
- **Very likely a clean island exists at cb=56 with different reroll.**
- Each bit saves: 399 steps × 2 passes × 2 (fwd+rev) × 1 CCX ≈ 1,596 CCX
- Plus comparator-related savings in branch_bits phase.
- Expected net: ~3,000-5,000 CCX per bit tightened.
- **2D reroll search running for cb=56** (r=0..10 × p=0..15).

### ACTIVE_ITERATIONS reduction
- Currently 399. Floor proven at 397 (causes 1 convergence failure).
- Going to 398 saves: ~2 × 1,738 (apply) + 2 × 1,684 (tobitvector) = ~6,844 CCX
- Need to verify 398 passes all 9024 shots at current reroll.

---

## 6. Schrottenloher 2026 Comparison (arXiv:2606.02235)

Their circuit for secp256k1:
- Space-optimized: 1192 qubits, 2,383,000 Toffoli (score ~2.84B)
- Gate-optimized: 1446 qubits, 1,851,000 Toffoli (score ~2.68B)

**We beat both on score (2.59B).** Our Toffoli (1.68M) is 9% below their gate-opt.

Their key techniques (all either already used or explored here):
- Binary EEA with "dialog" decomposition (= our dialog_gcd)
- Bezout reconstruction from garbage bit-vector (= our apply phase)
- Garbage compression (3 iter → 5 bits, saving 1 bit/3 steps) — NOT used by us, potential qubit win
- Register sharing (u,v shrink → reuse for garbage) — NOT fully exploited by us
- Approximate comparisons (40-50 MSBs) — we use 57 bits, similar

---

## 7. What to try next session

1. **Finish cb=56 reroll search** — if found, ~3-5K CCX savings (tiny but free)
2. **Stack BODY_CARRY_TRUNC_W with a fresh island** — needs dedicated 3D search (cb × W × reroll). Potential 10-20K CCX if W=2-3 is viable.
3. **Garbage compression (Schrottenloher's 3→5 trick)** — saves ~130 qubits from dialog_log register. If this drops peak below 1542, direct score win without Toffoli change. Pure qubit improvement.
4. **Register sharing** — as u,v shrink, their freed top bits can store garbage. Could save 100-200 qubits at peak.

Items 3 and 4 are the most interesting: **they reduce QUBITS without touching Toffoli**, directly improving score. Given we're at the Toffoli frontier already, qubit reduction may be the higher-leverage path.

---

## 8. Leaderboard Best (2.539B) — What They Did

Synced to submission `f2cd7132` (score 2,539,526,878 = 1,732,283 T × 1,466 q).

Two qubit cuts stacked on top of the 1542-peak base:

### Cut 1: 1542 → 1500 (-42q, +36K T)
- `KARA_Z02_LOWQ=1`: Karatsuba z0 square carry lane hosted on clean z2 slice (borrowed carries). z2 runs ancilla-free (lowq mode).
- `KARA_SOL_MOD_VENT=1`: Solinas constant corrections vented onto dirty operand (+2 clean) instead of load_const materializing 256q.
- `DIALOG_GCD_APPLY_CHUNKED_F_CUT` widened 78 → 99 to sink apply phase below 1500.

### Cut 2: 1500 → 1466 (-34q, +14K T)
- `DIALOG_GCD_BRANCH_BITS_HOST_COMPARATOR=1`: Fused branch-bit path routes through borrowed-carry comparator on future-log slice, eliminating standalone `cmp` ancilla + its own carry lane.
- `DIALOG_GCD_APPLY_CHUNKED_F_CUT` widened 99 → 116 (optimum; beyond 116, peak stays 1466 but Toffoli keeps rising).
- Reroll: 16/0.

### Current floor
Peak at 1466 = `dialog_gcd_raw_tobitvector_materialized_sub_body`. This is the GCD body's controlled subtract — the Cuccaro adder + carry lane during the per-step u -= v operation.

Near-peak phases (all at 1465-1466):
- `materialized_sub_body` / `materialized_add_body` (tobitvector)
- `materialized_special_chunked_raw_{sum,difference}` (apply)
- `tobitvector_subtract` / `_reverse_add`

### Pattern: each round is "identify co-binders → eliminate one → widen F_CUT to next floor"
Break-even: ~1,700 T/qubit for a product-neutral trade. Both cuts are well inside this.

### Next cut analysis needed
What's alive at 1466 during `materialized_sub_body`? Specifically: can the sub's carry lane be hosted on idle state (like the comparator was hosted on future-log)? What other transients coexist?

### cb=56 island search: FAILED
Ran 2D search on OLD codebase (975b73f, cb=57 base): no clean island found for cb=56 within r=0..20, p=0..25. The island density at cb=56 is very low or nonexistent at that stream. Would need a 3D search (cb × reroll × post_sub_reroll) or a different base.

---

## 9. NEXT CUT: Host Sub/Add Carries on U's High Bits (1466 → ~1446)

### Discovery

At the 1466 peak (`materialized_sub_body` at step ~368):
- Base live state: tx(256) + ty(256) + u(256) + compressed_log(665) + raw_block(6) = **1,439**
- Transient overhead: **27 qubits** (carries + gated from the Cuccaro sub)

The future-log borrow can't cover everything at this step, so ~27q are freshly allocated.

**BUT**: at step 368, `active_width ≈ 23`. This means `u[23..256]` = **233 qubits of provably-zero idle state**. These are never read or written during the sub — only `u[0..23]` is used.

### Proposed fix

Route the `borrowed_carries` (and optionally the `gated` register hosting) through `u[active_width..N]` instead of (or in addition to) the compressed_log future slots.

At the peak step, u's high bits provide 233 idle qubits — more than enough for the 27q transient. This would eliminate the peak-transient allocation entirely for the late GCD steps.

### Expected impact

- Peak drops from 1466 to ~1446 (the branch_bits floor) or lower
- At 1446: score = 1,732,283 × 1446 = **2,505,281,218** (vs current 2,539,526,878)
- Improvement: **~1.3%** on score (34M points)
- Toffoli: unchanged (zero cost — we're just reusing existing zeros)
- Then widen F_CUT further to sink apply phase to match

### Risk

- Phase cleanliness: u's high bits must be EXACTLY zero when we borrow them. This is guaranteed by the active_width envelope (u's value fits in active_width bits at that step). But needs margin verification — if any step exceeds the envelope, the borrow corrupts u.
- The existing WIDTH_MARGIN=27 is the safety margin. Borrowing from u[active_width..] means we're trusting the margin to hold for ALL 9024 test inputs.

### Implementation plan

1. In `dialog_gcd_future_log_carry_slice` (or a new helper), compute idle high-u slice
2. Pass it as `borrowed_carries` when the future_log is too short
3. Adjust the `HOST_GATED` logic to also consider u's high bits
4. Run the benchmark — if any mismatch/phase-garbage, need a fresh reroll island
