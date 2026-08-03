# The census miner fails its known-answer test

> Measured 2026-08-03. The miner is [`../tools/census/`](../tools/census/); the
> table under test is the shipped `src/point_add/deep_strip_keys.rs`.

## Result: FAIL, and the miner is the thing that is wrong

A delta-0 control census (120 M pairs, 12 seeds, `SUB4_APPLY_STRIP=0`) was
re-mined against the shipped head stream and compared to the known
12,543 dead / 3,923 downgrade.

**The keying machinery is provably correct.** Replaying the tripwire against the
dumped per-gate flags reproduces `build_circuit` exactly:

| | tripwire accepts | stale | build reports |
|---|---|---|---|
| dead | 12,292 | 251 | `removed 12292 / 12543 ... 251 stale` ✅ |
| downgrade | 3,923 | 0 | `downgraded 3923 / 3923` ✅ |

**The certification predicates are not.** Of the shipped keys the tripwire
accepts — the ones that actually get applied, in a circuit that passes
9,024/9,024 — this census claims:

| | accepted | census says it fires / is violated |
|---|---|---|
| dead | 12,292 | **3,076 (25.02%)** |
| downgrade | 3,923 | **1,674 (42.67%)** |

The shipped table demonstrably produces a 0/0/0 circuit. If 3,076 live Toffolis
were being deleted the circuit would be destroyed. **So the shipped table is
right and this census over-observes firing.**

## The failure is conservative, which is why the delta-2 result still stands

The miner certifies a strict subset of what is truly dead:

- delta 0: 9,378 dead / 2,196 downgrade, against 12,543 / 3,923 shipped.
- **Keys in the re-mine that are NOT in the shipped table: 0.** A pure subset.

A subset is *safe* — every gate it deletes really is dead — it just leaves score
on the table. That is consistent with everything measured:

| circuit | ops | q | classical | phase | score | vs head |
|---|---|---|---|---|---|---|
| head (shipped strip) | 9,058,005 | 1154 | 0 | 0 | 1,487,590,242 | — |
| delta 0, re-mined strip | 9,060,919 | 1154 | 20 | 13 | 1,492,415,116 | **+0.324%** |

λ_classical, n=400 per arm:

| arm | λ_classical | vs strip OFF |
|---|---|---|
| head, shipped strip | 16.025 ± 0.197 | +0.682 ± 0.273 |
| delta 0, strip OFF | 15.342 ± 0.189 | — |
| delta 0, **re-mined strip** | **15.258 ± 0.202** | **−0.084 ± 0.277** |

So the re-mined table is λ-free at delta 0 too — but it **costs 0.324% of score**.
**A delta-0 re-mine is not a score win. It is a score loss.** There is nothing to
recover: the 251 stale keys are real, but this miner cannot certify replacements
for them, let alone the 3,165 dead keys it fails to reproduce.

λ_total is unmeasured for every arm here; only λ_classical was measured.

## Untested hypothesis for the over-observation

The census drives Hmr/R from a xorshift PRNG, so it explores measurement-outcome
combinations freely. The shipped census may have been taken in the harness's own
XOF order, seeing a narrower set of reachable states and therefore certifying
more gates dead. If so the shipped table is tuned to that stream and this miner
is the safer of the two — which would be consistent with the strip's measured
0.682 λ cost. **This is a hypothesis; it has not been tested.** The test is to
re-run one shard with the harness XOF in W=64 order and see whether the 25%/43%
disagreement collapses.

## What this invalidates, and what it does not

- **Does not invalidate the delta-2 re-mine result**
  ([`HANDOFF-2026-08-03-remine.md`](HANDOFF-2026-08-03-remine.md)): that table
  was harness-verified correct (7 classical / 6 phase, 0 stale keys) and
  measured λ-free. Subset-safety explains why it worked.
- **Does invalidate any claim that this miner reproduces the shipped census.**
  It does not, and the 0.897% the delta-2 table recovered is a floor set by a
  conservative miner, not the true available score.
