# λ levers — what each one actually buys, measured on `801dd20`

> **Measured 2026-08-03** on the hardware in [`lambda-measurement.md`](lambda-measurement.md).
> Circuit head `801dd20`, with the two source changes described in
> [Two things that had to change first](#two-things-that-had-to-change-first); both are
> byte-identical to the shipped stream at the shipped knob set.
>
> **Classical channel only.** Every λ figure here is `λ_classical`, from
> [`../tools/lam-screen/`](../tools/lam-screen/). It is not λ_total, and a screen-clean nonce is a
> candidate, not a seed — see [`upstream-search-economics.md`](upstream-search-economics.md).

[`lambda-measurement.md`](lambda-measurement.md) established that λ reduction is the gating
project: at λ_total = 20.04 a clean seed costs ~5e8 harness trials, and every score-lowering
change re-rolls the seed. `memory/02-lambda.md` priced four classical λ sources by exact emulation
at `ITERS = 258`. This document measures them directly at the shipped `ITERS = 261`, and prices
each on the score axis, so the levers can be ranked by exchange rate rather than by size.

Raw data: [`data/lambda-levers-iters-801dd20.tsv`](data/lambda-levers-iters-801dd20.tsv),
[`data/lambda-levers-env-801dd20.tsv`](data/lambda-levers-env-801dd20.tsv), with the build-side
prices in the two `*-preflight.tsv` files beside them and the shared nonce list in
[`data/lambda-levers-nonces.txt`](data/lambda-levers-nonces.txt).

## Two things that had to change first

**`ITERS` could not be raised at all.** The eight blocked schedule vectors are re-fitted to the
current `ITERS` by `widen_sched_blocks`, but `SCHED_J2` and `GAP_J2` are indexed by the divstep
directly (`gcd.rs:21, 1226, 1475`), never through the `step()` cursor, so they were never re-fitted
— and both are exactly 261 entries long. `ITERS = 264` panicked in `build_circuit` with
`index out of bounds: the len is 261 but the index is 261`. **261 was a cap, not a tuning
decision.** `sched_j2(i)` / `gap_j2(i)` now clamp the index so both hold their terminal entry past
the end, which is the same "hold the terminal value" rule `widen_sched_blocks` already applies to
the blocked vectors, and for the same reason: the added divsteps run at the terminal register
width. Identity below the cap — verified, not assumed: at `ITERS = 261` the rebuilt `ops.bin` is
byte-identical, md5 `f5c5f98258ddb7a0b1f250750ad1c6d2`.

**Every arm had to be measured with the deep strip off.** At `ITERS = 264` the circuit comes back
at 7,874/9,024 classical mismatches — the "repointed gate-DROP table" row of `02-lambda.md`'s
triage table. With `SUB4_APPLY_STRIP=0` the same circuit gives 13 classical / 11 phase, squarely
in the intrinsic band. So the circuit was never the problem; the identity-keyed deep strip was. It
is therefore held off in every arm below, with `ITERS = 261, strip off` as the common baseline, so
that no arm is confounded by the strip repointing under it.

## The `ITERS` ladder is spent, and not for the recorded reason

| ITERS | λ_classical | sem | tail-curve prediction | avgT | peak q | score | vs shipped |
|---|---|---|---|---|---|---|---|
| 259 | 17.000 | ±0.202 | 17.312 | 1,297,846 | 1154 | 1,497,714,284 | +0.68% |
| **261 (shipped)** | **15.342** | ±0.189 | *(anchor)* | 1,304,032 | 1154 | 1,504,852,928 | +1.16% |
| 262 | 15.637 | ±0.184 | 15.059 | 1,307,157 | 1155 | 1,509,766,335 | +1.49% |

The prediction column is `02-lambda.md`'s divstep convergence tail — `258→5.228, 259→2.453,
260→1.114, 261→0.483, 262→0.200, 265→0.014` from a 1e6-sample convergence distribution — anchored
at the measured 261 value.

**Downward, the model is right.** 259 predicts 17.312 and measures 17.000 ± 0.202: 1.5σ, with the
small shortfall in the direction you would expect, since removing two divsteps also removes two
divsteps' worth of their *own* truncation error.

**Upward, it is wrong, and in the decision-relevant direction.** 262 predicts a *fall* to 15.059
and instead *rises* to 15.637 ± 0.184 — **3.1σ the wrong way**, Δ = +0.295 ± 0.264 against the
shipped 261.

That is not a contradiction of the convergence model, it is the model running out of room. The
tail term at 261 is already down to 0.483, so an added divstep can recover at most half a
λ-unit — while, because `SCHED_J2` holds at 9 and `GAP_J2` at 10 past the end, that divstep runs
at the *terminal* register width and contributes its own truncation error through the same two
channels (`SCHED_J2` dropping a nonzero bit, fold-window carry escapes) that cost 2.80 and 2.18 in
the original decomposition. Past 261 the second term dominates the first.

**So 261 is at or very near the λ minimum of this schedule**, and it costs 0.33% of score to move
one step either way. The lever is spent. This is a stronger statement than "the tail curve is
steep": adding divsteps at terminal width is not merely low-yield, it is *negative* yield.

### The `ITERS ≡ 0 mod 3` rule does not exist

`memory/05-qubit-reduction.md` records `ITERS` as "pinned at 261" because it "**must be ≡ 0 mod 3**
or `jump_dialog_regions` grows a ragged Pair/Raw tail", citing `ITERS=260 → 4,906` and
`ITERS=259 → 7,348` classical mismatches, "both destroyed". Two independent lines say otherwise:

- **Measured.** 259 with the strip off gives 21 classical on the shipped nonce and
  λ_classical = 17.000 over 400 nonces. That is the intrinsic band, not a destroyed circuit. Those
  arms were measured with the deep strip on, which repoints whenever `ITERS` moves off the value
  the census was mined at, exactly as it does at 264.
- **From the codec.** `jump_dialog_regions` starts at `n3 = iters/3` and *decrements until it
  fits*, coding the remainder as Pair/Raw. There is no mod-3 cliff: tape width grows smoothly at
  ~2.33 bits per divstep across 258–267 in both `tail4_top32` settings. The measured peak-qubit
  steps (1154 at 259 and 261, 1155 at 262 and 264, 1158 at 267) follow tape growth, not a
  divisibility rule.

The operative constraint on `ITERS` was `SCHED_J2`'s length, and the operative hazard was the
strip.

## The deep strip is not zero-error

`apply_deep_strip_identity` is documented in-source as *"Bit-exact: the removed gates never fire
for any valid curve-point input"*, and it carries an occupancy tripwire — each key records how
often its operand tuple occurred at census time, and keys whose occupancy moved are discarded with
a warning rather than applied — which is meant to make it safe under stream edits. Neither claim
survives measurement.

| strip | λ_classical | sem | avgT | peak q | score |
|---|---|---|---|---|---|
| on (shipped) | 16.025 | ±0.197 | 1,289,073 | 1154 | 1,487,590,242 |
| off | **15.342** | ±0.189 | 1,304,032 | 1154 | 1,504,852,928 |

**Δλ = −0.682 ± 0.273 (2.5σ) for Δscore = +1.16%.** The strip buys 1.16% of score and pays 0.68
λ-units for it — an exchange rate of **1.70% of score per λ-unit**.

And the tripwire is not sufficient. At `ITERS = 264` it discards 7,871 keys as stale, loudly and
correctly, and the survivors *still* take the circuit to 7,874/9,024 mismatches. At the shipped
261 it already discards 251. Occupancy matching is a necessary condition for a key to still
address the gate it was mined against, not a sufficient one — which is the same lesson as
`04-traps.md`'s "per-phase CCX equality is NOT a soundness certificate", one level up.

The λ cost is small in absolute terms and the strip is worth keeping at its current exchange rate.
The finding that matters is that **it has a λ cost at all**, so it belongs in the λ budget rather
than being carried as free, and that a re-mine against the current stream should recover both the
251 stale keys *and* this 0.68.


## Method

**Instrument.** [`../tools/lam-screen/`](../tools/lam-screen/), re-gated against all 199 harness
nonces at two lane widths before use. Every arm is 400 nonces × 9,024 shots = 3.6 M simulated
shots, at ~4,360 nonces/hour on 10 workers.

**Sample size and what it can resolve.** Per-nonce λ_classical is Poisson with sd ≈ 3.8, so
n = 400 gives sem ≈ 0.19 and a two-arm difference sem ≈ 0.27. That resolves a 1 λ-unit change at
3.7σ and a 0.5 λ-unit change at only 1.8σ, which is stated here rather than buried: the small-Δ
arms below are bounded, not measured precisely.

**The nonce set is fixed across arms but the pairing is cosmetic.** The Fiat–Shamir seed is a hash
of the *whole* op stream (`eval_circuit.rs:204`), so any change that alters the circuit re-rolls
all 9,024 test inputs at every nonce. Two arms at the same nonce therefore share nothing but the
label, and the shared list buys no variance reduction. It is used for protocol hygiene and so the
199 harness-known nonces sit inside every arm.

**Integrity checks, all three of which fired at least once during this work.**

1. *Every arm has a distinct `md5 ops.bin`, recorded in the preflight tables* — a null result is
   only a result if the stream moved (`memory/04-traps.md` §1). The env driver refuses to run a λ
   arm whose md5 matches another arm's.
2. *Every arm has 400 distinct stream fingerprints* — proof the tail-nonce edit reached the stream
   on every trial.
3. *The 199 harness-known nonces are inside the 400*, so the control arm re-derives the published
   harness result from inside the sweep. It does: all 199 counts reproduce exactly, mean 16.231,
   matching `lambda-measurement.md`.

**Scores are un-retuned.** `avgT` and peak qubits come from the real `eval_circuit` at the shipped
nonce, read from `results.tsv`, which records them on the FAIL path too — so a lever can be priced
even when the shipped nonce is not clean under it. But `TLM_TARGET_Q` and `TLM_SQUARE_PEAK_CAP`
are both pinned at 1154, fitted to the shipped geometry; an arm that moves peak qubits has not had
those re-fitted to it. Read every score here as the cost of the lever *before* re-tuning, i.e. an
upper bound on its cost.
