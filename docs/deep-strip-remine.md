# Re-mining the deep strip at `TLM_SCHED_J2_DELTA=2`

> Measured 2026-08-03 on head **`801dd20`** (score 1,487,590,242). The fork was rebased onto
> `ed4b529` / `6909d15` later the same day, where the score is 1,486,468,554
> ([`rebase-2026-08-03-upstream-ed4b529.md`](rebase-2026-08-03-upstream-ed4b529.md)). Every score
> and λ figure here is priced against the older head, and the re-mined table is keyed to the older
> stream.
>
> Read alongside [`census-miner-validation.md`](census-miner-validation.md), which shows the miner
> failing its known-answer test, and [`census-stream-provenance.md`](census-stream-provenance.md),
> which identifies the census stream.

The shipped `deep_strip_keys.rs` is keyed to one op stream. Any change that moves the stream, such
as the `TLM_SCHED_J2_DELTA` λ lever, repoints those keys and the strip stops working. This document
measures what a re-mine against a delta-2 stream recovers.

## The result

| | avgT | peak q | score | vs head |
|---|---|---|---|---|
| head (shipped, delta 0, shipped strip) | 1,289,073 | 1154 | **1,487,590,242** | n/a |
| delta 2, strip OFF | 1,319,429 | 1155 | 1,523,940,495 | **+2.444%** |
| delta 2, **re-mined strip ON** | 1,307,877 | 1155 | **1,510,597,935** | **+1.547%** |
| delta 0, re-mined strip ON | n/a (not recorded) | 1154 | 1,492,415,116 | +0.324% |

**The re-mined strip recovers 13,342,560 of score, which is 0.897% of head.** That figure is a
**floor, not the true available score**: the miner certifies a strict subset of what is genuinely
dead (see [`census-miner-validation.md`](census-miner-validation.md)), so a correct miner would
recover more.

λ at n=400 per arm, same instrument that measured the shipped strip's 0.682:

| arm | λ_classical | sem |
|---|---|---|
| delta 2, strip OFF | 5.787 | ±0.125 |
| delta 2, re-mined strip ON | **5.777** | ±0.115 |

**Δλ = −0.010 ± 0.170, statistically zero.** The shipped table costs +0.682 ± 0.273 λ on its own
stream; the re-mined table costs nothing measurable, so **the re-mine does recover the 0.682 λ**.

### Harness verification, read directly rather than from a driver

| arm | ops | qubits | classical | phase | ancilla | stale keys | md5 ops.bin |
|---|---|---|---|---|---|---|---|
| strip OFF | 9,214,624 | 1155 | 6 | 5 | 0 | n/a | `baac874cfdd26ec6b7f25ac15cb6a9dc` |
| re-mined strip ON | 9,204,392 | 1155 | 7 | 6 | 0 | **0** | `4991360767a0f364a146b039de3f2d65` |

Both sit in the intrinsic band, and **0 stale keys** is the number that carries this: the old table
applied to this same delta-2 stream discards 13,484 keys and takes the circuit to 9,022/9,024
mismatches. The re-mined table addresses every gate it names.

## Why the λ came back, and what the held-out shards showed

Census: 120 M random on-curve pairs, 12 independent seeds, `--lanes 64`, at
`TLM_SCHED_J2_DELTA=2` with `SUB4_APPLY_STRIP=0`. Emitting from the 10 mining shards (100 M) and
then re-emitting with the 2 held-out shards (120 M):

| | dead | downgrade |
|---|---|---|
| mining only, 100 M | 10,364 | 2,206 |
| with held-out, 120 M | 10,232 | 2,169 |
| **caught by 20 M of held-out data** | **132** | **37** |

169 false keys per 20 M samples. A Good-Turing reading of that rate puts the residual error of the
120 M table at ≈ 169/20e6 × 9024 ≈ **0.08 λ**, consistent with the measured −0.010 ± 0.170.

**This corrects an earlier prediction.** A 120 M re-mine was predicted to cost 2 to 3× the 0.682 λ
it replaces, from `λ ≈ (dead keys) × 3/N × 9024` = 2.3. That formula assumes every dead key sits at
the detection threshold. Most do not, since they are structurally dead with p = 0, so it is a large
overestimate. The held-out measurement is the right estimator and it says the opposite. Census depth
mattered far less than argued; 120 M was ample.

