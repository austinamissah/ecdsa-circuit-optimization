# The per-gate hotness census, and why "make gates cold" has no purchase here

> Measured 2026-08-03 on the rebased head (`upstream/main` `6909d15`, score
> **1,486,468,554**). Instrument: [`../tools/census/hotness.rs`](../tools/census/hotness.rs).
> Read alongside [`rebase-2026-08-03-upstream-ed4b529.md`](rebase-2026-08-03-upstream-ed4b529.md).

**Hotness** is the fraction of the 9,024 shots on which a gate is *charged*. Note that charged and
*fired* are different things, and the difference is the whole subject of this document.

## The mechanism is real, and it is not what the deep strip addresses

The scorer charges a Toffoli by the **popcount of its condition mask**, and by nothing else. From
`src/sim.rs:77-86`, word for word:

```rust
let mut cond = current_base_condition;                       // the PushCondition stack
if op.c_condition != NO_BIT { cond &= self.bit(op.c_condition); }
let executed_shots = cond.count_ones() as u64;
match op.kind {
    OperationType::CCZ | OperationType::CCX => { self.stats.toffoli_gates += executed_shots; }
```

The charge does **not** look at the control qubits. `CEILING.md`'s claim, "only CCX and CCZ are
charged, and only on shots satisfying their classical condition stack", is exactly right, and it
splits the cost surface into two independent levers:

- **delete** a gate, which needs a proof it never *fires* (effect mask always zero). That is what
  [`../tools/census/census.rs`](../tools/census/census.rs) certifies. Expensive, and fragile under
  stream edits.
- **cool** a gate, meaning tighten its condition stack so it is charged on fewer shots. Saves
  `(h − h′)/9024` of avgT **whether or not the gate ever fires**.

A gate that never fires but is unconditional is charged in full. A gate that fires on every shot but
is conditioned at hotness 0.5 costs half. These are different quantities and we had only ever
measured the first.

## The instrument, and its gate

`hotness.rs` replays the official measurement, meaning `point_add::build()`, `analyze_ops`, the
Fiat-Shamir XOF over the whole op stream, the same 9,024-shot draw, and the same 64-shot batching,
with `src/sim.rs::apply_iter` copied dispatch-for-dispatch and **one line added** to attribute
`executed_shots` to the op that incurred it.

**It reconstructs the scorer's own total exactly:**

```
GATE ok: attributed=11623826906 == toffoli_gates=11623826906
avgT=1288101.386  (eval_circuit reports 1288101.386)
```

That equality is the whole license for what follows. An earlier run drew 9,216 shots instead of
9,024 and produced `avgT=1288114.585`, which is visibly close and wrong. The gate caught it;
eyeballing would not have.

## The distribution

1,343,361 CCX/CCZ in the scored stream, 9,024 shots, total charge 11,623,826,906.

| band | gates | share of gates | charge | share of charge |
|---|---:|---:|---:|---:|
| **cold** (hotness 0) | **0** | **0.000%** | 0 | 0.000% |
| partial | 110,490 | 8.225% | 498,399,002 | 4.288% |
| **full** (hotness 1.0) | 1,232,871 | 91.775% | 11,125,427,904 | **95.712%** |

By decile, where the empty rows are the result:

| hotness bucket | gates | share of gates | share of charge |
|---|---:|---:|---:|
| 0.0 to 0.4 | 0 | 0.0000% | 0.0000% |
| 0.4 to 0.5 | 55,637 | 4.1416% | 2.1407% |
| 0.5 to 0.6 | 54,853 | 4.0833% | 2.1470% |
| 0.6 to 1.0 | 0 | 0.0000% | 0.0000% |
| exactly 1.0 | 1,232,871 | 91.7751% | 95.7123% |

**The distribution has two spikes, at 1.0 and 0.5, with nothing anywhere else.** The partial band
spans `[0.4805, 0.5208]`, 287 distinct charge values, mean hotness **0.4999**.

### Three findings, in order of how much they constrain the lever

