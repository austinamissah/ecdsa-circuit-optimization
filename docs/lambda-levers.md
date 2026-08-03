# λ levers — what each one actually buys, measured on `801dd20`

> **Measured 2026-08-03** on the hardware in [`lambda-measurement.md`](lambda-measurement.md).
> Circuit head `801dd20`, with the two source changes described in
> [Two things that had to change first](#two-things-that-had-to-change-first); both are
> byte-identical to the shipped stream at the shipped knob set.
>
> **Which λ.** The lever comparisons are `λ_classical`, measured with
> [`../tools/lam-screen/`](../tools/lam-screen/) at n=400 per arm — the screen is classical-channel
> only, and a screen-clean nonce is a candidate, not a seed. The winning configuration is then
> confirmed on the **full harness**, where λ_total is measurable; that is
> [the section that decides the question](#λ_total-at-delta-2-measured-on-the-full-harness).
>
> **Headline: `TLM_SCHED_J2_DELTA=2` takes λ_total from 20.04 to 8.111**, i.e. a clean seed from
> ~5e8 harness trials (279 years) to ~3,340 (16 hours), for 1.27% of score.

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

| ITERS | λ_classical | sem | Δλ vs 261 | tail-curve prediction | avgT | peak q | score | vs shipped |
|---|---|---|---|---|---|---|---|---|
| 259 | 17.000 | ±0.202 | **+1.658 ± 0.277** (6.0σ) | +1.970 | 1,297,846 | 1154 | 1,497,714,284 | +0.68% |
| **261 (shipped)** | **15.342** | ±0.189 | *(anchor)* | — | 1,304,032 | 1154 | 1,504,852,928 | +1.16% |
| 262 | 15.637 | ±0.184 | +0.295 ± 0.264 (1.1σ) | −0.283 | 1,307,157 | 1155 | 1,509,766,335 | +1.49% |
| 264 | 15.695 | ±0.195 | +0.353 ± 0.272 (1.3σ) | −0.449 | 1,313,674 | 1155 | 1,517,293,470 | +2.00% |
| 267 | 14.967 | ±0.202 | −0.375 ± 0.277 (1.4σ) | −0.480 | 1,323,362 | 1158 | 1,532,453,196 | +3.02% |

The prediction column is `02-lambda.md`'s divstep convergence tail — `258→5.228, 259→2.453,
260→1.114, 261→0.483, 262→0.200, 265→0.014`, from a 1e6-sample convergence distribution — as a
delta against the tail term at 261.

**Downward, the model is right, decisively.** 259 predicts +1.970 and measures **+1.658 ± 0.277**,
a 6.0σ effect at 84% of the predicted magnitude. The small shortfall is in the direction you would
expect: removing two divsteps also removes two divsteps' worth of their *own* truncation error.
This is the first direct confirmation of that curve on the current head, and it is the result that
lets the rest of the row be trusted.

**Upward, the predicted gain does not appear.** 262, 264 and 267 predict −0.28, −0.45 and −0.48.
They measure +0.295, +0.353 and −0.375, none individually significant and scattering either side
of zero; pooled they give **≈ +0.09 ± 0.16**. So the honest statement is a bound, not an effect:
**|Δλ| < 0.4 at 95% confidence for any `ITERS` above 261, against a predicted gain of 0.3–0.5.**

Two things plausibly absorb the predicted gain, and the data does not separate them. The tail term
at 261 is already down to 0.483, so an added divstep can recover at most half a λ-unit — while,
because `SCHED_J2` holds at 9 and `GAP_J2` at 10 past the end, that divstep runs at the *terminal*
register width and pays its own truncation error through the same two channels (`SCHED_J2`
dropping a nonzero bit, fold-window carry escapes) that cost 2.80 and 2.18 in the original
decomposition. Peak qubits also move — 1155 at 262 and 264, 1158 at 267 — and
`memory/05-qubit-reduction.md` records that the vent pool expands to fill whatever is freed, so
the 267 arm is not a clean read of the divstep channel alone.

**Either way the lever is spent.** Its entire remaining budget is the 0.483 λ still in the tail at
261, and buying it costs 0.33% of score per step up — 1.86% by 267. Nothing above 261 is worth
taking, and 259 is 1.66 λ worse for 0.48% of score. `ITERS = 261` is the right value; it is simply
not a place where λ can still be found.

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


## The lever: widening the divstep width schedule

`SCHED_J2[i]` is how many bits of `u` survive divstep `i` — `gcd.rs:1230` pops and frees
everything above it — so the schedule is a deliberate truncation, and `02-lambda.md` prices
"`SCHED_J2` drops a nonzero bit, walk still terminates" at 2.80 mismatches per 9,024.
`TLM_SCHED_J2_DELTA` (added in this branch, `schedule.rs`) widens it by a constant number of bits
at every divstep, with `GAP_J2` moving in lockstep.

**`GAP_J2` must move with it.** `memory/05-qubit-reduction.md` step 5 records that the divstep
error depends only on `s = SCHED_J2[i] − cmp_window(i)`, and that moving one without the other
takes that channel from 8.36 to 4,646 mismatches. A constant delta added to both preserves `s`
exactly, since `cmp_window` is `min(gap_j2(i), current_n)` with `current_n = sched_j2(i)`. Delta 0
is the identity — verified, md5 `f5c5f98258ddb7a0b1f250750ad1c6d2`.

| delta | λ_classical | sem | Δλ vs baseline | avgT | peak q | score | Δscore | % score per λ-unit |
|---|---|---|---|---|---|---|---|---|
| 0 (baseline) | 15.342 | ±0.189 | — | 1,304,032 | 1154 | 1,504,852,928 | — | — |
| **1** | **8.412** | ±0.145 | **−6.930 ± 0.238** (29σ) | 1,311,740 | 1155 | 1,515,059,700 | +0.68% | **0.098** |
| **2** | **5.787** | ±0.125 | **−9.555 ± 0.227** (42σ) | 1,319,429 | 1155 | 1,523,940,495 | +1.27% | 0.133 |
| 4 | 4.662 | ±0.102 | −10.680 ± 0.215 (50σ) | 1,335,901 | 1155 | 1,542,965,655 | +2.53% | 0.237 |

**This is an order of magnitude better than anything else measured.** Against 1.70% of score per
λ-unit for the deep strip and 1.8%+ for `ITERS`, the first delta step costs **0.098%**. Marginal
rates: delta 0→1 is 0.098% per λ-unit, 1→2 is 0.225%, 2→4 is 1.12% — so returns fall off sharply
and **delta 2 is the sensible stopping point**, with delta 1 the best value per unit if score is
tight.

Three things make this a real effect rather than an artefact:

- **The standard deviation tracks the mean.** sd falls 3.783 → 2.891 → 2.498 → 2.048 against
  √λ = 3.917 → 2.900 → 2.406 → 2.159. The distribution stays Poisson with the rate itself lowered;
  it is not a shifted or truncated distribution.
- **All 400 stream fingerprints are distinct in every arm**, and every arm has its own
  `md5 ops.bin` — `baac874c…` at delta 2 against the baseline's `4cb7eb53…`.
- **The direction check.** `05-qubit-reduction.md` measured this same lever the *other* way —
  narrowing 160 tail entries bought −0.49% of score for +3.6 λ, i.e. ≈0.14% per λ-unit. Running it
  backwards reproduces that exchange rate, from independent data on a different head.

The measured 9.56 λ at delta 2 is far larger than the 2.80 that `02-lambda.md` assigned to the
`SCHED_J2` channel. The width schedule is evidently feeding the other classical channels too,
which is consistent with that document's own warning that the four sources "are all
truncation-style approximations driven by the same input magnitudes".

## What this does to feasibility

The target was never "reduce λ_total by 11". With a screen, the search runs in two stages: find
nonces that are clean on the classical channel, then confirm each on the full harness. So the
quantity that gates screening throughput is **λ_classical**, and λ_phase_only sets the
candidates-per-seed ratio.

| | λ_classical | P(classical-clean) | nonces per candidate | at 4,360 nonces/hour |
|---|---|---|---|---|
| shipped (strip on) | 16.025 | 1.1e-7 | 9.1e6 | 87 days |
| baseline (strip off) | 15.342 | 2.2e-7 | 4.6e6 | 44 days |
| **`DELTA=1`** | **8.412** | 2.2e-4 | 4,520 | **62 minutes** |
| **`DELTA=2`** | **5.787** | 3.1e-3 | 327 | **4.5 minutes** |
| `DELTA=4` | 4.662 | 9.4e-3 | 106 | 1.5 minutes |

**At delta 2 the screen produces a classical-clean candidate every few minutes on this laptop**,
against 44 days at the baseline. That is the difference between a grind that cannot be started and
one that runs overnight.

### λ_total at delta 2, measured on the full harness

The screen is classical-only, so none of the above establishes λ_total — and λ_phase_only = 3.80 on
the shipped circuit is what sets the candidates-per-seed ratio. So this was measured directly:
**42 full `build_circuit` + `eval_circuit` trials at delta 2**, no screen, same estimator as
`lambda-measurement.md` (Poisson-overlap, λ_total = mean_c + mean_p − Cov(c,p)). All 42 produced
distinct `md5 ops.bin`.

| channel | shipped `801dd20` (n=199) | **delta 2 (n=42)** |
|---|---|---|
| classical | 16.231 ± 0.271 | **6.214 ± 0.429** |
| phase | 10.915 ± 0.229 | **6.595 ± 0.406** |
| ancilla | 0 | 0 |
| Cov(c,p) | 7.11 | 4.699 |
| **λ_total** | **20.04** | **8.111** |

**The phase channel moved with the classical one.** That was the thing that could have killed this
lever and it did not: phase falls 10.915 → 6.595, and λ_phase_only (λ_total − λ_classical) falls
**3.80 → 1.90**. The classical figure also cross-checks the screen: 6.214 ± 0.429 on the harness
against 5.787 ± 0.125 on 400 screened nonces, a 1.0σ agreement between two independent instruments.

### What a seed now costs

| | λ_total | P(clean) | harness trials/seed | at 205 trials/hour |
|---|---|---|---|---|
| shipped | 20.04 | 2.0e-9 | 5.0e8 | **279 years** |
| **delta 2** | **8.111** | **3.0e-4** | **3,340** | **16 hours** |

And with the screen in front of it, the two-stage search is better still: a classical-clean
candidate every ~4.5 minutes, of which `e^-1.90 = 15%` are also phase-clean, so **≈7 candidates and
under an hour per clean seed** on this laptop.

**This is the result the project needed.** `lambda-measurement.md` concluded that "nonce grinding is
not merely impractical on this hardware; it is off by three orders of magnitude", and that λ
reduction carried essentially all the weight. It did, and it was available: 11.9 λ-units of it, for
1.27% of score.

**Caveat on precision.** n=42 is a small sample for a covariance estimator; the sem on each channel
is ~0.42 and λ_total inherits more than that. Read 8.111 as "about 8", i.e. thousands of trials per
seed rather than hundreds of millions. That is an order-of-magnitude claim, and the order of
magnitude is what changed. The directional argument in `lambda-measurement.md` still applies —
the covariance estimate is conservative for planning and cannot understate the cost of a grind.

## Status: what is and is not established

**Established, n=400 per arm, on the classical channel:**

- `ITERS` is spent. The `02-lambda.md` tail curve is confirmed downward at 6.0σ; upward the
  predicted gain is absent, bounded at |Δλ| < 0.4.
- The `ITERS ≡ 0 mod 3` rule does not exist, and the deep strip is what made 259/260 look destroyed.
- The deep strip costs 0.682 ± 0.273 λ despite being documented as bit-exact, and its occupancy
  tripwire is insufficient rather than merely imperfect.
- `TLM_SCHED_J2_DELTA` buys 6.9 to 10.7 λ_classical at 0.098–0.237% of score per λ-unit.

- **λ_total = 8.111 at delta 2** (n=42, full harness), against 20.04 shipped. The phase channel
  moved with the classical one; λ_phase_only falls 3.80 → 1.90. A clean seed costs ~3,340 harness
  trials, ~16 hours, against 279 years.

**Not established:**

- **The precision of λ_total.** n=42 supports an order of magnitude, not a third significant figure.
- ~~**The score prices are un-retuned upper bounds.**~~ **Measured, and the effect is small.**
  Re-fitting `TLM_TARGET_Q` and `TLM_SQUARE_PEAK_CAP` from their pinned 1154 to match the arms'
  actual peak:

  | config | caps | avgT | peak | score | vs pinned |
  |---|---|---|---|---|---|
  | delta 2 | 1154 | 1,319,429 | 1155 | 1,523,940,495 | — |
  | delta 2 | 1155 | 1,318,032 | 1156 | 1,523,644,992 | −0.019% |
  | delta 2 | 1156 | 1,316,598 | 1157 | 1,523,303,886 | −0.042% |
  | delta 1 | 1154 | 1,311,740 | 1155 | 1,515,059,700 | — |
  | delta 1 | 1155 | 1,310,333 | 1155 | 1,513,434,615 | **−0.107%** |

  At delta 2 `peak = cap + 1` exactly at every setting, so raising the cap buys avgT and gives it
  straight back in width — a third independent confirmation of `05-qubit-reduction.md`'s central
  operational fact that the vent pool expands to fill whatever you free. Delta 1 is the exception:
  peak holds at 1155 while avgT falls, for a real −0.107%. **So the delta-2 price is +1.25% rather
  than +1.27%, and the delta-1 price is +0.57% rather than +0.68%** — the conclusions do not move.
- **Interaction with a re-mined census.** Every arm ran with the strip off. Whether a census
  re-mined against a delta-2 stream recovers its 1.16% without re-introducing its 0.68 λ is
  untested.

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
