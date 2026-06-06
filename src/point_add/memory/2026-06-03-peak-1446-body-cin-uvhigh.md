# 2026-06-03 PEAK 1466 -> 1446 (body c_in host + uv-high carry borrow)

Validated 0/0/0 over 9024 via the real `eval_circuit`. Score 1466 x 1,732,283 =
2,539,526,878 -> **1446 x 1,740,263 = 2,516,420,298 (-23,106,580, -0.91%)**.

## What pinned 1466
Four co-bound phases: the two apply mod add/sub
(`materialized_special_chunked_raw_{sum,difference}`) and the two GCD-body
add/sub (`raw_tobitvector_materialized_{add,sub}_body`). F_CUT had been tuned
(=116) to sink the apply pair to the *body* floor, so the body tier was the
true binder.

## Two value-exact carry-lane reclaims (no width-truncation lottery)
- **DIALOG_GCD_BODY_HOST_CIN=1**: `add/sub_nbit_qq_fast_borrowed_carries`
  allocated a FRESH `c_in` even though the carry lane was borrowed from the
  future-log -- that single ancilla pinned the body at 1466. With the odd-u
  fastpath `body_start=1`, `gated[0]` is never loaded/cleared (stays |0>) and is
  disjoint from operands + carries, so it serves as the Cuccaro carry-in. Body
  phases 1466 -> 1446. Exact: c_in=0 is the carry-in either way, restored to |0>.
- **DIALOG_GCD_LATE_BORROW_UV_HIGH=1**: at late steps the *compressed* future-log
  region has shrunk below 2*active_width-1, so the body fell back to allocating
  its own carry+gated lane (the 1465 `tobitvector_subtract`/`_reverse_add` marker
  tier, +~19q). The GCD has converged there, so `u[active_width..]` is |0> by the
  SAME premise the width truncation already relies on, and is already allocated.
  `dialog_gcd_pick_borrow_slice` falls back to `u[active_width..active_width+2n-1]`
  as scratch (disjoint from `u[..active_width]` and the `v` accumulator). Marker
  tier 1465 -> 1446. No new failure modes (any input with nonzero u-high already
  fails the truncation; confirmed ancilla-garbage=0).
- **DIALOG_GCD_APPLY_CHUNKED_F_CUT 116 -> 126**: with the body floor at 1446,
  widening the cut sinks the apply pair to 1446 (their min; F_CUT>126 rebalances
  the other block upward -> 1447+). 126 = apply minimum.

## Island
New op stream re-rolls Fiat-Shamir. 2-D reroll search (~0.6% clean density, like
the prior streams) lands **DIALOG_REROLL=7, DIALOG_POST_SUB_REROLL=13** clean
0/0/0 over all 9024. Co-tuned in `configure_ecdsafail_submission_route`.

## Next floor
Peak 1446 is now bound SOLELY by the apply pair; next tier is 1439 (the
ipmul/quotient `*_reacquire_terminal_u` + `pair1_quotient`/`pair2_product`
phases). Apply min via F_CUT is 1446, so reaching 1439 needs the apply transient
narrowed structurally (e.g. 3-block chunking) AND then those 4 ipmul/quotient
phases cut. See [[2026-06-02-cswap-floor-analysis]] for the deeper algorithmic
levers (safegcd/jump-GCD to kill per-step cswaps).
