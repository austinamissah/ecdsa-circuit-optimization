# `census` — re-mine the identity-keyed deep strip

> **Status: built and smoke-tested; the full re-mine at `TLM_SCHED_J2_DELTA=2` was still running
> when the session ended. No re-mined key table has been validated yet.** See
> [`../../docs/HANDOFF-2026-08-03-remine.md`](../../docs/HANDOFF-2026-08-03-remine.md).

The census tooling that produced `src/point_add/deep_strip_keys.rs` did not survive — it lived in
`/tmp` and `/dev/shm` on a VM (`memory/05-qubit-reduction.md` step 6). This is a replacement, built
on the fixed-base multiplier and wide-lane simulator from [`../lam-screen/`](../lam-screen/).

**The stream it was mined against did survive**: commit `d9ef3e9`, which rebuilds to exactly the
9,073,163 ops / 1,361,613 CCX+CCZ in the key table's header. See
[`../../docs/census-stream-provenance.md`](../../docs/census-stream-provenance.md) — which also
rules the stream difference out as the reason this miner cannot reproduce the shipped table.
[`dump_gates.rs`](dump_gates.rs) is the instrument for that comparison.

## What it certifies

For every CCX/CCZ in the **unstripped** stream, over many random on-curve input pairs:

| flag | meaning | consequence |
|---|---|---|
| never fired | effect mask always zero | **DEAD** — delete the gate |
| `cond & c2 & ~c1` never fired | `c1` is implied | **DOWNGRADE act=1** — CCX(c2,c1,t) → CX(c2,t) |
| `cond & c1 & ~c2` never fired | `c2` is implied | **DOWNGRADE act=2** — keep `c1` |

CCX's effect mask is `cond & c1 & c2`; CCZ's is `cond & t & c1 & c2`, so CCZ folds its target into
the effective condition. The implication predicate is `memory/03-proven-floors.md`'s
`cond & q1 & ~q2 == 0`, strictly weaker than "always 1" or "controls always equal".

Output is keyed exactly as `apply_deep_strip_identity` expects — `(kind, q_control2, q_control1,
q_target, c_condition, ordinal, tuple_occupancy)` — and the emit pass asserts its gate ordering
agrees with a fresh walk of the stream.

## Usage

```bash
# one shard per seed; shards are independent so any SUBSET can be merged
SUB4_APPLY_STRIP=0 TLM_SCHED_J2_DELTA=2 ./census \
    --mode shard --samples 10000000 --seed 1 --lanes 64 --out shards/s1.bin

# merge and emit; a gate is certified only if clean in EVERY merged shard
SUB4_APPLY_STRIP=0 TLM_SCHED_J2_DELTA=2 ./census \
    --mode emit --shards "s1.bin:10000000,s2.bin:10000000" --out deep_strip_keys.rs
```

Because shards are per-seed, emitting from different subsets gives the **λ-versus-census-depth
curve for free**, and holding two shards back gives a genuine out-of-sample estimate of how many
emitted keys are false. `drivers/finish.sh` does both.

## The depth/λ tradeoff — the thing to understand before using this

A census can only certify "never fired in N samples". A gate whose true fire rate is `p` survives
if `pN` is small, and each surviving false key costs about `9024·p` λ. So

    λ_from_false_keys  ~  (number of dead keys) × 3 / N × 9024

**Census depth buys λ, and stripping more gates costs λ.** The shipped table was mined at 320 M
samples with 12,543 dead keys, and measures at 0.682 ± 0.273 λ
([`../../docs/lambda-levers.md`](../../docs/lambda-levers.md)) — which is the right order for that
formula. Re-mining shallower than 320 M will recover the score but cost *more* λ than the table it
replaces. That is not a flaw in the re-mine; it is what the estimator is.

## Measured throughput (this laptop, 16 threads)

| lanes | samples/s, 1 worker |
|---|---|
| 16 | 3,066 |
| 32 | 3,369 |
| 64 | 3,924 |
| 128 | 4,077 |

Wider lanes amortise the 208 MB compact-op stream but grow the 529,634-entry bit array linearly,
so the two effects nearly cancel: L=128 is only 1.33× L=16. Twelve concurrent workers saturate at
**~14,100 samples/s aggregate**, a 4× per-worker slowdown — this is memory-bandwidth bound, and
320 M samples is a ~6-hour run.

**If you need materially more depth, the fix is to shrink the bit array's working set** (liveness
renumbering of the 529 k classical bits), not to add workers.

## `hotness.rs` — the per-gate charge census

A companion instrument answering a different question from the one above. `census.rs` certifies
whether a gate ever **fires**; [`hotness.rs`](hotness.rs) measures how often it is **charged** —
the popcount of its condition stack over the official 9,024 shots, which is the only thing the
scorer actually bills (`src/sim.rs:77-86`). The two are independent: an unconditional gate that
never fires is charged in full, and a conditioned gate that always fires costs only its hotness.

It reconstructs `eval_circuit`'s Toffoli total exactly and asserts as much before printing anything.
See [`../../docs/gate-hotness-census.md`](../../docs/gate-hotness-census.md) for the measured
distribution on head `6909d15` — bimodal at 1.0 and 0.5, **zero cold gates**, and charge spread so
evenly that the top 100 gates of 1.34 M carry 0.008% of it.
