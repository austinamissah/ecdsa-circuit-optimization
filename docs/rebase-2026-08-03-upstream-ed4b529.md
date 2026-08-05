# Rebase onto upstream `ed4b529`: the new baseline, measured

> Rebased and measured 2026-08-03 on the hardware in
> [`lambda-measurement.md`](lambda-measurement.md). Supersedes the
> `1,487,590,242` baseline used by every table in
> [`lambda-levers.md`](lambda-levers.md), [`census-miner-validation.md`](census-miner-validation.md)
> and [`deep-strip-remine.md`](deep-strip-remine.md).

Upstream accepted submission `248dfb4a` as commit **`ed4b529`** on 2026-08-03: 67 files,
12,028 insertions, touching `mod.rs`, `arith.rs`, `ec_add.rs` and a wholesale rewrite of
`deep_strip_keys.rs`. Five CI-only commits sit on top of it; `upstream/main` is **`6909d15`**
and its `src/` tree is identical to `ed4b529`'s.

## The new baseline is 1,486,468,554, and the in-source comment is wrong

**`src/point_add/mod.rs:2379` claims `score 1,487,599,474`. The tree does not build that.**

| | avgT | peak q | ops | score | md5 `ops.bin` |
|---|---|---|---|---|---|
| previous head `8af8a6f` | 1,289,073.125 | 1154 | 9,058,005 | 1,487,590,242 | `f5c5f98258ddb7a0b1f250750ad1c6d2` |
| **`upstream/main` `6909d15`** | **1,288,101.386** | **1154** | **9,057,301** | **1,486,468,554** | `ef30945f3afcb369192ea32897232d2f` |
| what the comment claims | *(1,289,081)* | *(1154)* | n/a | *1,487,599,474* | n/a |

Built twice from a clean checkout with no `TLM_*` / `SUB4_*` in the environment, byte-identical
both times, **0 classical / 0 phase / 0 ancilla**. The comment overstates avgT by exactly **980**,
i.e. the score by 1,130,920. The most likely explanation is that the comment was written before the
`deep_strip_keys.rs` re-mine landed in the same submission; either way it is stale and **must not be
quoted as the baseline.** Build it.

So the real move is **−1,121,688, or −0.0754%**, in the right direction. Upstream improved; it did
not regress. Reading the comment at face value inverts the sign of that conclusion, which is
precisely the failure mode this document exists to prevent.

### Three numbers disagree; only one was built

| source | value | what it actually is |
|---|---|---|
| `memory/RIG.md:12`, `CEILING.md:35` | 1,489,216,228 | frontier at submission `705b36a`, **source `7fa872d`**, an older head (avgT 1,290,482). `RIG.md` also pins 9,062,420 ops against the current 9,057,301. Stale by construction: it records the rig's last *promoted* witness, not the tree it ships in. |
| `mod.rs:2379` comment | 1,487,599,474 | stale by 980 avgT; see above. |
| **`./benchmark.sh`** | **1,486,468,554** | **the tree.** |

**Measure against 1,486,468,554.** The other two are provenance records of earlier states, and both
are worse than the circuit actually in the repo.

## Arms priced against the new baseline

Full harness, `build_circuit` + `eval_circuit`, at the shipped nonce. "upstream strip" is the new
13,056 / 4,222 table; delta is `TLM_SCHED_J2_DELTA`.

| arm | avgT | peak q | score | vs head | strip applied |
|---|---:|---:|---:|---:|---|
| head (delta 0, upstream strip) | 1,288,101.386 | 1154 | 1,486,468,554 | n/a | 13,056 / 4,222, 0 stale |
| delta 0, strip OFF | 1,304,021.591 | 1154 | 1,504,841,388 | +1.2360% | n/a |
| **A: delta 1, upstream strip** | 1,308,633.614 | 1155 | 1,511,472,270 | **+1.6821%** | 2,687 / 646, **13,945 stale** |
| **B: delta 1, strip OFF** | 1,311,736.513 | 1155 | 1,515,056,235 | **+1.9232%** | n/a |
| delta 2, upstream strip | 1,316,550.882 | 1155 | 1,520,616,405 | +2.2972% | 2,510 / 607, 14,161 stale |
| delta 2, strip OFF | 1,319,434.332 | 1155 | 1,523,946,270 | +2.5213% | n/a |

