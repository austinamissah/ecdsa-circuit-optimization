# λ measured on `upstream/main` `6909d15`, the current figure

> Measured 2026-08-04 on the hardware in [`lambda-measurement.md`](lambda-measurement.md)
> (i7-1270P, 12 physical cores, 31 GB, Ubuntu 24.04, rustc 1.93.0). Circuit head **`6909d15`**,
> whose `src/` tree is identical to accepted submission **`ed4b529`**, because the five commits on
> top are CI-only. Sweep ran at **6 workers**, not 14.
>
> Companion to [`lambda-measurement.md`](lambda-measurement.md), which measures the same quantities
> on `801dd20`. Method, estimator and conventions are unchanged, so the two are directly comparable.

Raw data: [`data/lambda-sweep-6909d15.tsv`](data/lambda-sweep-6909d15.tsv).

**What λ is.** The benchmark builds its test inputs by hashing the circuit's own op stream, so a
circuit is never simply passing or failing. It has a failure rate. λ is the average number of failed
shots per run, and P(clean) is the chance that a given seed produces a run with zero failures.

## Which head this is, and why

`src/point_add/memory/06-research-status.md` certifies score `1,490,805,286` for promoted
submission `0c5b1b7b`, source **`cf5aa02`**, and says *"the repository has been reset to that
official source."* That sentence was true on 2026-07-28 and is now out of date. `cf5aa02` is
**14 commits behind `upstream/main`**, and eight submissions have been accepted since:

```
cf5aa02  2026-07-28  Accept 0c5b1b7b   <- 06-research-status.md's "official frontier"
  … 7 further acceptances …
ed4b529  2026-08-03  Accept 248dfb4a   <- current source
6909d15  2026-08-03  upstream/main     <- CI-only on top of ed4b529
```

`cf5aa02` is also an ancestor of this repo's HEAD, since we rebased past it on 2026-08-03
([`rebase-2026-08-03-upstream-ed4b529.md`](rebase-2026-08-03-upstream-ed4b529.md)). It is also
*worse*: `1,490,805,286` against the built `1,486,468,554` here. So λ on `cf5aa02` would be a
historical figure, not a current one. **This measurement is on `6909d15`.**

## Method

Identical to the `801dd20` sweep: 202 trials, each a full `./benchmark.sh`, meaning a build plus a
9,024-shot `eval_circuit`. **No custom screen** (`memory/04-traps.md` §4: the lazy-XOF screening
bug that reported false clean results).

- **Block A** (99 non-control): contiguous, `base+1 … base+99`.
- **Block B** (100): `base + k·2^40`, k = 1…100, wide stride across the 48-bit space.
- **Controls** (3): the shipped nonce, once in block A and twice appended.

`base` = **`200321420125`**, the nonce this head bakes in at `src/point_add/mod.rs:2384`. This is
*not* `801dd20`'s `62000008397024`; the positive control must be the head's own shipped nonce.

### The three gates, all passed before the sweep was trusted

1. **Baseline.** Pristine `6909d15` builds to `md5 ef30945f3afcb369192ea32897232d2f`, 0/0/0,
   avgT 1,288,101.386 × 1154 qubits = **1,486,468,554**, matching upstream exactly.
2. **The knob is live.** The shipped nonce passed explicitly reproduces the baseline md5 at 0/0/0;
   `base+1` produces a *different* md5 at 19/15/0. All **199 non-control nonces produced 199
   distinct md5 values**. This is the check that catches issue #23: `benchmark.sh` prefers
   `sudo -n bwrap`, and sudo's `env_reset` strips `SUB4_TAIL_NONCE` before `build_circuit` sees it.
   The driver installs a `sudo` shim that exits non-zero, forcing the `setpriv --no-new-privs
   bwrap` fallback.
