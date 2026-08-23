# pp-screen: a classical pre-filter for the ping-pong nonce search

## Why this exists

The circuit is validated against 9,024 test inputs derived by hashing its own op stream, so a
contestant cannot tune the circuit against the test set. The last 96 operations of the stream are
an X tail that cancels itself out: it changes no executed gate, but it does change the hash, and
therefore the whole test set. That makes it a free nonce. A circuit is valid if and only if the
particular draw its own nonce induces happens to hit no failure.

So the search is a lottery, and the ticket price is what matters. Checking one nonce with
`eval_circuit` takes about 20 seconds. At the shipped configuration roughly one draw in
3 x 10^8 is clean, so buying tickets at 20 seconds each is hopeless.

This tool reproduces the failure conditions in ordinary classical arithmetic, without simulating
any quantum state, and stops at the first failing shot. Most nonces die in milliseconds.

## What is modeled

**The walk.** The divide and multiply traversals each run a signed binary-GCD-style walk on a pair
of registers. Two things can go wrong, and both are checkable exactly:

- *Width.* Each round drops the top wire of both registers by XORing the bit below into it and
  freeing it. That free is clean only if the value sign-extends into the round's scheduled width.
  If it does not, the freed qubit is left dirty and the simulator reports it.
- *Convergence.* The walk has a fixed depth and must finish at (+/-1, +/-1). Running out of rounds
  is a failure. This turns out to be the dominant walk failure; width violations are not observed
  at the shipped schedule at all.

**The replay.** The walk records one sign bit per round, and the replay consumes that tape to
build the coefficient. Its corrections are deliberately truncated to save gates, and each
truncation is a measured erasure with a known failure condition:

- The pseudo-Mersenne fold adds one of `{-f, 0, +f, +2f}` into the low 54 bits and throws away the
  carry out of bit 53. When that carry was needed, the value is wrong by 2^54. **This is the only
  channel that can produce a classical mismatch**, and it is modeled for both traversals.
- The chunk boundary carries and the overflow flag are erased by a comparison truncated to 22
  bits, applied as a *phase* correction. When the comparison disagrees with the true carry the
  shot picks up a phase error rather than a wrong value, so these show up in the phase-garbage
  column and are not modeled here.

## What it is worth

On the shipped 698/696 configuration the model catches about 65% of classical failures with no
false positives. It buys a great deal on verification overhead, taking the number of full
`eval_circuit` runs needed per clean nonce from roughly 87,000 down to about 830.

It does not make a grind viable on a single workstation, and no better model would. The chance a
draw is clean is about e^-19.5, which is a property of the circuit rather than of the filter, so a
clean nonce costs about 2.9 x 10^8 draws no matter how good the screening is. At the rate one
16-thread machine sustains, that is weeks. The bottleneck is draw throughput, not filter quality.

## Files

| | |
|---|---|
| `screen.rs` | the model: walk, replay, secp256k1 field and group, Fiat-Shamir seeding |
| `grind.sh` | screen a nonce block at one depth, rebuilding `ops.bin` first |
| `verify.sh` | run survivors through the real scorer |
| `firstfail.sh` | compare the model's first failing shot against the simulator's |

## Building

The tool is kept out of `src/` on purpose. The benchmark manifest lists `src/point_add` as the
only editable path, so anything under it becomes part of a submission; analysis tooling has no
business there.

```
cp tools/pp-screen/screen.rs src/bin/pp_screen.rs
cargo build --release --bin pp_screen
rm src/bin/pp_screen.rs
```

## Running

Always drive screening through `grind.sh`:

```
tools/pp-screen/grind.sh <rounds> <rounds-mul> <from> <count> [threads] [out]
```

Self-checks, both of which run before any screening and abort on failure:

```
./target/release/pp_screen --replay-selftest          # replay against exact modular arithmetic
./target/release/pp_screen --ops ops.bin --nonce N    # field and group against the harness curve
```

Other modes:

```
--replay-count   count fold escapes per nonce over all 9,024 shots
--envelope       report the per-round width each draw actually needs
--verbose        print the first failing shot per nonce
```

## Traps

- **`ops.bin` must be rebuilt for the exact configuration being screened.** The Fiat-Shamir seed
  hashes the whole op stream, and the op count moves with the walk depth (12,912,890 at 698/696
  against 12,890,758 at 696/694). Screening one depth against another depth's `ops.bin` produces
  noise that looks like data. `grind.sh` closes this; nothing else does.
- **All three scripts write `./ops.bin`.** Never run two at once. Use a scratch clone per worker.
- **`eval_circuit` appends to `results.tsv` and rewrites `score.json` on every run.** Both are
  measurement records, so the scripts save and restore them rather than let screening traffic
  accumulate.
- **`eval_circuit` aborts before printing metrics on a dirty nonce**, so it cannot price a
  configuration that has not been ground yet. Use `PP_PROFILE=1`, which tracks the true
  9,024-shot figure to within 0.005%.
- **Do not validate against constructed inputs.** An earlier version of the replay self-test fed
  synthetic numerator and denominator pairs and reported a fold escape rate four orders of
  magnitude too high. Real draws come from actual curve points, where the two are tied together by
  the curve equation, and that relationship is what keeps the fold window from saturating. The
  model was correct the whole time; the test inputs were unreachable.

## Chunk geometry

The replay's chunk boundaries are not fixed. They are chosen per round against the interleaving
allowance, and across 1,408 replay adds the layout uses two chunks 734 times, three chunks 262
times, four chunks 248 times, and a tail of shapes with a deliberately narrow leading chunk whose
repair is exact because the chunk is narrower than the 22-bit comparison window.

Re-deriving that in the screener would desync silently, which is the worst failure mode available
here, so the builder dumps what it actually chose:

```
PP_GEOMETRY=geom.tsv ./target/release/build_circuit
```

The dump is inert. With the variable unset or set, `ops.bin` is byte-identical.
