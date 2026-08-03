# `nonce-screen` — UNBUILT DRAFT

> **Status: never compiled, never run, never validated.** Written 2026-08-02 and parked before the
> first `cargo build`. It may not typecheck. Nothing here is a measurement.

Stored so the design survives; not a tool yet.

## Why it exists

Finding a shippable circuit requires a nonce whose 9,024 test inputs all pass, and at the measured
λ_total ≈ 20 that costs ~5 × 10⁸ trials ([`../../docs/lambda-measurement.md`](../../docs/lambda-measurement.md)).
At the harness's 61 s/trial that is ~60 wall-years on 16 cores. A screen at ~1.2 s/trial is worth
about 4 λ-units — cheaper to buy in engineering than in circuit correctness, which is why it was
started first.

The two intended savings are structural, not clever:

1. **No rebuild per nonce.** `apply_tail_nonce` (`src/point_add/mod.rs:1792`) rewrites only
   `q_target` on the last 96 ops, so the stream is patched in place.
2. **No re-hash per nonce.** `fiat_shamir_seed` (`src/bin/eval_circuit.rs:204`) is a *streaming*
   SHAKE256 absorb, so the state over `ops[0 .. n-96]` is absorbed once and cloned — 5,376 bytes
   per nonce instead of ~507 MB.

A 512 / 2,048 / 8,192 / 9,024 shot ladder with early exit supplies a further ~7.2× at λ = 20. That
ladder is upstream's, read from their controller
([`../../docs/upstream-search-economics.md`](../../docs/upstream-search-economics.md)).

## The gate it must pass first

It must reproduce the full harness's per-nonce classical mismatch **count** — not just its
clean/dirty verdict — on all 199 nonces in
[`../../docs/data/lambda-sweep-801dd20.tsv`](../../docs/data/lambda-sweep-801dd20.tsv).
**Exact agreement on every one.** Correlation is not sufficient. Report the paired distribution of
(screen count − harness count); any nonzero entry is a bug in the screen.

Three trap checks, each of which has already caught someone on this project:

- **Draw all test pairs for a rung before constructing the `Simulator`.** It continues from the same
  XOF reader, so lazy drawing makes it consume bytes the input draw still needs — yielding
  valid-but-wrong curve points that never mismatch and report false clean.
  (`src/point_add/memory/04-traps.md` §4; it cost its author a 1,344-vCPU grind.)
- **Hash every generated stream.** Two identical hashes across distinct nonces means the tail edit
  never reached the stream. This is what caught the sudo/`env_reset` problem during the λ sweep.
- **Never read avgT from the screen.** W=64 harness order only.

## Known limitation, by construction

Classical channel only. With λ_phase_only = 3.80, a screen-clean nonce still has
P(phase-clean) = e^-3.80 ≈ 2.2 × 10⁻², so **hits are candidates requiring full-harness
confirmation** — roughly 45 candidates per true seed. Never call a hit a clean seed.

## Building it

Deliberately **not** under `src/bin/`, so cargo does not auto-discover it and a broken draft cannot
break `cargo build` or `./benchmark.sh`. To work on it, copy `screen.rs` into `src/bin/` of a
throwaway copy of the repo — never the submission tree — and:

```bash
cargo build --release --bin screen
./target/release/screen --nonces nonces.txt --mode count --out screen.tsv
```

`--mode count` runs all 9,024 shots and is what the validation gate needs; `--mode ladder` is the
fast path. No new dependencies; it links the `quantum_ecc` lib.
