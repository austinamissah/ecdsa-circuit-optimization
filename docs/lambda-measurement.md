# λ, the third axis — measured on `801dd20`

> **Measured 2026-08-02** on a Lenovo ThinkPad X1 Carbon Gen 10 — Intel i7-1270P (12 physical
> cores: 4 performance + 8 efficient, 16 threads), 31 GB RAM, Ubuntu 24.04.4, kernel 6.17.0-35,
> rustc/cargo 1.93.0. Circuit head `801dd20`. Every wall-clock figure below is anchored to the
> **measured aggregate throughput of 205 trials/hour** on this machine — 202 trials in 59 minutes
> across 14 concurrent workers — and not to an extrapolation from a single uncontended run. The
> distinction matters: see [Throughput](#throughput).

The score is `average executed Toffoli × peak qubits`. There is a third quantity that decides
whether a circuit can ship at all, and it is not in the score: **λ, the intrinsic per-run failure
rate**. `src/point_add/memory/02-lambda.md` names it; this document measures it on the rebased
head and prices what it costs us.

Raw data: [`data/lambda-sweep-801dd20.tsv`](data/lambda-sweep-801dd20.tsv).

## Why it binds

`eval_circuit` derives its 9,024 test inputs from a SHAKE256 hash **of the whole op stream**
(`fiat_shamir_seed`, `src/bin/eval_circuit.rs:204`). So any change that lowers the score also
changes the stream, which re-rolls the test inputs. A circuit does not have a fixed pass/fail
status — it has a failure *rate*, and shipping requires finding a nonce whose particular 9,024
draws all pass.

`apply_tail_nonce` (`src/point_add/mod.rs:1792`) exists for exactly this. It rewrites only
`q_target` on 48 adjacent `X;X` identity pairs at the tail, so the circuit's *function* is
provably unchanged across all 2^48 nonces while the seed moves freely. The live knob is
`SUB4_TAIL_NONCE` (`mod.rs:2384`, default `62000008397024`).

## Method

202 trials, each a full `./benchmark.sh` — build plus a 9,024-shot `eval_circuit`. **No custom
screen.** `memory/04-traps.md` §4 documents a lazy-XOF screening bug that reported false clean
results and cost its author a 1,344-vCPU grind; the harness itself was used as the oracle to avoid
reproducing it.

- **Block A** (99 trials): contiguous, `base+1 … base+99` — tests whether clean seeds cluster.
- **Block B** (100 trials): `base + k·2^40`, k = 1…100 — independent regions of the space.
- **Controls** (3): the shipped nonce `62000008397024`, once in block A and twice appended.

All three controls returned `0/0/0` with md5 `f5c5f98258ddb7a0b1f250750ad1c6d2`, matching the
shipped artifact. The 199 non-control nonces produced **199 distinct md5 values** — proof the knob
was live, and the check that caught the harness bug described below.

## Results

| channel | mean | sd | sem | var/mean | range |
|---|---|---|---|---|---|
| classical | **16.231** | 3.825 | ±0.271 | 0.902 | 6–26 |
| phase | **10.915** | 3.225 | ±0.229 | 0.953 | 4–20 |
| ancilla | 0 | 0 | — | — | 0 |

var/mean ≈ 1 on both channels is Poisson with no overdispersion, reproducing the nonce-invariance
result in `memory/02-lambda.md`: this is intrinsic per-shot error, not overfitting to a test set.
Ancilla garbage is identically zero because `B::free` emits an unconditional `R`, so the channel
cannot fire — every would-be ancilla failure is laundered into half a phase failure.

**zero-classical 0/199 · zero-phase 0/199 · zero-both 0/199.**

### λ_total

The two channels overlap, so λ_total is neither the max nor the sum of the means. Under a
Poisson-overlap model — `classical ~ Pois(λ_c + λ_both)`, `phase ~ Pois(λ_p + λ_both)` with
independent components — the covariance *is* λ_both, which makes it directly measurable:

| | λ_total | P(clean) | trials/seed |
|---|---|---|---|
| lower bound, `max(means)` | 16.231 ± 0.271 | 8.9e-8 | 1.1e7 |
| **covariance estimate** | **20.04** (95% CI 18.22–21.85) | **2.0e-9** | **5.0e8** |
| upper bound, `sum(means)` | 27.146 ± 0.355 | 1.6e-12 | 6.2e11 |

Decomposition: λ_classical_only 9.12, λ_both 7.11, λ_phase_only 3.80. Pearson ρ(c,p) = 0.576,
against 0.5205 measured on the older head — the same overlap structure.

**What would falsify this, and which way it breaks.** The covariance estimator relies on an
assumption that is not tested by the data: that the *non-shared* components — the
classical-only and phase-only failures — are mutually **independent**, so that all of the
observed covariance is attributable to the shared term λ_both. If instead those components are
positively correlated beyond the shared term, the covariance overstates λ_both, and since
λ_total = mean_c + mean_p − λ_both, **λ_total is overestimated**. The true value then moves down
toward the `max(means)` lower bound of **16.23**, and P(clean) up toward 8.9e-8 — a grind roughly
30× cheaper than the point estimate implies.

The error is directional, which is the useful part: the covariance estimate is **conservative for
planning**. It cannot understate the cost of a grind, only overstate it. A plausible physical
mechanism for such correlation exists — the four λ sources listed below are all truncation-style
approximations driven by the same input magnitudes, so an input that stresses one may stress
another — so this is a live possibility, not a formality. Testing it would need per-shot failure
identities rather than per-run counts, which the harness does not expose.

`memory/02-lambda.md` fits the same structure by a different route (conditional means of
`E[pg|cm]` over bins, discriminating a "phase ⊂ classical" model from a "phase-only failures
exist" model, SSE 13.37 vs 2.44). That it lands on a comparable decomposition from independent
data is mild corroboration, not proof.

`memory/02-lambda.md` reports λ_total = 23.29 on the promoted head `02146ca`. This head is
cleaner on both channels (16.23 vs 18.13 classical, 10.92 vs 12.64 phase), and 20.04 sits
sensibly below it.

Direct observation alone bounds P(clean) only at < 1.5e-2 (rule of three on 0/199) — far too weak
to act on, which is why the model estimate carries the conclusion.

### Clean seeds are isolated, not clustered

| block | n | classical | phase |
|---|---|---|---|
| A, contiguous | 99 | 16.202 ± 0.386 | 10.798 ± 0.330 |
| B, 2^40 stride | 100 | 16.260 ± 0.383 | 11.030 ± 0.318 |

Statistically indistinguishable. Sitting next to a known-clean seed buys nothing: `base+1`
through `base+10` run 13–18 classical, and none of the 99 contiguous successors is clean. The
closest any trial came was `base+47` at 6 classical / 7 phase. Grinding near a known-good nonce
is no better than grinding anywhere.

## What it costs

<a name="throughput"></a>
### Throughput

A single uncontended trial takes **61 s**. That number does not extrapolate: running 14 workers
concurrently gave an aggregate of **205 trials/hour**, i.e. **245 s per trial under load** and a
parallel efficiency of only **3.5×**, not 14×. Two reasons, both structural — this is a hybrid
laptop CPU where only 4 of the 12 cores are performance cores, and each trial re-emits the full
op stream and pushes ~507 MB through zstd to produce a 30 MB `ops.bin`, so the workers contend on
memory bandwidth and I/O rather than on arithmetic.

**Use 205 trials/hour, the measured aggregate, for any cost estimate on this hardware.** An
earlier draft of this document quoted "60 wall-years" by dividing the 61 s single-trial time by
16; that assumed perfect scaling and understated the cost by about 4.6×.

### The cost of a seed

| | λ_total | trials/seed | wall-time at 205 trials/hour |
|---|---|---|---|
| 95% CI low | 18.22 | 8.2e7 | **46 years** |
| **point estimate** | **20.04** | **5.0e8** | **279 years** |
| 95% CI high | 21.85 | 3.1e9 | **1,715 years** |

Nonce grinding is not merely impractical on this hardware; it is off by three orders of magnitude.

**Limitation worth stating plainly: the 95% CI 18.22–21.85 spans a factor of ~38 in
trials-per-seed** (46 to 1,715 wall-years). λ enters the cost exponentially, so even a
well-determined λ leaves the thing you actually care about loosely determined. Every planning
figure here should be read as an order of magnitude, not a quantity.

### What would make a grind feasible

Since every score-lowering change re-rolls the seed, **λ reduction is the gating project**, not a
side concern. For a one-day grind on this machine:

| | trials/day | λ needed |
|---|---|---|
| measured full harness (205/hr) | 4.9e3 | **≈ 8.5** |
| with a 50× screen, if achieved | 2.5e5 | **≈ 12.4** |

A screen is therefore worth roughly **4 λ-units** — cheaper to buy in engineering than in circuit
correctness, which is the argument for building it first. The 50× is upstream's inferred per-trial
figure, not ours, and is unvalidated here; a screen should also scale better in parallel than the
harness does, since it eliminates the per-trial rebuild and the zstd I/O that caused the 3.5×
efficiency. Both effects are unmeasured. See
[`upstream-search-economics.md`](upstream-search-economics.md) for what such a screen looks like
and why it is only a *candidate* filter, and
[`../tools/nonce-screen/`](../tools/nonce-screen/) for an unbuilt draft.

`memory/02-lambda.md` prices the four classical sources: divstep convergence tail (5.73, bought
back by raising `ITERS` at ~2,930 emitted CCX per iteration), i=257 apply skips (5.30),
`SCHED_J2` dropping a nonzero bit (2.80), LSBS=53 fold-window carry escapes (2.18). Every one of
them is a deliberate correctness-for-gates trade, so λ reduction pushes *against* the score axis.
That tension is the real shape of the problem.

## Two traps this measurement hit

**The knob was silently dead for the first two trials.** `benchmark.sh` prefers `sudo -n bwrap`,
and sudo's `env_reset` strips `SUB4_TAIL_NONCE` before `build_circuit` ever sees it, so every
trial silently measured the default nonce and produced a byte-identical `ops.bin`. Caught only by
the standing rule in `memory/04-traps.md` §1: *a null result is only a result if `md5 ops.bin`
changed.* Worse, it is intermittent — `sudo -n` succeeds only while a credential timestamp is
cached, so a long sweep can start env-stripped and finish env-honouring, silently splitting into
two different experiments. The driver forces the `setpriv --no-new-privs bwrap` fallback.

**Corollary for submissions:** a submission must never depend on an environment variable. Bake the
value into the `set_default_env` / `unwrap_or` default in `src/point_add/`, because the scored run
may strip it.

**avgT is W=64-harness-order only.** `memory/04-traps.md` §4: classical outcomes are insensitive
to the Hmr/R stream, but phase and avgT are not. The `avgT` column in the raw data comes from
`eval_circuit` (`BATCH = 64`) and from nowhere else. Across the 199 nonces it varies with
sd 8.3 — small, but a single-nonce Toffoli comparison still gates at roughly ±40, not ±20.

## Reproducing

Measured on the hardware in the header. **Expect ~59 minutes** for the full 202-trial sweep at
14 workers (205 trials/hour); scale accordingly. Peak disk is ~3 GB for the worker trees, and the
machine is saturated throughout — this is not a background job.

```bash
# From the repo root, on the circuit head you want to characterise.
git rev-parse --short HEAD          # record this; λ is a property of the head

# 1. Baseline: the unmodified head must come back 0/0/0 before anything else.
./benchmark.sh
#    Expect on 801dd20: score 1487590242, qubits 1154,
#    md5 ops.bin = f5c5f98258ddb7a0b1f250750ad1c6d2

# 2. Run the sweep. Edit SCRATCH/BASE/NWORK at the top of the driver first.
cp docs/data/lambda-sweep-driver.sh /path/to/scratch/sweep.sh
bash /path/to/scratch/sweep.sh      # ~59 min; writes sweep_results.tsv
```

The driver expects worker trees `w00`…`w13` to already exist under `SCRATCH`, each a copy of the
repo with its **own** `cargo build`. That is not optional: `eval_circuit` writes `results.tsv` to
the `CARGO_MANIFEST_DIR` baked in at compile time, so workers sharing a build would all append to
one file. Create them with:

```bash
for w in $(seq -w 0 13); do
  cp -a "$SCRATCH/w00" "$SCRATCH/w$w"          # w00 built once from a repo copy
  ( cd "$SCRATCH/w$w" \
    && touch src/bin/build_circuit.rs src/bin/eval_circuit.rs \
    && cargo build --release --locked --offline --bin build_circuit --bin eval_circuit )
done
```

The `touch` forces the two binaries to recompile so each bakes its own path; the library does not
rebuild, so this costs ~15 s per worker rather than ~70 s.

### Three things that will silently invalidate the run

1. **`sudo` must not be usable.** The driver installs a `sudo` shim that exits non-zero, forcing
   `benchmark.sh` onto its `setpriv --no-new-privs bwrap` path. Without it, sudo's `env_reset`
   strips `SUB4_TAIL_NONCE` and every trial silently measures the default nonce. Verify before
   trusting anything: two different nonces must produce two different `ops.bin` md5 values.
2. **The sandbox must be able to run.** On Ubuntu 24.04+, `kernel.apparmor_restrict_unprivileged_userns=1`
   blocks `bwrap`, and `benchmark.sh` fails closed with
   `bwrap: loopback: Failed RTM_NEWADDR`. An AppArmor profile permitting `userns` for
   `/usr/bin/bwrap` fixes it. Also ensure the scratch path is traversable by uid 65534 (`o+x` on
   every parent directory), or `bwrap` fails with `execvp: Permission denied`.
3. **The controls must come back clean.** The nonce list includes the shipped nonce three times.
   If any control row is not `0/0/0` with the baseline md5, the sweep is void — do not analyse it.

### Analysis

`docs/data/lambda-sweep-801dd20.tsv` is the output of the above. The reported statistics are
per-channel mean/sd/sem over the 199 non-control rows, plus
`λ_total = mean_classical + mean_phase − Cov(classical, phase)`, with a 4,000-resample bootstrap
over rows for the CI. Nothing in the analysis is weighted or filtered; rows are used exactly as
recorded.
