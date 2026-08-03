# Census re-mine at `TLM_SCHED_J2_DELTA=2` — RESULT

Superseded the incomplete handoff of the same name. The re-mine is done and
harness-verified.

## The four numbers

| | avgT | peak q | score | vs head |
|---|---|---|---|---|
| head (shipped, delta 0, shipped strip) | 1,289,073 | 1154 | **1,487,590,242** | — |
| delta 2, strip OFF | 1,319,429 | 1155 | 1,523,940,495 | **+2.444%** |
| delta 2, **re-mined strip ON** | 1,307,877 | 1155 | **1,510,597,935** | **+1.547%** |

**The re-mined strip recovers 13,342,560 of score — 0.897% of head.**

**λ (n=400 per arm, same instrument that measured the shipped strip's 0.682):**

| arm | λ_classical | sem |
|---|---|---|
| delta 2, strip OFF | 5.787 | ±0.125 |
| delta 2, re-mined strip ON | **5.777** | ±0.115 |

**Δλ = −0.010 ± 0.170 — statistically zero.** The shipped table costs
+0.682 ± 0.273 λ on its own stream; **the re-mined table costs nothing
measurable. The re-mine does recover the 0.682 λ.**

## Harness verification (read directly, not from a driver)

Both arms built and evaluated with `build_circuit` + `eval_circuit`:

| arm | ops | qubits | classical | phase | ancilla | stale keys | md5 ops.bin |
|---|---|---|---|---|---|---|---|
| strip OFF | 9,214,624 | 1155 | 6 | 5 | 0 | — | `baac874cfdd26ec6b7f25ac15cb6a9dc` |
| re-mined strip ON | 9,204,392 | 1155 | 7 | 6 | 0 | **0** | `4991360767a0f364a146b039de3f2d65` |

Both sit in the intrinsic band, and **0 stale keys** is the load-bearing number:
the old table applied to this same delta-2 stream discards 13,484 keys and takes
the circuit to 9,022/9,024 mismatches. The re-mined table addresses every gate
it names.

## Why the λ came back, and what the held-out shards showed

Census: 120 M random on-curve pairs, 12 independent seeds, `--lanes 64`, at
`TLM_SCHED_J2_DELTA=2` with `SUB4_APPLY_STRIP=0`. Emitting from the 10 mining
shards (100 M) and then re-emitting with the 2 held-out shards (120 M):

| | dead | downgrade |
|---|---|---|
| mining only, 100 M | 10,364 | 2,206 |
| with held-out, 120 M | 10,232 | 2,169 |
| **caught by 20 M of held-out data** | **132** | **37** |

169 false keys per 20 M samples. A Good–Turing reading of that rate puts the
residual error of the 120 M table at ≈ 169/20e6 × 9024 ≈ **0.08 λ**, which is
consistent with the measured −0.010 ± 0.170.

**This corrects the pessimism in my earlier handoff.** I predicted a 120 M
re-mine would cost 2–3× the 0.682 λ it replaces, from
`λ ≈ (dead keys) × 3/N × 9024` = 2.3. That formula assumes every dead key sits
at the detection threshold. Most do not — they are structurally dead with
p = 0 — so it is a wild overestimate. **The held-out measurement is the right
estimator and it says the opposite.** Census depth mattered far less than I
argued; 120 M was ample.

The re-mined table is smaller than the shipped one (10,232 dead vs 12,543;
2,169 downgrade vs 3,923), which is why it recovers 0.897% rather than the
1.16% the shipped strip is worth on its own stream. Part of that is genuine
census depth (120 M vs 320 M); part is that the delta-2 geometry simply has
fewer dead gates.

## Net position

Delta 2 with the re-mined strip is **+1.547% of score against the head**, and
buys λ_total 20.04 → ~8 (see `lambda-levers.md`). The strip is now free on the
λ axis, so the whole 1.547% is the price of the λ lever itself.

## Not done

- The table is committed as **data only**
  (`data/deep-strip-keys-delta2-120M.rs.gz`), not installed into
  `src/point_add/deep_strip_keys.rs`. It is mined against a delta-2 stream and
  would corrupt the shipped delta-0 circuit. The repo table is untouched.
- A delta-0 control re-mine, which would check the tool against the shipped
  12,543 / 3,923 counts. Still the strongest available validation of the
  predicates and still unrun — the delta-2 harness rows are strong evidence but
  not that check.
- λ_total for the re-mined arm (only λ_classical was measured at n=400; the
  single-nonce harness phase counts, 5 vs 6, are n=1 and prove nothing).
