# Apply-swap width analysis, is candidate C (truncate the apply-swap) real?

Read-only correctness investigation. No code changed. Question: can the apply-swap
`for j in 0..n { cswap(swp, x[j], y[j]) }` (`gcd.rs:1336`) be bounded to the GCD live width
`current_n = SCHED_J2[i]` instead of the full 256, **without changing results for any input**?

**Verdict up front: No. Truncating the apply-swap to the GCD live width is NOT safe.** The
swapped registers are the full-256-bit field-element accumulator, whose high limbs are meaningful
and input-dependent, in fact at the end of the sweep they are *provably different*, not equal. The
detailed reasoning follows.

## 1. What `x` and `y` are, and the loop bound

The apply-swap lives inside `apply_step_reverse` / `apply_step_forward`. The loop bound is a
**literal `n = 256`**, not a variable (`gcd.rs:1321` / `1273`):

```rust
// gcd.rs:1310-1348 (apply_step_reverse), phase context 1324-1345
fn apply_step_reverse(circ, i, sub, swp, s2, t1, x_reg, y_reg, dirty_vents) {
    let n = 256usize;                                     // gcd.rs:1321
    ...
    circ.set_phase("tlm_apply_inverse_swap");             // gcd.rs:1334
    if !apply_inv_cswap_skip(i) {
        for j in 0..n {                                   // gcd.rs:1336  <-- full 256
            circ.cswap(*swp, x_reg[j], y_reg[j]);         // gcd.rs:1337
        }
    }
    circ.set_phase("tlm_apply_inverse_mod_sub");          // gcd.rs:1341
    ...
    controlled_mod_sub_vented(circ, sub, &x_reg[..n], &y_reg[..n], Some(k));  // gcd.rs:1345 full 256
}
```

`x_reg`/`y_reg` come from the caller as `apply_inv = Some((xr, yr))` (`gcd.rs:855-866`), which is
the `(y, tmp)` pair passed into `forward_gcd_jump` by `mod_mul_inverse_in_place`
(`gcd.rs:1430` / `1444`). These are the **modular-arithmetic accumulator registers**, each a full
256-bit field element:

```rust
// mod_mul_inverse_in_place, Direction::Inverse, gcd.rs:1424-1435
let tmp = (0..256).map(|_| circ.alloc_qubit()).collect();   // 256 fresh qubits (=|0>)
for j in 0..256 { circ.swap(y[j], tmp[j]); }                // tmp <- original y ; y <- 0
let mut tape = forward_gcd_jump(circ, &mut xv, Some((y, &tmp)));   // xr = y, yr = tmp
```

So at the apply-swap, `x = y` and `y = tmp` are the two 256-bit accumulators tracking the
extended-GCD (Bezout-style) coefficients. The GCD *value* register is the separate `xv`/`v`
(and the internal `u` = the secp256k1 modulus), which is what actually shrinks.

## 2. State of `x[j]`, `y[j]` for `j >= SCHED_J2[i]` at the apply-swap

**Category (d): not provably related, they genuinely differ per input.** Reasoning from the code:

- The accumulators are updated every iteration by **full-width modular arithmetic**: in the same
  apply phase, `controlled_mod_add_k(.., &x_reg[..256], &y_reg[..256], ..)` (`gcd.rs:1281-1288`) or
  `controlled_mod_sub_vented(.., &x_reg[..256], &y_reg[..256], ..)` (`gcd.rs:1345`), plus the fold
  `fused_double_cdouble(circ, s2, y_reg)` (`gcd.rs:1306/1331`) which doubles `y_reg` mod p across its
  full width. Every one of these writes all 256 bits and reduces mod p via the `F = 2^32+977` fold.
  There is no step that confines the accumulator's meaningful content to the low `current_n` limbs.
- The values held are reduced field elements mod p (`p ≈ 2^256`), i.e. essentially uniform 256-bit
  numbers. Their high limbs are ordinary data bits, not structurally zero.
- The two accumulators start *unequal*: after the initial `swap`, `x = y = 0` and `y = tmp = original y`
  (`gcd.rs:1427-1429`), already different in general, and then diverge further under independent
  accumulation.

There is no invariant in the code forcing `x[j] == y[j]` (nor `x[j]==0 && y[j]==0`) for the high
limbs at any iteration. The GCD *value* registers `u,v` provably shrink (which is why `current_n`
tracks them), but the *accumulator* does the opposite, it fills up as coefficients accumulate.

