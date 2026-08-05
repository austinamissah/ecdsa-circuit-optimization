# How upstream is actually landing submissions

> **Which head the λ figures are priced against.** This document was written against `801dd20`
> (λ_total 20.04, ~5.0e8 trials/seed at 205 trials/hour, see
> [`lambda-measurement.md`](lambda-measurement.md)). The current figure is on `6909d15`: **λ_total
> 20.560, ~8.5e8 trials/seed** at 183 trials/hour, see
> [`lambda-6909d15.md`](lambda-6909d15.md). Bootstrapped, the two heads are statistically
> indistinguishable, so every conclusion below is unchanged in kind. Quote `6909d15` for anything
> current.

At λ_total ≈ 20 a clean seed costs ~5.0e8 full-harness trials on `801dd20`, ~8.5e8 on `6909d15`.
Upstream landed 25 accepted submissions in 21 days. This document works out how, because the answer
determines what is worth building.

Everything here is read from `src/point_add/memory/repro/dgm_search.py` (2,848 lines) and from the
upstream commit graph. That file arrived with the 2026-08-02 rebase; it is another contestant's
work, not ours.

## The cadence, corrected

25 accepted submissions between 2026-07-11 and 2026-08-01: **0.88 days each**, not the ~1.6 we first
assumed. Every one is authored by a single GitHub App, `yukon-autoresearch[bot]`. This is one
automated program, not a field of contestants.

## It is a Darwin-Gödel-Machine controller, not a nonce grinder

`dgm_search.py` is an automated code-search loop. It derives an archive of stepping stones from a
ledger, selects a parent, registers a structured prediction in advance, has an LLM modify the repo
inside a disposable git worktree, then verifies. Changes are confined to
`ALLOWED_MUTATION_ROOT = src/point_add` with `.rs` suffixes, the same `editablePaths` the platform
enforces.

Five permitted action kinds, one of which matters here:

```
NONCE_ONLY, EXACT_REWRITE, GEOMETRY, RISK_OR_STRIP_BUDGET, REPRESENTATION
```

`NONCE_ONLY` confirms re-grinding is treated as a first-class change, so they do pay the seed cost
per candidate.

**Four of the five sibling modules did not ship.** `artifact_io`, `exact_scorer`, `schema_harness`,
and `world_model` are all imported and all absent. So is the ledger
(`.autoresearch/measurements.jsonl`). What we can read is the controller skeleton only.

Two things worth naming because they are easy to misread: `exact_scorer.score(average_toffoli,
qubits)` is just the score formula, and `backtest` replays ledger records. Neither one predicts
mismatches. **There is no cheap classical oracle for correctness in anything that shipped.**

## The real mechanism: a shot ladder

```python
SHOT_LADDER = (512, 2_048, 8_192, 9_024)
```

`_build_eval_variant` rewrites `const NUM_TESTS: usize = {shots}` in `eval_circuit.rs` and compiles
a dedicated evaluator per rung. The controller walks the ladder and stops at the first failure. The
full 9,024-shot run is additionally gated on a budget and on beating the public frontier.

At λ = 20.04 (`801dd20`) this costs **1,255 expected shots instead of 9,024, a 7.2× saving**:

| rung | P(reach it) | expected shots |
|---|---|---|
| 512 | 1.000 | 512 |
| 2,048 | 0.321 | 657 |
| 8,192 | 0.0106 | 87 |
| 9,024 | 1.25e-8 | ~0 |

## λ is not a selection criterion

```python
eligible = [n for n in nodes if n.functioning and n.reproducible]
ranked   = sorted(eligible, key=lambda n: (n.conservative_score, n.candidate_id))
quality  = math.exp(-2.0 * percentile)
weight   = (0.05 + quality) / (1.0 + node.functioning_children)
```

`Metrics` carries only `average_toffoli` and `qubits`. `ArchiveNode` has **no failure-count field at
all**. λ enters solely as the binary `functioning` gate: pass 0/0/0 or be ineligible. Selection
pressure is score plus a bonus for under-explored branches. λ is tolerated, never improved.

