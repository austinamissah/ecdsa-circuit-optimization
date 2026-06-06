# 2026-06-05 Deeper jump (K3/Kj) measured qubit-negative; K2 is the sweet spot

Drew the "deeper jump-GCD step reduction" idea from trailbits/trailmix
(`jump-lowqubit`, Stein trailing-zero removal). The deployed route already runs
**K2** (`DIALOG_GCD_K2=1`: remove up to one EXTRA trailing zero of `v` per
recorded step, flag stored in `k2_shift2_log`). The plan was to generalize to
K3/Kj. Generalized the classical convergence model and measured before any
quantum rewrite. Result: **deeper jump worsens the qubit x toffoli product.**

## Mechanism (why K2 won but K3 loses)

Per the K2 emitter (`mod.rs` ~24890-24911, 27581-27624):
- `GROUP_SIZE = 3` steps/block. Base branch bits = 6 raw, round763-compressed to
  5 (`DIALOG_GCD_HIGH_TAIL_ALIAS_BLOCK_BITS`).
- Each jump level beyond 1 adds one shift flag per step, stored **UNCOMPRESSED**:
  K2 = 5 + 3 = 8 bits/block. Kj = 5 + 3*(j-1).
- Compressed-log size ~= ceil(steps/3) * block_bits, and the log is the largest
  single peak-qubit block (~691q at the 1319q frontier).

## Measurement (`island_search_prefilter ISLAND_MEASURE_JUMP=1`)

Convergence over the 18048 real GCD factors (`dx`, `c`) of the baked tail nonce
120002648. `max_steps` = binding `active_iterations` (every shot must converge).

| depth | max_steps | block_bits | log_bits | rel_steps | rel_log |
|------:|----------:|-----------:|---------:|----------:|--------:|
| 1     | 401       | 5          | 670      | 1.000     | 1.000   |
| 2 (K2)| 259       | 8          | 696      | 0.646     | 1.039   |
| 3     | 227       | 11         | 836      | 0.566     | 1.248   |
| 4     | 213       | 14         | 994      | 0.531     | 1.484   |
| 5     | 207       | 17         | 1173     | 0.516     | 1.751   |

- K1 -> K2: steps -35% at ~flat log (+4%). Huge Toffoli win, ~free on qubits.
  This is exactly why K2 is deployed.
- K2 -> K3: steps only -12% but log +20% (+140q at peak). The Toffoli saving
  (~6-8%; only the per-step body/cswap/comparator scale with steps, the apply and
  fixed overhead do not) cannot offset a ~140q peak rise on the product.
- Even with optimal entropy coding of the shift count (H ~= 1.5 bits/step at d=3
  vs the 2 naive flag bits), K3's log (~757) still exceeds K2's (696). K3 loses.

## Conclusion

K2 is the jump-depth optimum for this score. The quantum K3/Kj emitter is
**contraindicated** (would raise the product). The win condition for deeper jump
would require compressing the shift flags BELOW their ~1 bit/step entropy or
hosting the extra log bits for free at peak; neither is available (round763 is
already 6->5, and shift2 is ~Bernoulli(1/2), incompressible alone).

Reusable tooling added:
- `dialog_gcd_classical_filter::jump_steps_until_zero` /
  `measure_jump_convergence` (pure-number-theory jump-depth convergence model).
- `island_search_prefilter ISLAND_MEASURE_JUMP=1 [ISLAND_MEASURE_JUMP_MAX_DEPTH=k]`
  reports the table above for the baked nonce.

Pivot: the remaining qubit headroom is NOT deeper jump. Attack (a) co-living of
the log with tx/ty (ghost/pebble) and (b) the truncation-island notch loop, now
that the classical pre-filter is validated bit-exact.