**1. There are no cold gates. Not one.** Every CCX/CCZ in the stream is charged on at least 4,336 of
9,024 shots. "Cold gates are free" is true of the scorer and empty on this circuit: there is no
existing free lunch to harvest, and no dead weight that is already costless.

**2. The partial band is a fair coin, and cannot be tightened.** Every CCX/CCZ in the stream has
`c_condition = NO_BIT`, so there is exactly one distinct condition-bit value across all 1.34 M
gates. All conditioning therefore comes from the enclosing `PushCondition` stack, and the bits those
blocks push are measurement outcomes (`Hmr`). A measurement outcome is an unbiased coin, which is
exactly why the band sits at 0.4999 and why its spread (±0.02) is sampling noise on 9,024 draws.
**There is nothing to tighten**: the condition is already the sharpest test available, and a fair
coin cannot be sharpened without changing what the circuit computes.

**3. The charge is spread as evenly as it can be, so there is no hot spot to attack.**

| | share of gates | share of charge |
|---|---:|---:|
| top 100 gates | 0.007% | 0.008% |
| top 1,000 | 0.074% | 0.078% |
| top 10,000 | 0.744% | 0.776% |
| top 100,000 | 7.444% | 7.763% |

A perfectly uniform circuit would put 0.0074% of charge in its top 100 gates. This one puts 0.008%.
Grouped by operand tuple (187,387 distinct), the single largest contributor is `CCX(1028, 772, 4)`
at **0.0401%** of total charge. **Ranking gates by charged cost produces a flat list.** The
requested "top contributors" do not exist as a category. The cost is spread almost perfectly evenly
over 1.34 M gates, which is what you would expect of a circuit that has already had 144 constprop
drops, a fanout pass, and a 17,278-key deep strip run over it.

## What this means for the lever

The framing, *we've been trying to DELETE gates when we should be making them CONDITIONALLY COLD*,
identifies a real mechanism and describes it correctly. The census says it has no purchase on this
circuit as stated, for a reason that is structural rather than incidental:

- Cooling an **existing** conditional gate is impossible: 4.29% of charge sits behind fair coins.
- Cooling an **unconditional** gate is not "tightening a condition stack", because those gates have
  no condition stack. It requires *introducing* one: computing a bit `b`, emitting
  `PushCondition(b)`, and proving the gate's effect is never needed when `b` is false. That is
  synthesis, not a postpass. It pays Clifford ops and classical bits for the bit, and it re-rolls
  the Fiat-Shamir stream and therefore λ.

There is a real prize sitting in plain sight, though, and the census is what makes it a number
rather than a guess. **The 8.2% of gates that are already conditioned save 4.29% of the total charge
compared to running them unconditionally.** Conditioning is worth roughly `0.5 × h` of a gate's
cost, and 95.7% of the charge has not been conditioned at all. The question the census
*cannot* answer, and the next instrument to build, is which of those 1.23 M unconditional gates have
an effect that is only needed on a subset of shots you can determine in advance.

That is a **fire-versus-charge cross-census**: for each unconditional gate, the set of shots on
which its effect mask is non-zero. `census.rs` already computes the effect mask per gate. It
currently reduces it to a single "ever fired" bit, and would need to keep the per-shot pattern and
test it against candidate condition bits. A gate that fires on about half the shots, in a pattern
that matches an available bit, is a genuine cooling candidate worth `0.5` avgT. **No such candidate
has been identified yet, and this document does not claim any exist.**

## Reproducing

```bash
mkdir -p examples && cp tools/census/hotness.rs examples/
cargo build --release --offline --example hotness
./target/release/examples/hotness out          # writes out.hot.tsv, one row per gate
./target/release/examples/hotness -            # summary only
```

It honors `SUB4_APPLY_STRIP` / `TLM_SCHED_J2_DELTA`, so it belongs under the same environment as
the build being priced. **The `GATE ok` line and the `avgT` match against `eval_circuit` are what
license everything else in the dump.** The instrument is only meaningful when it reconstructs the
scorer's total.
