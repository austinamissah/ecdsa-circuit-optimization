# The census stream, identified, and what the stream difference does not explain

> Measured 2026-08-03 by rebuilding every commit that touched Rust sources since the census header
> first appeared. Read alongside [`deep-strip-remine.md`](deep-strip-remine.md)
> and [`census-miner-validation.md`](census-miner-validation.md), whose step 1, *"diff the streams
> first, cheapest and most likely cause"*, this document carries out.
>
> **⚠ "Current head" below means `801dd20` (rebase `8af8a6f`), not the current one.** The stream
> walk ends at that head; the fork was rebased onto `ed4b529` / `6909d15` on 2026-08-03, where the
> shipped stream is 9,057,301 ops. The findings about the *census-era* stream stand, meaning the
> identification of `d9ef3e9`, the 251 stale keys, and the miner's unexplained misses, but every
> "current head" stream figure is one head behind. See
> [`rebase-2026-08-03-upstream-ed4b529.md`](rebase-2026-08-03-upstream-ed4b529.md#what-this-invalidates).

`src/point_add/deep_strip_keys.rs`'s header says the census ran against **9,073,163 ops /
1,361,613 CCX+CCZ**. The then-current head `801dd20`'s unstripped stream is
**9,070,297 / 1,360,635**. That mismatch was the leading suspect for why the re-miner in [`../tools/census/`](../tools/census/)
cannot reproduce the shipped table.

**It is not the cause.** The stream difference is real, small, and now fully attributed, and it
accounts for the 251 stale keys *exactly and completely*, while accounting for **none** of the
~3,165 dead / ~1,727 downgrade keys the miner fails to reproduce.

## The census stream is commit `d9ef3e9`

Built with `SUB4_APPLY_STRIP=0`, commit `d9ef3e9` ("Accept submission 8233cd7e") emits
**9,073,163 ops / 1,361,613 CCX+CCZ**, bit-for-bit the header's numbers. `7fa872d` is
md5-identical to it. The census stream is recoverable, not lost with the VM.

**The header is honest, and this has to be established before anything else.** The key table was
rewritten and appended across ~10 commits, growing 9,324 → 12,543 dead, so "a union of censuses
taken over different streams, all stamped with one stale header" was the obvious hypothesis. It is
false:

| key population | occupancy stamps matching the `d9ef3e9` stream |
|---|---|
| 9,232 dead keys already present at `d9ef3e9` | **9,232 / 9,232 (100.00%)** |
| 3,311 dead keys added *after* `d9ef3e9` | **3,311 / 3,311 (100.00%)** |
| 2,081 downgrade keys already present at `d9ef3e9` | **2,081 / 2,081 (100.00%)** |
| 1,842 downgrade keys added *after* `d9ef3e9` | **1,842 / 1,842 (100.00%)** |

Zero mismatches, zero tuples absent, zero ordinals out of range. **All 16,466 keys stamp that one
stream at 100%.** Every key in the shipped table describes one stream, and the header names it
correctly.

One precision. The table is also consistent with every repo stream from `d9ef3e9` through
`5265674`, because those differ only on tuples the table never names (see the `−344` row below).
It is inconsistent from `d6eed9a` onward. The header's *counts* pin the census to
`d9ef3e9`/`7fa872d`/`37ab267` exactly.

## The divergence: 978 CCX (0.072%), in three attributable edits

354 hunks; 1,429 gates removed, 451 inserted, net **−978**. All of them CCX, and **CCZ is untouched**.
Op-kind deltas across the whole stream: X −268, CX −1,600, CCX −978, and −4 each on CZ / R / Hmr /
PushCondition / PopCondition (four whole conditional gadgets). Total −2,866 ops, exact.

| stage | Δgates | Δops | shape of the change | cause |
|---|---|---|---|---|
| `7fa872d` → `7726431` | −344 | 0 | 344 isolated single-CCX deletions, spread across the whole stream at mean gap ~3,900 gates | one entry in `codec.rs` `NORMALIZER_OPS`: `(2,6,10,9)` → `(1,10,9,0)` |
| `5265674` → `d6eed9a` | −630 | −2,806 | **6 hunks, all in one band**, census gates 657,181 to 699,681, op 4.41 to 4.84 M (48.6 to 53.3% of the stream). Three are exactly `(128 → 0)`; the others `(131→125)`, `(254→127)`, `(240→127)` | large refactor commit |
| `4579b79` → `8af8a6f` | −4 | −60 | 4 hunks of shape `(19 → 18)`, a one-index shift in a 6-wide register window | `TLM_COORD_MSBS=18`, `TLM_COORD_Y_SUB_FINAL_MSBS=19` (the "H3 coordinate-width-18 stream") |

The `−344` family replaces one Toffoli-class entry with a CX-class entry in a program that runs 344
times, which is why the op count does not move while 344 CCX disappear. The `−630` family is the
only one that touched a keyed gate.

Everything after `8af8a6f` is gate-stream identity: `b1c8f84` (the `ITERS` clamp) and `9f34bb9`
(`TLM_SCHED_J2_DELTA` at delta 0) both rebuild to 9,070,297 / 1,360,635 with an identical gate
dump, an independent confirmation of the md5 identity claims in
[`lambda-levers.md`](lambda-levers.md).

Per-commit stream sizes are in
[`data/stream-walk-by-commit.tsv`](data/stream-walk-by-commit.tsv); the hunk-level diff is in
[`data/census-vs-head.gates.diff.gz`](data/census-vs-head.gates.diff.gz).

## The 251 stale keys: fully explained, and nothing was recoverable

All 251 arise at **one commit, `d6eed9a`**, and all for the same reason: **their operand tuple was
physically deleted from the stream.** 243 distinct tuples, every one an unconditional CCX, every
removed instance inside the 6-hunk band. **Zero of the 251 are occupancy-drift cases**, and there is
not a single key in the table whose tuple survives with a changed count.

| stage | dead keys stale | downgrade stale | newly stale |
|---|---|---|---|
| `d9ef3e9` (census) | 0 | 0 | 0 |
| `7fa872d` | 0 | 0 | 0 |
| `7726431` | 0 | 0 | 0 |
| `5265674` | 0 | 0 | 0 |
| **`d6eed9a` / `4579b79`** | **251** | **0** | **251** |
| `8af8a6f` … HEAD | 251 | 0 | 0 |

### The correction

[`deep-strip-remine.md`](deep-strip-remine.md) records the 251 as *"not zero,
so in principle there was something to recover"*, and
[`lambda-levers.md`](lambda-levers.md) says a re-mine *"should recover both the 251 stale keys and
this 0.68 [λ]"*.

**There was nothing to recover.** Those gates do not exist in the current circuit. A later
optimization deleted them outright, which is precisely what the strip existed to do, so **the score
they represented is already banked in the head figure**. No census, at any depth, on any stream,
can recover a gate that is not in the stream. The 251 is a closed line of enquiry, not a pending
0.02%.

The same three edits also removed **1,178 gates the census certified *live***: 344 at `7726431`,
758 at `d6eed9a`, 76 at `8af8a6f`. Those are ordinary optimization wins already in the head score,
and they are the bulk of the 1,429 removals; only 251 of the 1,429 were keyed dead, and 0 were keyed
downgrade.

## The decisive negative: the stream does not explain the 3,165 / 1,727

This is the result that matters for what to do next.

Every shipped key the re-miner fails to certify addresses a tuple that is **still present in the
head stream with unchanged occupancy**:

| key population | occupancy matching the then-current **`801dd20`** stream |
|---|---|
| 3,311 dead keys added after `d9ef3e9` | **3,311 / 3,311 (100.00%)** |
| 1,842 downgrade keys added after `d9ef3e9` | **1,842 / 1,842 (100.00%)** |
| 9,232 dead keys present at `d9ef3e9` | 8,981 / 9,232 (97.28%), and the 251 deleted ones are the whole remainder |
| 2,081 downgrade keys present at `d9ef3e9` | **2,081 / 2,081 (100.00%)** |

The tripwire accepts them, `build_circuit` applies all 12,292 + 3,923 of them, and the resulting
circuit passes 9,024/9,024. So the miner's **25.02% dead / 42.67% downgrade over-observation is
measured on gates that are identical in both streams.** The stream difference cannot be the cause,
and that line of enquiry is now closed.

What remains, from that same plan: the CCZ effective condition (this census folds the target in as
`cond & t & c1 & c2`; the shipped census may not), condition-stack handling of nested
`PushCondition`, and the input distribution the census draws from.

## A lead: the appends are monotonic, which is backwards for depth

The table's last four commits appended **with zero retractions**:

| commit | +dead | −dead | +downgrade | −downgrade |
|---|---|---|---|---|
| `ea785b7` | +59 | 0 | +37 | 0 |
| `37ab267` | +1,236 | 0 | +616 | 0 |
| `7726431` | +311 | 0 | +103 | 0 |
| `5265674` | +253 | 0 | +116 | 0 |

**This is backwards for census deepening.** A deeper census draws more samples, so it observes
*more* gates firing, so it certifies *fewer* dead. The dead set should shrink under deepening, or
at best hold. It grew monotonically, four times, without ever retracting a key. Whatever produced
those 1,859 dead and 872 downgrade additions was not simply more sampling.

The split against the sampling re-miner is correspondingly sharp:

| shipped key population | reproduced by the 120 M-sample re-mine |
|---|---|
| pre-header (`d9ef3e9`-era) dead keys | **8,913 / 9,232, 96.5%** |
| post-header dead additions | **368 / 3,311, 11.1%** |

The re-miner substantially *is* the shallow census, and substantially is not the deep additions.

### The confound, stated plainly

**The added keys are, by construction, the rarest-firing ones.** They are exactly the gates that a
shallower census would not have distinguished from dead, so a sampling miner at 120 M is expected to
disagree about them at a higher rate than about the easy bulk, for reasons that have nothing to do
with the certification mechanism. The 96.5% / 11.1% split is therefore consistent with both
explanations, and does not on its own establish either.

What is *not* explained by the confound is the direction of the monotonic growth. Rarity predicts
that a deeper census disagrees more; it does not predict that a deeper census **certifies strictly
more gates dead and never retracts one**. That asymmetry is the actual signal, and it points at an
additional non-sampling certification layer, an analytic or restricted-reachability argument
applied on top of the sampling census, which would explain both monotonicity and why a purely
statistical re-miner reproduces almost none of it.

**Read this as a lead, not a conclusion.** It is the next thing to test, and the test is to look for
a certification path that can only add: check whether the added keys share a structural property
(register, condition nesting depth, gadget type) that a proof-based pass could key on, rather than
looking for a sampling parameter that would produce them.

## A prior fix that this analysis depends on

The harness-order census mode committed in `7d844fa` surfaced a bug worth restating, because every
measurement above assumes the simulator consumes the XOF the way `src/sim.rs` does. Real `sim.rs`
reads **8 XOF bytes for `R` as well as for `Hmr`** (`src/sim.rs:140–152`), and the census's PRNG
path did not consume for `R`. Any XOF-order run before that fix would have desynchronized the entire
downstream stream, since every measurement outcome after the first `R` would have been drawn from the
wrong offset. It was found and fixed in the previous session, and the fix is what makes
`--harness` mode meaningful at all; it is recorded here because the bug is invisible in PRNG mode
and would silently invalidate any future harness-order census.

## Method and artifacts

**Instrument.** [`../tools/census/dump_gates.rs`](../tools/census/dump_gates.rs), a cargo example
that calls `point_add::build()` and emits every CCX/CCZ in stream order as
`opidx, kind, c2, c1, t, cond, ordinal, occupancy`, plus the full op stream and a per-kind
histogram. Build it with `cargo build --release --example dump_gates` after copying it to
`examples/`, and run it under `SUB4_APPLY_STRIP=0`.

**Gate on the instrument.** Replaying the occupancy tripwire against the head dump reproduces
`build_circuit` exactly, at **12,292 dead accepted / 251 stale, 3,923 downgrades / 0 stale**, so the
dump is the same stream the strip sees. Every count in this document comes from these dumps, not
from `compare_d0.py`, whose occupancy bug is described in [`deep-strip-remine.md`](deep-strip-remine.md).

**Stream walk.** 18 commits from `d9ef3e9` to HEAD, each checked out into a detached worktree,
rebuilt, and dumped. Results in
[`data/stream-walk-by-commit.tsv`](data/stream-walk-by-commit.tsv). Note that the repo's stream
oscillates between 1,361,613 and 1,361,269 across `330d42d`/`cf5aa02`/`37ab267`/`7fa872d`/`7726431`
(submissions toggling the `NORMALIZER_OPS` entry back and forth), so "the stream at the time of the
commit" is not a monotone quantity and cannot be inferred from commit order.

**Data.**

| file | contents |
|---|---|
| [`data/census-vs-head.gates.diff.gz`](data/census-vs-head.gates.diff.gz) | the 354-hunk unified diff between the two streams |
| [`data/stream-walk-by-commit.tsv`](data/stream-walk-by-commit.tsv) | ops / gates / distinct tuples at each of the 18 commits |

The two full gate dumps behind the diff are **not checked in**, at ~12 MB each compressed, and about a
minute apiece to rebuild. [`data/README.md`](data/README.md#regenerating-the-gate-dumps) has the
exact commands, including the detached-worktree recipe for `d9ef3e9`, which predates the tool. Every
occupancy claim in this document is re-derivable from those two dumps; the integrity gate on the
regenerated head dump is `build_circuit`'s own 12,292 / 251 and 3,923 / 0.