Each arm has a distinct `md5 ops.bin`. Note peak qubits go 1154 → 1155 at every delta ≥ 1, which
costs **+0.0867%** on its own before any avgT movement.

### Arm C is a foregone conclusion, so do not spend the census

C was "delta 1 with a census re-mined at delta 1", and it is worth running only if it can land
*below* 1,486,468,554. It cannot, and the bound does not depend on how good our miner is:

- B sits at **+1.9232%**. To reach the head at q=1155 the strip must take avgT from 1,311,736.5 to
  ≤ 1,286,985, a saving of **24,751.5**.
- The *complete, upstream-quality* strip at delta 0 saves **15,920.2** avgT (the head vs
  strip-OFF row). A delta-1 strip would have to save **1.55×** what a full-quality strip saves at
  delta 0.
- Even granting a perfect miner that matches upstream's own table, C lands at
  **+0.686%**, still above the head.
- Our miner is a strict conservative subset. On the old head it realized 0.897% where a full strip
  was worth 1.16%, a ratio of 0.77; against upstream's *larger* 13,056 / 4,222 table that ratio can
  only fall. Scaling 1.236% by it predicts **C ≈ +0.97%**.

So the realistic outcome is +0.97% and the optimistic ceiling is +0.69%, against a target of 0.00%.
**Skipped.** Two hours of census would have confirmed a gap that the delta-0 strip row already
bounds.

The delta lever itself is unaffected as a **λ** instrument. This says the delta arms cost score on
the new head, exactly as they did on the old one, not that λ has stopped responding to them. λ on
the new stream is unmeasured.

## The identity gate: our two source changes survive the rebase exactly

46 commits replayed onto `upstream/main` with **zero conflicts**, since upstream touched none of the
three files we carry (`trailmix_ludicrous/gcd.rs`, `trailmix_ludicrous/schedule.rs`,
`memory/05-qubit-reduction.md`). Pre-rebase state is preserved on
`backup-pre-rebase-7bfbfda`, pushed.

The rebased tree at `TLM_SCHED_J2_DELTA=0` with the strip on rebuilds to **md5
`ef30945f3afcb369192ea32897232d2f`, score 1,486,468,554, 0/0/0**, byte-identical to stock
`upstream/main`. The `ITERS`-clamp and the delta lever are still exact identities at delta 0 on the
new stream, which is the same gate applied at `b1c8f84` / `9f34bb9` on the old one.

## What upstream actually changed

- **Backed out the H3 coordinate-width-18 lever.** `TLM_COORD_MSBS=18` and
  `TLM_COORD_Y_SUB_FINAL_MSBS=19` are gone from `configure_q1153_second512_submission_defaults`,
  and `arith.rs` lost the three `*_with_cleanup` variants that let `cleanup_bits` differ from
  `msbs()`. That is a reversal of the `4579b79 → 8af8a6f` edit that
  [`census-stream-provenance.md`](census-stream-provenance.md) attributes the `−4` gate family to.
- **New tail nonce**, `62000008397024` → `200321420125`.
- **A new, deeper strip table**: **13,056 dead / 4,222 downgrade, 0 stale**, against our
  12,543 / 3,923 with 251 stale. Its header is
  `AUTO-GENERATED by union max-coverage admission (census6/maxcov)`, so it no longer records a
  sample count or the censused stream's op/gate totals.