**The width profiles are inverse.** `SCHED_J2[i]` is large for small `i` and small for large `i`
(it follows the shrinking GCD value). The accumulator is *most fully populated* exactly at large
`i` (after ~250 iterations of mod-add/double). So bounding the apply-swap by `SCHED_J2[i]` would
skip the most limbs precisely where those limbs are most certainly meaningful, the truncation is
backwards relative to where the accumulator's information lives.

## 3. Is the accumulator width bounded by any nearby apply-phase op?

**No, the opposite.** Every accumulator operation in the apply phase is explicitly full 256-bit:
- mod-add: `&x_reg[..n]`, `&y_reg[..n]` with `n=256` (`gcd.rs:1284-1285`).
- mod-sub register: `controlled_mod_sub_vented(.., &x_reg[..n], &y_reg[..n], ..)`, `n=256`
  (`gcd.rs:1345`), and inside it `let n = x.len()` operates on the full slice (`gcd.rs:1350-1352`).
- fold: `fused_double_cdouble(circ, s2, y_reg)` over the full `y_reg` (`gcd.rs:1306/1331`).

So the code's own answer to "what is the accumulator's live width at this phase" is **256**, it is
never narrowed to `current_n` anywhere in the sweep. This is strong, direct evidence that the
accumulator's meaningful width is the full 256, unlike `u`/`v` which are sliced to `current_n` at
`gcd.rs:768,791-792,799,827-828`.

Confirming end-state (`gcd.rs:1431-1435`, `1445-1448`): after the sweep, `tmp` must return to `|0⟩`
(`zero_and_free`), while `y` (the other accumulator) holds the inverse result. So at the final
iterations the two registers are being driven to *different* full-width values (one → 0, one → the
256-bit inverse), a concrete case where their high limbs are provably **un**equal.

## 4. "Happens to be equal" vs "provably equal", the honest answer

I cannot establish from the code that `x[j] == y[j]` for `j >= SCHED_J2[i]` on all inputs. It is
not merely "unprovable from a static read", the code gives positive evidence of the *opposite*:
the accumulators are two independent reduced field elements updated at full width, initialized
unequal, and ending at different full-width values (result vs 0). A conditional swap
`cswap(swp, x[j], y[j])` on a high limb `j` is therefore a **real data operation** whenever
`swp = 1` and `x[j] != y[j]`, which does occur for some inputs. Dropping those cswaps would leave
the high limbs of the two accumulators unswapped while the low limbs were swapped, corrupting the
modular result for those inputs. Such a bug would likely pass casual testing (it only manifests when
`swp=1` and the specific high limbs differ) but is a genuine, input-dependent correctness failure.

## 5. Is there a width `w(i)` above which the limbs are provably equal/zero?

No such `w(i) < 256` exists based on the code. The provable width of the accumulator at the
apply-swap is the full **256** for every iteration `i`; it has no relationship to `SCHED_J2[i]`.

What *is* already exploited is a different, coarser and sound optimization: the **whole-iteration**
skip. `apply_fwd_cswap_skip(i)` / `apply_inv_cswap_skip(i)` (`gcd.rs:156-174`) drop the *entire*
256-limb swap for the last `N` iterations (`i + N >= ITERS`), driven by
`TLM_APPLY_FWD_CSWAP_SKIP_LAST` (submission sets it to `2`, `mod.rs`) and
`TLM_APPLY_INV_CSWAP_SKIP_LAST` (unset ⇒ 0). That is valid when the swap *control* `swp` is provably
inactive (or the swap is otherwise a proven no-op) for those tail iterations, an all-or-nothing
claim about the control, **orthogonal** to per-limb width truncation. It does not generalize to
"truncate the width," because when the swap does fire it must fire across all 256 limbs.

## Conclusion for candidate C

Candidate C as stated, *bound the apply-swap to `current_n = SCHED_J2[i]`*, is **not real**: the
swapped registers are the full-width field-element accumulator, its high limbs are input-dependent
and provably not universally equal, and no nearby bound narrows it. The only sound lever in this
neighborhood is the existing whole-iteration control-is-dead skip (already applied to the forward
tail), which is a different optimization and does not truncate width. Any width-truncation attempt
here should be treated as a correctness change requiring a proof about the accumulator's high bits
that the current code does not support, and the code in fact contradicts.
