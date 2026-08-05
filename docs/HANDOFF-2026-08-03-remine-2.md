# Handoff: deep-strip re-mine, second pass

Read alongside [`census-miner-validation.md`](census-miner-validation.md) and
[`HANDOFF-2026-08-03-remine.md`](HANDOFF-2026-08-03-remine.md).

> **Head: `801dd20`** (score 1,487,590,242, λ_total 20.04). Dated session state, kept as a record.
> The fork was rebased onto `ed4b529` / `6909d15` later the same day. Current score is
> 1,486,468,554, λ_total 20.560 ([`lambda-6909d15.md`](lambda-6909d15.md)). Every score and λ
> figure below is priced against the superseded head, and the re-mined table is keyed to the old
> stream ([`rebase-2026-08-03-upstream-ed4b529.md`](rebase-2026-08-03-upstream-ed4b529.md#what-this-invalidates)).

## Where the score stands

| circuit | score | vs head |
|---|---|---|
| head (shipped, delta 0, shipped strip) | 1,487,590,242 | n/a |
| delta 2, strip OFF | 1,523,940,495 | +2.444% |
| **delta 2, re-mined strip ON** | **1,510,597,935** | **+1.547%** |
| delta 0, re-mined strip ON | 1,492,415,116 | +0.324% |

Delta 2 with the re-mined strip is the best λ-lever configuration: λ_total
20.04 → ~8, and λ_classical 5.777 ± 0.115 against 5.787 ± 0.125 strip-off, so
the strip is **λ-free**. Harness-verified: 7 classical / 6 phase, **0 stale
keys**.

**The 0.897% the re-mined strip recovers is a FLOOR, not the true available
score.** The miner certifies a strict subset of what is genuinely dead (below),
so a correct miner would recover more and the real net at delta 2 is better than
+1.547% by an unknown margin.

## Delta 0 is closed

- Stale keys of the shipped table against the current head stream: **251**, not
  zero, so in principle there was something to recover.
  **Corrected 2026-08-03: there was not.** All 251 went stale at commit `d6eed9a`
  because their operand tuple was *deleted from the stream*; none are occupancy
  drift. The gates do not exist and the score is already banked. See
  [`census-stream-provenance.md`](census-stream-provenance.md).
- The miner **cannot certify replacements for those 251**, and separately misses
  **3,165 dead** and **1,727 downgrade** keys the shipped table finds.
- Re-mined delta 0 scores **+0.324% against head**, which is a loss. λ_classical
  15.258 ± 0.202 against 15.342 ± 0.189 strip-off, so λ-free, but that buys
  nothing when the score moves the wrong way.

**Do not pursue a delta-0 re-mine with this miner.**

## Known-answer test: the miner FAILS, conservatively

Replaying the tripwire against a per-gate dump reproduces `build_circuit`
exactly, at dead 12,292 accepted / 251 stale and downgrade 3,923 / 0, so the
**keying is provably correct**. The certification predicates are not: of the
shipped keys the tripwire accepts, in a circuit that passes 9,024/9,024, this
census claims **3,076 dead keys fire (25.02%)** and **1,674 downgrades are
violated (42.67%)**.

The shipped table gives 0/0/0, so it is right and this census over-observes
firing. Direction matters: **0 keys in the re-mine are absent from the shipped
table**, a strict subset, so safe but under-claiming. That is why the
delta-2 table came out correct and λ-free despite the miner being wrong.

## Harness-order hypothesis: NOT RESOLVED, and now doubtful

The hypothesis: the shipped census ran in the harness's XOF order (W=64), saw a
narrower reachable set and so certified more gates dead, while this miner drives
Hmr/R from a xorshift PRNG and explores freely.

A harness-order mode is implemented and committed (`--harness`: W=64, inputs and
the Hmr/R stream both from the real Fiat–Shamir XOF, pairs drawn up front,
simulator continuing from the same reader). Writing it surfaced a real bug:
**real `sim.rs` reads 8 XOF bytes for `R` as well as `Hmr`**, and the PRNG path
did not consume for `R`, so any XOF-order run would have desynchronized the
entire downstream stream. Fixed.

**The comparison did not complete.** The PRNG arm at 1 M samples finished:
**19,719 never-fired, 2,170 c1-implied, 3,081 c2-implied**. The harness arm did
not finish and was killed.

**Why it did not finish is the useful part.** Harness order needs **1,013,644
XOF words per 64-shot pass**, which is 8 MB of SHAKE256 output for every 64 shots. 1 M
samples is 15,625 passes ≈ 127 GB of SHAKE output; the 320 M-sample census the
shipped table claims would need ~40 TB. **The shipped census therefore almost
certainly was not taken in harness order**, which undercuts the hypothesis on
cost grounds before the measurement lands. Something else explains the 25%/43%
disagreement.

### Stream-specificity, if harness-order mining ever does win

**The resulting table would be stream-specific and NOT safe to submit.** A
census taken in one XOF order certifies gates dead only for the measurement
outcomes that stream produces. The scored circuit's stream is determined by its
own op sequence, and every score-lowering edit re-rolls it, so a table tuned to
one stream would silently delete live gates under another, which is exactly the
failure mode in `04-traps.md` §1 and the reason the occupancy tripwire exists.
**Only a stream-agnostic census is shippable.** If harness-order mining wins on
counts, that is evidence the shipped table is fragile, not a license to copy it.

## A bug worth knowing about

`compare_d0.py` built its occupancy map only from tuples present in the emitted
key table, so tuples whose gates are all live were miscounted as occupancy
mismatches: it inferred **2,715** stale keys where the build reports **251**.
**Every number in this document and in `census-miner-validation.md` comes from
the per-gate dump (`--mode dump`), not from that script.**

## What to run next, in order

1. ~~**Diff the streams first, cheapest and most likely cause.** The shipped
   census header says **9,073,163 ops / 1,361,613 CCX+CCZ**; the current head's
   unstripped stream is **9,070,297 / 1,360,635**. It was mined on a materially
   different circuit. Establish what that stream was before anything else.~~
   **Done 2026-08-03, and it is NOT the cause.** See
   [`census-stream-provenance.md`](census-stream-provenance.md). The census stream
   is commit `d9ef3e9`; the header is accurate and all 16,466 keys stamp that one
   stream at 100%. Head differs by 978 CCX in three attributable edits, which
   explains all 251 stale keys and **none** of the 3,165/1,727 gap: every
   un-reproduced key addresses a tuple still present in head with unchanged
   occupancy. Go straight to item 2.
2. Then the two remaining suspects for the over-observation: (a) the CCZ
   effective condition. This census folds the target in (`cond & t & c1 & c2`)
   and the shipped census may not; (b) condition-stack handling of nested
   `PushCondition`.
3. Only once the miner reproduces the shipped 12,543 / 3,923 is a re-mine worth
   trusting for score.
4. Independently of all the above: **λ_total for delta 2 with the re-mined strip
   is unmeasured.** Only λ_classical was measured, n=400, from the screen.
