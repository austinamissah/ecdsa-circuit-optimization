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

PLACEHOLDER-BODY

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