The re-mined table is smaller than the shipped one (10,232 dead against 12,543; 2,169 downgrade
against 3,923), which is why it recovers 0.897% rather than the 1.16% the shipped strip is worth on
its own stream. Part of that is genuine census depth (120 M against 320 M); part is that the delta-2
geometry simply has fewer dead gates.

## Delta 0 is closed

- Stale keys of the shipped table against the delta-0 head stream: **251**, not zero, so in
  principle there was something to recover. **There was not.** All 251 went stale at commit
  `d6eed9a` because their operand tuple was *deleted from the stream*; none are occupancy drift.
  The gates do not exist and the score is already banked. See
  [`census-stream-provenance.md`](census-stream-provenance.md).
- The miner **cannot certify replacements for those 251**, and separately misses **3,165 dead** and
  **1,727 downgrade** keys the shipped table finds.
- Re-mined delta 0 scores **+0.324% against head**, which is a loss. λ_classical 15.258 ± 0.202
  against 15.342 ± 0.189 strip-off, so λ-free, but that buys nothing when the score moves the wrong
  way.

**Do not pursue a delta-0 re-mine with this miner.**

## The harness-order hypothesis: unresolved, and doubtful on cost

The hypothesis: the shipped census ran in the harness's XOF order (W=64), saw a narrower reachable
set and so certified more gates dead, while this miner drives Hmr/R from a xorshift PRNG and
explores freely. That would explain the miner's over-observation.

A harness-order mode is implemented (`--harness`: W=64, inputs and the Hmr/R stream both from the
real Fiat-Shamir XOF, pairs drawn up front, simulator continuing from the same reader). The PRNG arm
at 1 M samples finished: **19,719 never-fired, 2,170 c1-implied, 3,081 c2-implied**. The harness arm
did not finish and was killed.

**Why it did not finish is the useful part.** Harness order needs **1,013,644 XOF words per 64-shot
pass**, which is 8 MB of SHAKE256 output for every 64 shots. 1 M samples is 15,625 passes ≈ 127 GB
of SHAKE output; the 320 M-sample census the shipped table claims would need ~40 TB. **The shipped
census therefore almost certainly was not taken in harness order**, which undercuts the hypothesis
on cost grounds before any measurement lands. Something else explains the 25%/43% disagreement, and
[`syntactic-certification-is-exhausted.md`](syntactic-certification-is-exhausted.md) identifies it:
a sampler cannot see an invariant at any depth.

### If harness-order mining ever does win, the table is still not shippable

A census taken in one XOF order certifies gates dead only for the measurement outcomes that stream
produces. The scored circuit's stream is determined by its own op sequence, and every score-lowering
edit re-rolls it, so a table tuned to one stream would silently delete live gates under another.
That is exactly the failure mode in `04-traps.md` §1 and the reason the occupancy tripwire exists.
**Only a stream-agnostic census is shippable.** If harness-order mining wins on counts, that is
evidence the shipped table is fragile, not a licence to copy it.

## Two bugs found while doing this

**`sim.rs` reads 8 XOF bytes for `R` as well as `Hmr`.** The PRNG path did not consume for `R`, so
any XOF-order run would have desynchronized the entire downstream stream. Found while writing the
harness-order mode, and fixed.

**`compare_d0.py` built its occupancy map only from tuples present in the emitted key table**, so
tuples whose gates are all live were miscounted as occupancy mismatches: it inferred **2,715** stale
keys where the build reports **251**. Every number in this document and in
[`census-miner-validation.md`](census-miner-validation.md) comes from the per-gate dump
(`--mode dump`), not from that script.

## What is not established

- **The re-mined table is committed as data only**
  ([`data/deep-strip-keys-delta2-120M.rs.gz`](data/deep-strip-keys-delta2-120M.rs.gz)), not
  installed into `src/point_add/deep_strip_keys.rs`. It is mined against a delta-2 stream and would
  corrupt the shipped delta-0 circuit. The repo table is untouched.
- **λ_total for the re-mined arm.** Only λ_classical was measured, at n=400, from the screen. The
  single-nonce harness phase counts (5 against 6) are n=1 and prove nothing.
- **A delta-0 control re-mine against the shipped 12,543 / 3,923 counts** is the strongest available
  check on the predicates. [`census-miner-validation.md`](census-miner-validation.md) runs it and
  the miner fails, conservatively.
- **The two remaining suspects for the over-observation**, namely the CCZ effective condition (this
  census folds the target in as `cond & t & c1 & c2` and the shipped census may not) and
  condition-stack handling of nested `PushCondition`, are untested.
