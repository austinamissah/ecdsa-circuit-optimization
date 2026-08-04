# Can any never-firing gate be certified dead independent of the draw? — No, not structurally

> Measured 2026-08-03/04 on head `6909d15` (1,486,468,554). Instrument:
> [`../tools/census/constzero.rs`](../tools/census/constzero.rs), cross-checked against the fire
> counts from [`fire-vs-charge-cross-census.md`](fire-vs-charge-cross-census.md).

## The question

46,134 gates fire on none of the 9,024 official shots — 3.35% of the score, against a strict-beat
bar of 0.886 avgT, so **one certified gate is a submission**. But "never fired on this draw" is a
claim about one draw, and deleting a gate re-rolls all 9,024 inputs. The certificate cannot survive
its own use.

The structural version of the question: is a control **provably zero** at that point in the stream,
for every input? A gate whose control is constant-zero never fires on any input at all, so deleting
it is an exact identity — no sampling, no stream dependence, no λ cost.

## Method

A three-valued (`Zero` / `One` / `Unknown`) constant-propagation pass over the final op stream —
the one that is actually scored, after CONSTPROP, the fanout pass, the deep strip and the tail
nonce. It starts from a genuinely strong initial state: `eval_circuit` calls `clear_for_shot()`
and then writes only the four input registers, so **642 of the 1,154 qubits are provably Zero at
op 0** and only the 512 input-register qubits are Unknown.

Conditions are tracked as `AllOnes` / `AllZeros` / `Mixed` across the shot lanes, combining the
`PushCondition` stack with each op's own `c_condition`. A write under a `Mixed` condition lands on
some lanes and not others, so its target degrades to `Unknown` — the conservative direction, which
is what makes a certificate sound rather than merely plausible.

Certification: `CCX` fires on `cond & q(c1) & q(c2)`, so `Zero` on either control certifies it;
`CCZ` fires on `cond & q(t) & q(c1) & q(c2)`, so `Zero` on the target certifies it too.

## Result: zero gates certified

```
CERTIFIED constant-zero-control gates: 0
gates inside provably-dead condition blocks: 0
never-fire gates in dump: 46134   certified structurally: 0   unexplained: 46134
```

**And the analysis is not vacuous — the diagnostics are the load-bearing part of this result:**

| diagnostic | value |
|---|---|
| ops with a provably-`AllOnes` condition | 6,294,228 |
| ops with a `Mixed` condition | 2,756,809 |
| qubit lattice at op 90,573 | Zero=63, One=0, Unknown=1,091 |
| qubit lattice at op 4,528,650 | Zero=320, One=0, Unknown=834 |
| qubit lattice at op 9,057,300 (end) | Zero=598, One=0, Unknown=556 |
| **CCX/CCZ with both controls `Unknown`** | **1,343,361 — all of them** |
| **CCX/CCZ with a provably-`One` control** | **0** |

The lattice carries real information: it tracks 6.3 M unconditional ops exactly, and the Zero
population moves 63 → 320 → 598 as ancillas are allocated and uncomputed back to `|0⟩`. It simply
has **no power at the gates**: every one of the 1,343,361 CCX/CCZ has both controls `Unknown`.

That is the expected outcome once you look at it the right way round. `point_add::build()` already
runs a CONSTPROP pass (`dropped=144, folded_cx=23, aff_drop=9` on this head), which harvests
exactly this certificate class. By the time the stream is final, **there is nothing left for a
constant-propagation argument to find** — the class is empty because it has already been emptied.

## What this says about the 46,134, and about the census gap

All 46,134 are unexplained by constant propagation. Their controls are genuine data-dependent
values that happen never to be simultaneously 1. That is a **data invariant of the binary-GCD and
modular-arithmetic engines**, not a dataflow fact — a theorem about what values those registers can
hold, not about which wires are constants.

This also explains the census miner's 25.02% / 42.67% over-observation gap from the other side:
the miner is a sampler, so it cannot see an invariant either. It observes firing and reports it.
Neither instrument has access to the thing that actually makes these gates dead. **A structural
certificate would close both, and neither of the two cheap structural arguments — constant
propagation and condition-stack domination — is that certificate.**

All three sub-cases from the original framing are now answered:

| hypothesised reason | verdict |
|---|---|
| control constant-folded to zero | **0 gates** — CONSTPROP already took them |
| control uncomputed to `\|0⟩` at that point | **0 gates** — same analysis covers it; ancilla Zeros are never used as controls while Zero |
| dominated by an earlier condition | **0 gates** — no gate sits in a provably-dead condition block |
| merely absent from this sample | **all 46,134** |

## Nothing was removed

No gate was deleted, and no configuration was produced, so the grind trigger did not fire. Removing
any of the 46,134 on the strength of the 9,024-shot observation would be precisely the
stream-specificity error that `04-traps.md` §1 documents and that
[`HANDOFF-2026-08-03-remine-2.md`](HANDOFF-2026-08-03-remine-2.md) raised against harness-order
mining. **Prove before removing** — and nothing here proved anything.

## The next rung, if this is worth continuing

The analysis is a *constant* analysis. The obvious strengthening is an **affine-relation analysis
over GF(2)**: track not just `q = 0` and `q = 1` but relations between qubits — `q_a = q_b`,
`q_a = ¬q_b`, and XOR combinations. A `CCX(c1, c2, t)` whose controls are provably **complementary**
(`c1 = ¬c2`) never fires, and complementary flag pairs are exactly the shape the GCD sign/branch
logic produces. That certificate is still structural, still stream-agnostic, and still cheap to
check — and CX/X chains propagate affine relations exactly, so the analysis is a natural fit for
this circuit's op mix.

It is a bigger build than a constant lattice (union-find plus an XOR basis per qubit) and was not
attempted here. It is the single most promising remaining route to a *provable* deletion, and it is
the one thing that would turn the 46,134 from an observation into a submission.

## Reproducing

```bash
mkdir -p examples
cp tools/census/hotness.rs tools/census/constzero.rs examples/
cargo build --release --offline --example hotness --example constzero
./target/release/examples/hotness /tmp/head
./target/release/examples/constzero --check /tmp/head.hot.tsv --out /tmp/certified.tsv
rm -rf examples
```

`--check` is the gate: every certified gate must have `fire == 0` in the measured dump, and the
tool exits non-zero if any certified gate fired. With zero certified it passes trivially — which is
why the non-vacuity diagnostics above, not the check, are what make the negative result meaningful.
