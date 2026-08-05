# `lam-screen`: the λ instrument

> **Status: built, run, and re-gated 2026-08-03 on head `801dd20`.** Reproduces the full harness's
> per-nonce classical mismatch **count** on **199 / 199** nonces, exactly, at two independent lane
> widths. **14.2×** the harness on throughput.

This is [`../nonce-screen/`](../nonce-screen/) with the two hot paths replaced. Same seed, same
pairs, same comparison, same counts. It is the *speed* that changed, and the gate is the proof
that nothing else did. Read `../nonce-screen/README.md` first: everything it says about what a
screen can and cannot tell you applies here unchanged.

## Gate result

Every one of the 199 non-control nonces of the λ sweep, against the harness counts in
[`../../docs/data/lambda-sweep-801dd20.tsv`](../../docs/data/lambda-sweep-801dd20.tsv):

| lane width | compared | exact | harness total | screen total | distinct fingerprints |
|---|---|---|---|---|---|
| `--lanes 4` (256 shots/pass) | 199 | **199 / 199** | 3,230 | 3,230 | 199 / 199 |
| `--lanes 16` (1,024 shots/pass) | 199 | **199 / 199** | 3,230 | 3,230 | 199 / 199 |

Raw output: [`../../docs/data/lamscreen-gate-801dd20.tsv`](../../docs/data/lamscreen-gate-801dd20.tsv).

The two widths agreeing is a second, independent check: the wide simulator is licensed by
`memory/04-traps.md` §4 (classical outcomes are insensitive to the value and consumption order of
the Hmr/R stream), and if that license were wrong, L=4 and L=16 would consume the randomness
differently and diverge.

## What was changed, and what it bought

Per nonce, one idle core, 9,024 shots:

| | pairs | simulate | total |
|---|---|---|---|
| `nonce-screen` | 12.5 s | 22.8 s | **48.6 s** |
| `lam-screen --lanes 16` | 1.6 s | 3.1 s | **4.7 s** |

**1. Fixed-base scalar multiplication.** `WeierstrassEllipticCurve::mul` is an affine
double-and-add that pays a modular inversion on every one of its ~384 group operations, and the
screen calls it 18,048 times per nonce to draw the test pairs. `FastBase` is an 8-bit fixed-base
window table plus Jacobian accumulation: 32 mixed additions and exactly one inversion, 79 µs
against 1.46 ms. `--selftest N` asserts bit-equality against the library routine over N XOF-drawn
scalars plus `k ∈ {0, 1, 255, 256, order}`.

**2. A wide-lane simulator.** `sim::Simulator` bitslices 64 shots into one `u64` and walks all
9.06 M ops once per batch: 141 passes over 507 MB, plus a random read into a 4.2 MB bit array for
every conditioned op. `--lanes L` gives `W = 64·L` shots per pass, so the bit-array read becomes L
consecutive words (one cache line up to L = 8) and the miss count per op is unchanged while L times
the shots ride on it. Alongside that, a 24-byte packed op replaces the 56-byte `Op`, the 535,472
phase-only ops and 1,028 structural ops are dropped (9,058,005 → 8,521,505), and a xorshift PRNG
feeds the Hmr/R lanes in place of 1.01 M 8-byte SHAKE256 squeezes per pass.

`--lanes 0` keeps the library simulator as a reference path.

## Throughput

| | nonces/hour |
|---|---|
| full harness (`./benchmark.sh`) | 205 |
| `nonce-screen`, 8 workers | ~795 |
| `lam-screen --lanes 4`, 8 workers | 2,523 |
| `lam-screen --lanes 16`, 8 workers | 2,912 |
| `lam-screen --lanes 16`, 10 workers | ~4,360 |

On one core that is `ln(110/4.7) = 3.1` λ-units bought rather than the screen's 2.2, still nowhere
near the ~11 needed. **It does not make a grind feasible; it makes λ work fast enough to explore
with.** See [`../../docs/lambda-levers.md`](../../docs/lambda-levers.md) for what it was used for.

## Building

Deliberately not under `src/bin/`, so cargo does not auto-discover it and it cannot affect the
submission tree's build.

```bash
cp tools/lam-screen/lamscreen.rs /path/to/throwaway-repo-copy/src/bin/
cd /path/to/throwaway-repo-copy
cargo build --release --locked --offline --bin lamscreen

./target/release/lamscreen --selftest 3000                        # multiplier self-check
./target/release/lamscreen --nonces LIST --mode count --lanes 16 --out OUT.tsv
```

`--mode count` runs all 9,024 shots and is what a λ measurement needs; `--mode ladder` walks the
512 / 2,048 / 8,192 / 9,024 rungs and stops at the first mismatch, which is the fast path for
screening but reports a truncated count. The build is ~24 s per **process**, so batch as many
nonces per invocation as possible.

## Invariants, unchanged from `nonce-screen`

- All test pairs for a rung are drawn before the simulator is constructed.
- Every generated stream is fingerprinted; two identical fingerprints across distinct nonces mean
  the tail edit never reached the stream.
- avgT is never read from this binary, since it is W=64-harness-order only.
- **Re-run the gate against any new circuit head before trusting it there.**
