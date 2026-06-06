# Architectural insight: s = p at end of forward Kaliski

## Discovery

Classical simulation verified that, after our specific Kaliski variant runs
for 407 iterations (our pair1_iters), the state is:
- u = 1
- v_w = 0
- r = -inv_raw (the negated Montgomery inverse, scaled by 2^511 mod p)
- **s = p** (exactly the secp256k1 prime, as a 256-bit integer)

This matches HRSL 2020's observation: "After the inversion, three registers
contain known values of 0, 1, and the modulus p."

## Implementation

`with_kal_inv_raw` in `src/point_add/mod.rs` now:
1. After forward Kaliski: X-flips bits of p to zero out s, then frees s
   (256 qubits freed during body, ~128 X gates, 0 Toffoli).
2. Before backward Kaliski: re-allocates s, X-flips bits of p to load.

Default: enabled (KAL_FREE_S env var defaults to on, set to "0" to disable).

## Effect

**Body peak reduction**: `pair1_mul1`, `pair1_mul2`, `pair1_kaliski_backward`
body sites had 256 fewer qubits alive. Peak dropped from ~2460 to ~2204
at those sites.

**Global peak NOT reduced**: backward Kaliski's `bk_step6_7_8` phase
still hits 2716 because s is alive during backward (re-allocated +
loaded with p).

## Why global peak doesn't drop

Backward Kaliski reverses forward's modifications to (u, v, r, s, m_hist).
s IS modified during backward (reverse of `s += r if add_f`), so s must
be quantum-live during backward.

To reduce global peak, we need to ALSO reduce backward. Options:
- Venting halve (tried, drops backward to ~2460 but costs Toffoli).
- Different backward algorithm that doesn't need full Kaliski state live.
- Luo-style register sharing (big rewrite).

## Current status

Baseline (KAL_FREE_S=1 default, no venting): 4.18M Toffoli / 2716 qubits.
With venting halve+modadd: 4.70M / 2714q.

The s=p insight is architectural groundwork. Doesn't improve primary
metric (Toffoli) alone but enables future qubit-first optimizations
when combined with backward-peak reduction.

## Verification

`/tmp/our_kaliski.py` (Python reference): 5 random trials, all show
s = p after iters=407. Also at iters=511 (upper bound).
At iters=256 or 350: s != p (Kaliski hasn't fully terminated).
