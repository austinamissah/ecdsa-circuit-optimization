# lf-screen: a classical walk pre-filter for the Leapfrog nonce search

The circuit is validated on 9,024 inputs derived by hashing its own op stream, and the last 96
ops are an X tail that cancels itself out. Changing that tail changes no executed gate but does
change the whole test set, so it works as a free nonce: a circuit passes only if its nonce draws a
test set with no failing shot.

Most failing shots come from the two Leapfrog rail walks (the divide's on `tx - ox`, the multiply's
on `ox - x3`), and those are plain integer arithmetic. This tool replays both walks for every shot
and stops at the first one that fails, so a doomed nonce costs milliseconds instead of a two-minute
simulation.

## What is modeled

A walk fails in one of two ways, and both are certain failures of the shot:

- **Width.** Each step resizes the rails to the width table's next entry. If a value does not
  sign-extend into that width, the circuit frees a dirty wire.
- **Park.** After the last tick both rails must be +-1. The tail batch lends wires from both parked
  rails, so an unparked rail gives a wrong output, not just a random phase.

The choice rule changes by tick: tick 0 has its own sign test, ticks 1 to 136 use the 22-bit
window rule with its late schedule and veto, and the last tick has no forced steps. The screen
reads the window length, the late schedule and the width table from `install_recipe` in
`leapfrog.rs`, and refuses to run if a setting it depends on has a value it does not model.

## How it was checked (at upstream `0c3a075`)

| | |
|---|---|
| Simulator, 14 nonces (777001 to 777014), 126,336 shots | 180 predicted failures, all 180 failed; 0 false positives |
| Simulator failures the walk model cannot see | 78 (payload arithmetic), 5.6 per 9,024 |
| Measured failure rate | 18.4 per 9,024 = 12.9 walk + 5.6 payload |
| Rust against Python, same 100,000 random denominators | identical: 87 failures, same sites |
| Rust against Python, the 126,336 real shots | identical inputs and verdicts |
| Submitted nonce 1048009047001 | 0 walk failures on 9,024 shots |

The per-walk failure rate is about 7.3e-4 (740 in 1,000,000 random denominators).

## What it is worth

On 16 threads the screen does about 190 nonces/s; the mean first failing shot is 696. A nonce
survives the walks with probability about e^-12.9, so roughly one survivor every 35 minutes. Each
survivor still needs `eval_circuit`, since the payload misses (e^-5.6, about 1 in 270) are not
modeled. A fully clean nonce costs about e^18.4, or 1e8 draws, whatever the filter does: about
6 days of screening on this machine, plus roughly 270 survivor evals.

## Files

| | |
|---|---|
| `screen.rs` | the Rust screen: walk, recipe reader, secp256k1, Fiat-Shamir seeding |
| `lf_model.py` | the Python reference model the screen was ported from |
| `lf_model_peel139.py` | the model for the 139-tick walk at `58229f8`, kept for reference |
| `lfcheck.rs` | per-shot simulator dump (nonce, shot, inputs, wrong, phase) for validation |
| `compare2.py` | Python model against an `lfcheck` dump, broken down by failure site |

## Build and run

Neither binary lives in `src/`, so neither can end up in a submission:

```
cp tools/lf-screen/screen.rs src/bin/lf_screen.rs
cargo build --release --bin lf_screen
rm src/bin/lf_screen.rs

./target/release/lf_screen --ops ops.bin --from 0 --count 100000 --out survivors.txt
./target/release/lf_screen --ops ops.bin --nonce 777001 --full    # walk-failing shots, no early abort
./target/release/lf_screen --ops ops.bin --nonce 777001 --dump    # one line per shot with failure sites
./target/release/lf_screen --walks 1000000                        # per-walk failure rate
```

`ops.bin` must be built from the same tree whose `leapfrog.rs` the screen reads (`--src`, default
`src/point_add`), because the seed is a hash of the whole stream.