3. **Worker isolation.** `eval_circuit` writes `results.tsv` to the `CARGO_MANIFEST_DIR` baked in
   at compile time, so workers sharing a build would append to one file. Verified by observation,
   not merely by rebuilding: a trial in `w01` grew `w01/results.tsv` and left `w00`'s untouched.

All three control rows returned `0/0/0` with the baseline md5.

## Results

| channel | mean | sd | sem | var/mean | range |
|---|---|---|---|---|---|
| classical | **17.126** | 4.149 | ±0.294 | 1.005 | 7 to 29 |
| phase | **11.628** | 3.473 | ±0.246 | 1.037 | 3 to 22 |
| ancilla | 0 | 0 | n/a | n/a | 0 |

var/mean ≈ 1 on both channels, so the counts are Poisson with no overdispersion, the same as on
`801dd20`. (Poisson is the distribution you get when rare events happen independently at a steady
rate; var/mean ≈ 1 is its signature.) Ancilla garbage is identically zero because `B::free` emits
an unconditional `R`, so the channel cannot fire.

**zero-classical 0/199 · zero-phase 0/199 · zero-both 0/199.**

### λ_total

Same Poisson-overlap estimator: `classical ~ Pois(λ_c + λ_both)`, `phase ~ Pois(λ_p + λ_both)` with
independent components, so `Cov(c,p) = λ_both` and `λ_total = mean_c + mean_p − Cov`. In words:
some shots fail on both channels at once, so adding the two means double-counts them, and the
covariance measures exactly that overlap.

| | λ_total | P(clean) | trials/seed |
|---|---|---|---|
| lower bound, `max(means)` | 17.126 ± 0.294 | 3.7e-8 | 2.7e7 |
| **covariance estimate** | **20.560** (95% CI 18.007 to 23.016) | **1.2e-9** | **8.5e8** |
| upper bound, `sum(means)` | 28.754 ± 0.384 | 3.3e-13 | 3.1e12 |

Decomposition: λ_classical_only 8.932, λ_both 8.193, λ_phase_only 3.435. Pearson ρ(c,p) = 0.569.

CI is a 4,000-resample bootstrap over whole rows (seed 20260804). A bootstrap re-draws the measured
rows at random, with replacement, thousands of times, and reports how much the answer moves. The
`sum(means)` sem is the independent-sum `sqrt(sem_c² + sem_p²)`, the same convention
`lambda-measurement.md` uses.

The directional caveat from `lambda-measurement.md` carries over unchanged: the estimator assumes
the classical-only and phase-only components are independent of each other. If they are positively
correlated beyond the shared term, λ_both is overstated and **λ_total is overestimated**, moving
down toward the 17.13 lower bound. The error only runs one way, so the covariance estimate stays on
the safe side for planning.

Direct observation alone bounds P(clean) only at < 1.5e-2 (rule of three on 0/199).

### Against the other two measurements

| head | source | λ_total | classical | phase |
|---|---|---|---|---|
| `02146ca` | `memory/02-lambda.md` | 23.29 | 18.13 | 12.64 |
| `801dd20` | [`lambda-measurement.md`](lambda-measurement.md) | 20.04 | 16.231 | 10.915 |
| **`6909d15`** | **this document** | **20.560** | **17.126** | **11.628** |

**λ_total did not move significantly.** Bootstrap on the difference `6909d15 − 801dd20` gives
**+0.525, 95% CI −2.626 to +3.632**; 37% of resamples put the difference at or below zero. On this
evidence the two heads have the same λ_total.

Both *per-channel* means did rise, each by a small amount: classical +0.894 ± 0.400 (t = 2.24,
p = 0.025), phase +0.714 ± 0.336 (t = 2.12, p = 0.034). λ_total absorbs most of it because λ_both
rose in step (7.111 → 8.193), which is why the channel-level movement does not carry through to the
total. Given two uncorrected comparisons and p-values near 0.03, this is weak evidence of a real
per-channel rise. Worth noting, not worth acting on.

