# Can any never-firing gate be certified dead independent of the draw? No, not structurally

> Measured 2026-08-03/04 on head `6909d15` (1,486,468,554). Instrument:
> [`../tools/census/constzero.rs`](../tools/census/constzero.rs), cross-checked against the fire
> counts from [`fire-vs-charge-cross-census.md`](fire-vs-charge-cross-census.md).

## The question

46,134 gates fire on none of the 9,024 official shots, which is 3.35% of the score, against a strict-beat
bar of 0.886 avgT, so **one certified gate is a submission**. But "never fired on this draw" is a
claim about one draw, and deleting a gate re-rolls all 9,024 inputs. The certificate cannot survive
its own use.

The structural version of the question: is a control **provably zero** at that point in the stream,
for every input? A gate whose control is constant-zero never fires on any input at all, so deleting
it is an exact identity, with no sampling, no stream dependence, and no λ cost.

## Method

A three-valued (`Zero` / `One` / `Unknown`) constant-propagation pass over the final op stream,
the one that is actually scored, after CONSTPROP, the fanout pass, the deep strip and the tail
nonce. It starts from a genuinely strong initial state: `eval_circuit` calls `clear_for_shot()`
and then writes only the four input registers, so **642 of the 1,154 qubits are provably Zero at
op 0** and only the 512 input-register qubits are Unknown.

Conditions are tracked as `AllOnes` / `AllZeros` / `Mixed` across the shot lanes, combining the
`PushCondition` stack with each op's own `c_condition`. A write under a `Mixed` condition lands on
some lanes and not others, so its target degrades to `Unknown`, the safe direction, which
is what makes a certificate sound rather than merely plausible.

Certification: `CCX` fires on `cond & q(c1) & q(c2)`, so `Zero` on either control certifies it;
`CCZ` fires on `cond & q(t) & q(c1) & q(c2)`, so `Zero` on the target certifies it too.

## Result: zero gates certified

```
CERTIFIED constant-zero-control gates: 0
gates inside provably-dead condition blocks: 0
never-fire gates in dump: 46134   certified structurally: 0   unexplained: 46134
```

**And the analysis is not empty. The diagnostics are what carries this result:**

| diagnostic | value |
|---|---|
| ops with a provably-`AllOnes` condition | 6,294,228 |
| ops with a `Mixed` condition | 2,756,809 |
| qubit lattice at op 90,573 | Zero=63, One=0, Unknown=1,091 |
| qubit lattice at op 4,528,650 | Zero=320, One=0, Unknown=834 |
| qubit lattice at op 9,057,300 (end) | Zero=598, One=0, Unknown=556 |
| **CCX/CCZ with both controls `Unknown`** | **1,343,361, all of them** |
| **CCX/CCZ with a provably-`One` control** | **0** |

The lattice carries real information: it tracks 6.3 M unconditional ops exactly, and the Zero
population moves 63 → 320 → 598 as ancillas are allocated and uncomputed back to `|0⟩`. It simply
has **no power at the gates**: every one of the 1,343,361 CCX/CCZ has both controls `Unknown`.

That is the expected outcome once you look at it the right way round. `point_add::build()` already
runs a CONSTPROP pass (`dropped=144, folded_cx=23, aff_drop=9` on this head), which harvests
exactly this certificate class. By the time the stream is final, **there is nothing left for a
constant-propagation argument to find**. The class is empty because it has already been emptied.

## What this says about the 46,134, and about the census gap

All 46,134 are unexplained by constant propagation. Their controls are genuine data-dependent
values that happen never to be simultaneously 1. That is a **data invariant of the binary-GCD and
modular-arithmetic engines**, not a dataflow fact. It is a theorem about what values those registers can
hold, not about which wires are constants.

This also explains the census miner's 25.02% / 42.67% over-observation gap from the other side:
the miner is a sampler, so it cannot see an invariant either. It observes firing and reports it.
Neither instrument has access to the thing that actually makes these gates dead. **A structural
certificate would close both, and neither of the two cheap structural arguments, constant
propagation and condition-stack domination, is that certificate.**

All three sub-cases from the original framing are now answered:

| hypothesized reason | verdict |
|---|---|
| control constant-folded to zero | **0 gates**, CONSTPROP already took them |
| control uncomputed to `\|0⟩` at that point | **0 gates**, same analysis covers it; ancilla Zeros are never used as controls while Zero |
| dominated by an earlier condition | **0 gates**, no gate sits in a provably-dead condition block |
| merely absent from this sample | **all 46,134** |

