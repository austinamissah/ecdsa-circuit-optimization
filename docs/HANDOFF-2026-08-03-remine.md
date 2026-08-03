# Handoff — census re-mine at `TLM_SCHED_J2_DELTA=2`, incomplete

Written on running out of context mid-run. **No re-mined key table was produced or validated.**
Everything below is committed.

## What was asked

Re-mine `deep_strip_keys` against a delta-2 stream and report: score with the strip off, score with
the re-mined strip on, net position vs the 1,487,590,242 head, and whether the re-mine recovers the
0.682 λ the current strip costs.

## What exists now

The census tooling did not survive its original run
(`memory/05-qubit-reduction.md` step 6), so it had to be rewritten.
[`../tools/census/`](../tools/census/) is that replacement — built, smoke-tested, and committed,
with drivers for the full pipeline. Its README documents the predicates, the key format, measured
throughput, and the depth/λ tradeoff.

**Smoke test, delta 2, 65,536 samples:** 9,214,624 ops / **1,379,831 CCX+CCZ**; 43,081 never-fired,
3,962 c1-implied, 5,963 c2-implied. (Those counts are meaningless as a key table — at 65 k samples
most "never fired" gates simply have not fired *yet*. They only confirm the tool runs and finds a
plausible population.)

## What was mid-flight

A 120 M-sample census — 10 mining shards + 2 held-out, 10 M each, `--lanes 64` — launched ~09:40
and due to finish ~12:05. Shards land in `scratchpad/remine/shards/`, and `drivers/finish.sh` was
chained to run automatically when all 12 appear: it emits keys from the mining shards, re-emits
including the held-out pair, diffs the two to count keys the mining census got wrong, installs the
result, rebuilds, verifies on the **full harness**, and measures λ at n=400.

**If that chain completed, its output is in `scratchpad/remine/finish.log` — but nothing in it has
been checked by me. Treat it as unverified until the harness row is read directly.**

## The thing to know before continuing

I under-planned the compute. The shipped table was mined at **320 M** samples; the run I could
afford is **120 M**. That matters because a census only certifies "never fired in N samples", so

    λ_from_false_keys  ~  (dead keys) × 3 / N × 9024

**A 120 M re-mine will recover the score but should cost roughly 2–3× the 0.682 λ it replaces.**
So the honest expected answer to "does the re-mine also recover the 0.682 λ" is **no, not at this
depth** — and the useful deliverable is the λ-versus-depth curve, which the per-seed shards give
for free by emitting from different subsets. That framing is set up in the drivers but unmeasured.

Throughput is the binding constraint: 12 workers saturate at ~14,100 samples/s (memory-bandwidth
bound on the 208 MB op stream plus the 529,634-entry bit array), so 320 M is ~6 hours.

## What I would do next, in order

1. **Read `finish.log`.** If the chain ran, check the harness row first — classical/phase in the
   intrinsic band means the keying and predicates are right, which is the main correctness risk.
   Thousands of mismatches means a predicate bug, not a shallow census.
2. **Emit at several depths** (20 M / 60 M / 120 M subsets) and measure λ at each. That curve is
   the real answer to the question, and it is cheap once the shards exist.
3. **Only then decide the depth.** If the goal is to beat the shipped table's 0.682 λ, the census
   must go deeper than 320 M, which needs the bit-array working-set fix below.
4. **Shrink the bit array** by liveness-renumbering the 529 k classical bits. That is the single
   change that would make deep censuses affordable on this machine; adding workers will not.

## Correctness checks that are NOT yet done

- Full-harness verification of any re-mined table.
- λ at n≥12 paired for the re-mined strip.
- A delta-0 control re-mine, which would let the tool be checked against the shipped
  12,543 dead / 3,923 downgrade counts — the strongest available validation of the predicates, and
  worth doing before trusting any delta-2 table.