That is a gap, and it is where our opening is: they are not managing the quantity that gates their
own throughput.

## The ledger is in the nonces

The ledger file did not ship, but each submission bakes its winning nonce into
`src/point_add/mod.rs`, and those values are not uniform draws over the 48-bit space:

- only **7 of 22** are ≥ 1e13, against **96.4%** expected under uniform sampling
- mean longest run of zero digits is **2.82**, against ~1.2 expected

Both point to a sharded, sequentially-enumerated search rather than random sampling, with implied
depths of order **1e6 to 1e9 trials per accepted submission**. The exact shard and counter encoding
could not be pinned down; the non-uniformity and the order of magnitude are the solid parts.

## Putting it together

Two dull things, not one clever one:

1. **λ was not 20 for most of the campaign.** `ITERS` moved 258 → 261 on 2026-07-26, the same day
   **8 submissions landed**. Our λ = 20.04 is a *late* head (`801dd20`; `6909d15` measures 20.560,
   statistically the same). Earlier, less aggressive circuits were cleaner and their grinds
   correspondingly cheap. The shape of the campaign (8 submissions on 07-26, 3 on 08-01) is a
   rising-λ ramp.
2. **Cost per try, not number of tries.** ~1e8 trials in ~1 day is ~1,157 trials/s; on 1,344 vCPU
   that is ~1.2 s/trial, about **90× faster than our 110 s full-harness run**. The ladder supplies
   7.2× of that. Skipping the rebuild supplies most of the rest, and there is nothing exotic about it.
   `apply_tail_nonce` touches only the last 96 ops, and `fiat_shamir_seed` is a *streaming* SHAKE256
   absorb. So the hash state over ops[0 … n−96] can be computed once and cloned per nonce, absorbing
   5,376 bytes instead of ~507 MB, with no circuit rebuild at all.

**Both of those are now built and measured on our side, and they do not close the gap.** The
validated screen ([`../tools/nonce-screen/`](../tools/nonce-screen/)) does exactly the above and
reaches **12 s/nonce against the harness's 110 s, so 9.2× and not 90×**. Since the rebuild was the
whole of our saving, the remaining ~10× must come from somewhere we have not touched: the
simulation itself. `eval_circuit` spends 57 s on 9,024 shots, which includes 18,048 secp256k1
scalar multiplications for test-pair generation. A faster simulator, cheaper pair generation, or
simply better hardware would each account for it. Which of those they use is **unknown**, since the
screening code did not ship, and it is the open question worth the most.

So they are not beating brute force with a correctness oracle. They are much faster per trial
through engineering we have only partly reproduced, and they spent most of the campaign at a λ where
the grind was cheap.

## Consequence: a screen is a candidate filter, never a seed finder

A screen built on the ladder covers the **classical channel only**. Our measured decomposition is
λ_classical_only 9.12, λ_both 7.11, λ_phase_only 3.80, so a nonce that is clean on the classical
channel still has

    P(phase-clean) = e^(-3.80) = 2.2e-2

Screen hits are **candidates requiring full-harness confirmation**. Expect roughly **45 candidates
per true seed**. Reporting a screen hit as a clean seed would be the same class of error as the
lazy-XOF bug in `memory/04-traps.md` §4.

Three non-negotiables for any such screen, each of which has already bitten someone:

- Draw **all** test pairs for a rung before constructing the simulator. The simulator continues from
  the same XOF reader; drawing lazily makes it consume bytes the input draw still needs, yielding
  valid-but-wrong points that never mismatch and report false clean.
- md5 every generated stream. Two identical hashes across distinct nonces means the tail edit is not
  reaching the stream.
- Never read avgT from a screen. It is W=64-harness-order only.

The correctness bar is that the screen reproduces the full harness's per-nonce classical mismatch
**count**, not its clean/dirty verdict, on nonces with known full-harness results. Exact agreement,
not correlation. [`data/lambda-sweep-801dd20.tsv`](data/lambda-sweep-801dd20.tsv) provides 199 such
nonces.
