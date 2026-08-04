# Fire versus charge — 76.7% of the score is provably wasted, and provably unreachable

> Measured 2026-08-03 on head `6909d15` (score 1,486,468,554). Instrument:
> [`../tools/census/hotness.rs`](../tools/census/hotness.rs), extended to count fires alongside
> charges. Follows [`gate-hotness-census.md`](gate-hotness-census.md), which established that the
> scorer bills the condition stack and ignores the controls.

## Result

Same instrument, same gate — the attributed charge still reconstructs the scorer's total exactly
(`attributed = toffoli_gates = 11,623,826,906`, `avgT = 1288101.386`) — now also accumulating, per
gate, the number of shots on which its **effect mask is non-zero**.

| quantity | shot-charges | as avgT | share of score |
|---|---:|---:|---:|
| total charge | 11,623,826,906 | 1,288,101.4 | 100% |
| of which the gate actually **fires** | 2,708,896,591 | 300,188.0 | 23.30% |
| **cooling headroom** (charged but does nothing) | **8,914,930,315** | **987,913.4** | **76.70%** |

A semantics-preserving condition must be true on every shot where the gate fires, so
`charge − fire` is the most any condition could save. Summed over the stream, and granting a
**perfect oracle condition to every one of the 1,343,361 gates at once**, the ceiling on the cooling
lever is 987,913 avgT — 76.7% of the entire score.

That number is real, and it is also almost entirely unreachable. The reason is in the distribution,
not in the total.

## Why it is unreachable: fire is quantum, conditions are classical

Fire-rate histogram over the 1,232,871 full-hotness (unconditional) gates:

| fire rate | gates | share |
|---|---:|---:|
| [0.00, 0.05) | 129,743 | 10.52% |
| [0.05, 0.20) | 111,233 | 9.02% |
| **[0.20, 0.30)** | **741,489** | **60.14%** |
| [0.30, 0.40) | 246,788 | 20.02% |
| [0.40, 1.00] | 3,618 | 0.29% |

**The mode is 0.25, and 0.25 is exactly the rate at which a Toffoli with two independent uniform
controls fires.** These gates are not wasteful; they are Toffolis doing what Toffolis do. Three
quarters of a Toffoli's charge is the price of the shots where its controls happen not to be 1.

And that is precisely the part no condition can remove:

- The condition stack takes a **classical bit** (`PushCondition(BitId)`), and the charge is
  `popcount` of that mask (`src/sim.rs:77-86`).
- Whether a gate fires is a function of its **quantum control qubits**.
- In this circuit the classical bits are measurement outcomes (`Hmr`) — which is why the 8.2% of
  gates that *are* conditioned sit at hotness 0.4999, a fair coin
  ([`gate-hotness-census.md`](gate-hotness-census.md)).

So cooling a gate requires a classical bit that is true whenever the quantum controls are both 1.
No such bit exists to be found: a measurement outcome is independent of the control values, and the
probability that a given gate's ~2,256 firing shots all fall inside a given fair coin's 4,512 true
shots is `2^-2256`, or about `10^-679`. Manufacturing the bit instead means *measuring* the
controls — an `Hmr`, which destroys the qubit and costs Cliffords, to save at most 0.75 of one
Toffoli.

**This is a structural block, not a search that came up empty.** The 76.7% is the gap between "a
Toffoli is charged" and "a Toffoli's controls are both 1", and closing it would require classically
knowing quantum data. No candidate set was implemented because the candidate class is empty for a
reason that does not depend on how hard one looks.

## What the census did surface: 46,134 gates that never fire

Of the post-strip stream, **46,134 gates (3.4%) fired on none of the 9,024 official shots**,
carrying 389,977,573 shot-charges = **43,215.6 avgT, 3.35% of the score**. Only 80 gates in the
whole circuit fire on every shot they are charged for.

Against a strict-beat bar of **0.886 avgT** — the head is `1,288,101 × 1154`, so
`round(avgT) ≤ 1,288,100` wins by 1,154 points — deleting a *single* never-firing full-hotness gate
would clear it.

**It is not harvestable, and the reason is the trap this project has already documented twice.**
"Never fires on the official 9,024 shots" is a certificate about *one draw*. The 9,024 test inputs
are a SHAKE256 hash of the whole op stream (`eval_circuit.rs:204`), so deleting any gate re-rolls
every one of them, and the gates that were quiet on the old draw are not the gates that will be
quiet on the new one. This is `memory/04-traps.md` §1 exactly, and it is the same stream-specificity
objection that
[`HANDOFF-2026-08-03-remine-2.md`](HANDOFF-2026-08-03-remine-2.md#stream-specificity-if-harness-order-mining-ever-does-win)
raised against harness-order census mining: **only a stream-agnostic certificate is shippable.**

The quantitative version: these 46,134 gates survived upstream's `union max-coverage` census, so
they are not census-dead — they are gates whose fire probability is small enough to miss in 9,024
draws but not in the census's. Deleting a gate with true fire rate `p` costs about `9024·p` λ. At
`p ≈ 1e-5` the whole set would cost ≈ 4,163 λ; the circuit currently has to hit λ ≈ 0 to ship. There
is no subset of this set that is both large enough to matter and safe, because the ones large enough
to matter are exactly the ones with the highest `p`.

## The honest summary

- The mechanism in `CEILING.md` is correctly stated and now exactly quantified: 76.70% of the score
  is charge on gates that do nothing that shot.
- It is unreachable by conditioning, because fire depends on quantum controls and conditions are
  classical bits that are independent of them.
- The 3.35% sitting on never-firing gates is reachable only by deleting them, and "never fired on
  this draw" is not a property that survives the deletion.
- **No cooling candidate was implemented because none exists.** This closes the lever rather than
  parking it.

The instrument is worth keeping regardless: `charge − fire` per gate is the correct way to price any
future proposal that claims to remove wasted Toffoli work, and it is cheap (53 s) and exactly gated.

## Reproducing

```bash
mkdir -p examples && cp tools/census/hotness.rs examples/
cargo build --release --offline --example hotness
./target/release/examples/hotness /tmp/head     # /tmp/head.hot.tsv: opidx kind c2 c1 t cond charge fire head hotness
rm -rf examples
```

Check `GATE ok` and that `avgT` matches `eval_circuit` before using any of it.
