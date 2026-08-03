# `nonce-screen` — validated instrument

> **Status: built, run, and gated 2026-08-02 on head `801dd20`.** Reproduces the full harness's
> per-nonce classical mismatch **count** on **199 / 199** nonces, exactly. Paired difference
> (screen − harness) is a single spike at zero.

This is the instrument for λ work. It is **not** a submission tool and it cannot find a clean seed
on its own — see [Known limitation](#known-limitation-by-construction).

## Gate result

| | |
|---|---|
| nonces compared | **199** (every non-control row of the λ sweep) |
| exact agreement | **199 / 199** |
| paired difference | min 0, max 0, mean 0.000000, nonzero entries **0** |
| total mismatches | harness 3,230 · screen 3,230 |
| distinct stream fingerprints | 199 / 199 |

The bar was exact agreement on the **count**, not correlation and not the clean/dirty verdict, so
that the screen can be trusted to rank near-misses and not merely to reject. It cleared that bar.

Evidence, committed:

- [`../../docs/data/screen-gate-801dd20-paired.tsv`](../../docs/data/screen-gate-801dd20-paired.tsv) — harness vs screen, per nonce, with the difference column
- [`../../docs/data/screen-gate-801dd20.tsv`](../../docs/data/screen-gate-801dd20.tsv) — raw screen output
- [`../../docs/data/lambda-sweep-801dd20.tsv`](../../docs/data/lambda-sweep-801dd20.tsv) — the harness reference it was checked against

**Re-run the gate against any new circuit head before trusting the screen there.** It validates a
transcription of `eval_circuit`'s test loop, and that is only known correct for the stream it was
checked on.

## Measured throughput

Uncontended, on the machine described in [`../../docs/lambda-measurement.md`](../../docs/lambda-measurement.md):

| | per nonce | note |
|---|---|---|
| `./benchmark.sh` | **110 s** | build 59 s + eval 57 s, paid on every trial |
| `screen --mode count` | **~55 s** | eval-equivalent; build amortised |
| `screen --mode ladder` | **~12 s** | mean over 20 nonces |

**~9× over the harness.** The saving is the rebuild, not the simulation: `point_add::build()` is
~59 s and the harness pays it every single trial, while the screen pays it once per process. So
**batch as many nonces per invocation as possible** — a one-nonce invocation is worse than useless.

Ladder rung distribution over those 20 nonces, which is what the 7.2× theoretical saving looks
like in practice: 12 stopped at 512 shots, 6 at 2,048, 2 at 8,192.

## Why it is fast

1. **No rebuild per nonce.** `apply_tail_nonce` (`src/point_add/mod.rs:1792`) rewrites only
   `q_target` on the last 96 ops, so the stream is patched in place.
2. **No re-hash per nonce.** `fiat_shamir_seed` (`src/bin/eval_circuit.rs:204`) is a *streaming*
   SHAKE256 absorb, so the state over `ops[0 .. n-96]` is absorbed once and cloned — 5,376 bytes
   per nonce instead of ~507 MB.
3. **Early rejection.** A 512 / 2,048 / 8,192 / 9,024 shot ladder, stopping at the first rung that
   shows a mismatch. The ladder is upstream's, read from their controller
   ([`../../docs/upstream-search-economics.md`](../../docs/upstream-search-economics.md)).

## Known limitation, by construction

Classical channel only. With λ_phase_only = 3.80, a screen-clean nonce still has
P(phase-clean) = e^-3.80 ≈ 2.2 × 10⁻², so **hits are CANDIDATES requiring full-harness
confirmation** — roughly **45 candidates per true seed**. Never call a hit a clean seed. Doing so
would be the same class of error as the lazy-XOF bug in `src/point_add/memory/04-traps.md` §4.

Not reproduced at all: phase-garbage, ancilla-garbage, avgT. avgT is W=64-harness-order only and
this binary must never report it.

## Three invariants that keep it honest

Each has already caught a real bug on this project:

- **All test pairs for a rung are drawn before the `Simulator` is constructed.** It continues from
  the same XOF reader, so lazy drawing would make it consume bytes the input draw still needs —
  yielding valid-but-wrong curve points that never mismatch and report false clean.
- **Every generated stream is fingerprinted** (`stream_fp`, SHAKE256 over the whole op stream).
  Two identical fingerprints across distinct nonces means the tail edit never reached the stream.
- **avgT is never read from the screen.**

## Building

Deliberately **not** under `src/bin/`, so cargo does not auto-discover it and it cannot affect the
submission tree's build. Verified via `cargo metadata` that the only targets remain the lib and the
two harness binaries.

```bash
cp tools/nonce-screen/screen.rs /path/to/throwaway-repo-copy/src/bin/
cd /path/to/throwaway-repo-copy
cargo build --release --locked --offline --bin screen

./target/release/screen --nonces LIST --mode count  --out OUT.tsv   # gate mode
./target/release/screen --nonces LIST --mode ladder --out OUT.tsv   # fast path
```

No new dependencies; it links the `quantum_ecc` lib.