The useful observation is the direction: `6909d15` scores **better** than `801dd20`
(1,486,468,554 vs 1,487,590,242) while its channels run slightly **dirtier**. That is the
score-versus-λ tension `02-lambda.md` describes, showing up across upstream's own progress.

`02-lambda.md`'s 23.29 is the most out-of-date figure of the three, and is the one this measurement
is offered to replace.

### Clean seeds are isolated, not clustered, reproduced

| block | n | classical | phase |
|---|---|---|---|
| A, contiguous | 99 | 17.354 ± 0.441 | 11.606 ± 0.328 |
| B, 2^40 stride | 100 | 16.900 ± 0.391 | 11.650 ± 0.369 |

Welch t = +0.770 (classical) and −0.089 (phase), so the two blocks are indistinguishable. That
reproduces the `801dd20` isolation result on a different head with a different base nonce. The
closest any trial came to clean was `base+29` at 8 classical / 4 phase. Grinding near a known-good
nonce buys nothing.

## What it costs

### Throughput at 6 workers

202 trials in **66 min 11 s** = **183 trials/hour** aggregate, against 205/hour at 14 workers on
`801dd20`. Losing 8 workers cost only 11% of throughput, which fits that document's finding that
the harness is limited by memory bandwidth and I/O (each trial pushes ~507 MB through zstd), not by
arithmetic. Per-worker that is ~118 s/trial under 6-way contention; a single uncontended
`./benchmark.sh` on a cold machine measured 84.6 s here.

**6 workers is the better operating point on this hardware**, at near-identical throughput, and it
leaves the desktop session responsive.

### The cost of a seed

| | λ_total | trials/seed | wall-time at 183 trials/hour |
|---|---|---|---|
| 95% CI low | 18.007 | 6.6e7 | **41 years** |
| **point estimate** | **20.560** | **8.5e8** | **529 years** |
| 95% CI high | 23.016 | 9.9e9 | **6,168 years** |

Unchanged in kind from `801dd20`: nonce grinding is off by three orders of magnitude. Note again
that the CI spans a factor of ~150 in wall-time. λ enters exponentially, so every figure here is an
order of magnitude, not a quantity.

### λ affordable for a one-day grind

| | trials/day | λ affordable |
|---|---|---|
| one core, 118 s/trial (6-way contended) | 732 | **≈ 6.60** |
| one core, 84.6 s/trial (uncontended) | 1,021 | **≈ 6.93** |
| whole machine, 6 workers @ 183/hr | 4,395 | **≈ 8.39** |

Against λ_total = 20.56 the shortfall is ~12 λ-units on the whole machine. The nonce screen
([`../tools/nonce-screen/`](../tools/nonce-screen/)) buys ≈ 2.2 of them. Lowering λ itself remains
the gating project.

## Reproducing

```bash
# pristine upstream tree. do NOT measure from our HEAD, which carries
# schedule.rs/gcd.rs lever edits that are env-gated but not provably inert
git archive 6909d15 | tar -x -C "$SCRATCH/w00"
cd "$SCRATCH/w00" && cargo build --release --locked --bin build_circuit --bin eval_circuit

# gate 1: must print 0/0/0 and md5 ef30945f3afcb369192ea32897232d2f
PATH="$SCRATCH/shim:$PATH" ./benchmark.sh

# 6 worker trees, each with its OWN build (see lambda-measurement.md)
# then:
bash docs/data/lambda-sweep-driver-6909d15.sh   # ~66 min
python3 analyse.py sweep_results.tsv
```

The analysis code was checked by re-deriving every published `801dd20` figure from
[`data/lambda-sweep-801dd20.tsv`](data/lambda-sweep-801dd20.tsv) before being applied here.

The three ways to silently invalidate the run are unchanged from
[`lambda-measurement.md`](lambda-measurement.md#three-things-that-will-silently-invalidate-the-run):
`sudo` must not be usable, `bwrap` must be able to run, and the controls must come back clean.
