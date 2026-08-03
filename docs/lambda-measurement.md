# λ, the third axis — measured on `801dd20`

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

At the measured 61 s per trial, one clean seed is ~5.0e8 trials ≈ **60 wall-years on 16 cores**.
Nonce grinding is out of scope on a workstation.

Since every score-lowering change re-rolls the seed, **λ reduction is the gating project**, not a
side concern. The targets:

| screen | trials/day on 16 cores | λ needed for a 1-day grind |
|---|---|---|
| current 61 s full harness | 2.3e4 | ≈ 10.0 |
| a 1.2 s fast screen | 1.2e6 | ≈ 14.0 |

A fast screen is worth ~4 λ-units — cheaper to buy in engineering than in circuit correctness.
See [`upstream-search-economics.md`](upstream-search-economics.md) for what such a screen looks
like and why it is only a *candidate* filter.

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