That header phrase bears directly on the open lead in
[`census-stream-provenance.md`](census-stream-provenance.md#a-lead-the-appends-are-monotonic-which-is-backwards-for-depth):
monotonic appends with zero retractions are backwards for census *deepening*, and we inferred "an
additional non-sampling certification layer". Upstream's own header now says the table is a
**union** admitted by a **max-coverage** criterion, which is a mechanism that can only add, and is
not a deeper sample. That is corroboration for the lead, not proof of it, and the header's loss of
provenance counts means the stream-identification method of that document no longer applies to the
new table.

## Question 1: "risk-3.0" is a provenance nickname, not a phase-risk budget

The string occurs **exactly once in the entire tree**, in that one comment. Grepping `ed4b529`
for `risk-3`, `risk_3` and `risk 3.0` returns that line and nothing else.

The only other `risk` tokens are in the submitter's own search harness under
`memory/repro/`: a free-text `correctness_risk` field required by `schema_harness.py`, and an
`ActionKind.RISK_OR_STRIP_BUDGET` enum in `world_model.py`. Neither is read by the circuit. There
is no risk knob, no risk parameter, and no numeric budget anywhere in `src/point_add/` outside that
comment. `H2` in the same phrase is their niche taxonomy, since `niche_portfolio.json` and `CEILING.md`
define `H2-square` as "reversible modular square", unrelated to the phase channel.

**So: upstream does not budget phase risk explicitly, and the `3.0` is not our λ_phase_only.** Our
3.000 (measured conditional on classical-clean, `c4767be`) matching their `3.0` is a coincidence
until something connects them, and nothing in the tree does. Worth stating because the coincidence
is a tempting one.

The one genuinely suggestive detail is `ActionKind.RISK_OR_STRIP_BUDGET` requiring
`PAIRED_FIXED_RANDOMNESS` and `EXACT_CENSUS` as evidence: upstream's search treats "risk budget"
and "strip budget" as a single lever class needing paired-randomness plus an exact census, which is
the same shape as the λ-versus-strip trade we measured. That is a statement about their process,
not about their circuit.

## Question 2: what the Blacksmith workflow requires

`.github/workflows/benchmark.yml`, new in `0cd1f92`, on `blacksmith-32vcpu-ubuntu-2404`,
`workflow_dispatch` only, **`timeout-minutes: 45`** covering `./setup.sh` *and* `./benchmark.sh`.

Binding constraints on a submission:

1. **Editable surface is enforced twice.** `benchmark.json` sets
   `"editablePaths": ["src/point_add"]`, and the workflow re-checks it independently for
   `refs/heads/submissions/*`: the commit must be **single-parent**, and every path in
   `git diff --name-only $SHA^ $SHA` must be under `src/point_add/`. Our `docs/` and `tools/`
   work can never ride along in a submission commit. It has to be a clean single commit touching
   `src/point_add/` only.
2. **Score is read from `score.json`** via `jq -er '.score | select(type == "number")'`, so a
   FAIL that still writes the file does not silently pass, but nothing in the workflow re-checks
   `correct == OK`, so validity is entirely `eval_circuit`'s gate.
3. **The 45-minute cap is not currently a threat.** Our full build+eval is ~110 s on a throttling
   2-core-equivalent laptop; a 32-vCPU runner with a warm `target/` cache is far inside the cap.
   It becomes a threat only if `build_circuit` grows substantially, so it is worth re-checking before any
   submission that raises `ITERS` or widens schedules, since those lengthen the emitted stream.
4. **`benchmark.sh` changed with it.** The sandbox scratch dir is now `chmod 1777` and
   `build_circuit` is staged into it as a `0555` copy before `bwrap` execs it, because Blacksmith
   keeps `/home/runner` unreadable by uid 65534. This does not change the env-stripping trap
   recorded in [`lambda-measurement.md`](lambda-measurement.md): a submission must still never
   depend on an environment variable.

## What this invalidates

Everything λ in `docs/` was measured on the pre-`ed4b529` stream and is now stale in its absolute
numbers:

- λ_total = 20.04, λ_classical = 16.231, λ_phase = 10.915 are properties of the old head.
- Every score in [`lambda-levers.md`](lambda-levers.md) is priced against 1,487,590,242.
- The delta-2 re-mined table in `docs/data/` is keyed to the old stream and will strip almost
  nothing here.
- [`census-stream-provenance.md`](census-stream-provenance.md)'s stream walk ends at the old head.
  Its findings about the *census-era* stream stand; its "current head" column does not.

The *exchange rates*, meaning 0.098 to 0.237% of score per λ-unit for the delta lever and the strip's 1.70%,
are ratios measured across arms on one stream, and are the part most likely to survive. That is a
hypothesis, not a carry-over: it needs re-measuring here.