## Nothing was removed

No gate was deleted, and no configuration was produced, so the grind trigger did not fire. Removing
any of the 46,134 on the strength of the 9,024-shot observation would be precisely the
stream-specificity error that `04-traps.md` §1 documents and that
[`HANDOFF-2026-08-03-remine-2.md`](HANDOFF-2026-08-03-remine-2.md) raised against harness-order
mining. **Prove before removing**, and nothing here proved anything.

## Rung 2: affine-relation analysis over GF(2), also zero, and also not empty

Built as [`../tools/census/affine.rs`](../tools/census/affine.rs). Each qubit and bit carries an
affine form `constant XOR (XOR of atoms)`, with atoms XOR-hashed into a `u128` so equality and
complementarity are one compare. `X`, `CX`, `Swap`, `R`/`Hmr` propagate **exactly** under a
provably-`AllOnes` condition. `CCX` is nonlinear, so its target takes a **hash-consed AND term**
keyed on the two control forms, making the representation an XOR-of-AND graph in which identical
subexpressions collapse rather than an XOR of opaque unknowns.

Certificates: either control constant-0, or the controls **complementary** (`c1 = ¬c2`, so the AND
is identically zero); for `CCZ`, the same over any pair among `{t, c1, c2}`.

**Positive control: the analysis can see a pair it is handed directly.** `--positive-control`
runs a synthetic stream where `CX(q0→q1); X(q1)` makes `q1 = ¬q0`, then a `CCX(q1, q0, q2)` and a
`CCZ(q1, q0, q5)` on that complementary pair, plus a `CCX(q3, q0, q4)` on unrelated controls:

```
CERTIFIED never-firing gates : 2
  op 2 kind 13 reason c1=!c2
  op 4 kind 14 reason c1=!c2
POSITIVE CONTROL PASS
```

Both complementary gates certified, the unrelated one correctly not. **On the real circuit:**

```
CERTIFIED never-firing gates        : 0
CCX/CCZ whose controls share a tag  : 0
never-fire in dump: 46134   certified: 0   still unexplained: 46134
```

The propagation diagnostics say why, and they are the substance of the result:

| diagnostic | value |
|---|---|
| atoms minted | 2,996,434 |
| CCX total | 1,338,625 |
| CCX with a constant-1 control (stays affine) | **0** |
| CCX with equal controls (stays affine) | **0** |
| CCX handled as a hash-consed AND | 1,338,625 |
| of those, reused an existing AND term | 6,354 |
| distinct AND terms | 1,226,517 |
| **CCX/CCZ whose controls share a tag** | **0** |

**Not one gate in the circuit has controls that are affinely related at all**: not equal, not
complementary, not even sharing a single atom. Hash-consing recovers almost nothing: 6,354 of
1,338,625 AND terms are reused, so 1,226,517 distinct nonlinear terms are genuinely distinct
subexpressions.

That is the honest shape of this circuit. It computes modular inversion and multiplication, so
essentially every value is a *nonlinear* function of the inputs, and affine structure survives only
through `CX`/`X` chains that no `CCX` interrupts. There are no such chains reaching any gate's
control pair. The complementary-flag intuition, that GCD sign/branch logic would produce
`c1 = ¬c2` pairs, is not borne out: whatever complementary flags exist are consumed by nonlinear
gates before they meet as a control pair, or are never a control pair to begin with.

## The next rung, if this is worth continuing

Two rungs of syntactic analysis are now done and both return zero, with positive controls showing
neither is empty. What they have in common is that they reason about the *form* of a value, and
this circuit's values have no exploitable form: 1.23 M distinct nonlinear terms, no two gate
controls affinely related.

Any further rung has to reason about **what the values can be**, not how they are written. The
data invariants of the binary-GCD engine (register bit ranges, mutual exclusion of branch flags,
the loop invariant relating `u`, `v` and the schedule width). That is a semantic proof obligation,
plausibly discharged by a bounded model checker or an SMT encoding over one divstep, then lifted by
induction. It is a research-scale task, not an overnight one, and it is the same obligation that
would explain the census miner's 25%/43% gap.

**The cheap structural routes are exhausted.** That is a real finding: it means the 46,134 are not
low-hanging, and any claim to delete them has to carry a semantic argument.

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
tool exits non-zero if any certified gate fired. With zero certified it passes trivially, which is
why the non-vacuity diagnostics above, not the check, are what make the negative result meaningful.
