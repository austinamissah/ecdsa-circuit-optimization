# Syntactic certification is exhausted: three closed approaches, three passing controls

> Measured 2026-08-03/04 on head `6909d15` (score **1,486,468,554**, avgT 1,288,101.386 × 1154,
> 0/0/0, md5 `ef30945f3afcb369192ea32897232d2f`). Consolidates
> [`gate-hotness-census.md`](gate-hotness-census.md),
> [`fire-vs-charge-cross-census.md`](fire-vs-charge-cross-census.md),
> [`stream-agnostic-certification.md`](stream-agnostic-certification.md) and
> [`census-miner-validation.md`](census-miner-validation.md) into one statement, because the three
> results are the same result.

**Terms used here.** *Certify* means prove, not assume. *Syntactic* means an argument from the form
a value is written in. *Semantic* means an argument from what the value actually means. The whole
finding is that the syntactic routes are closed and only a semantic one is left.

## The target

46,134 gates, which is 3.35% of the score, fire on **none** of the 9,024 official shots. The
strict-beat bar is **0.886 avgT** (`round(avgT) ≤ 1,288,100` wins by 1,154 points), so *one*
certified gate is a submission. The obstacle is that the 9,024 test inputs are a SHAKE256 hash of
the whole op stream (`eval_circuit.rs:204`): deleting a gate re-rolls every shot, so "never fired on
this draw" is a certificate that cannot survive its own use.

Three ways to get a certificate that does survive have now been tried. All three fail, and they
fail for one reason.

## Why a negative result needs a control

A search that finds nothing looks exactly like a search that cannot see. Every negative below is
therefore paired with a demonstration that the instrument detects the thing it reports absent, and
two of those controls caught real defects in this work before the negatives were believed.

---

## 1. Cooling: make gates conditionally cold

**Approach.** The scorer bills a Toffoli by `popcount` of its condition stack and ignores the
controls (`src/sim.rs:77-86`). So a gate charged on fewer shots costs less, whether or not it ever
fires. Tighten condition stacks; harvest the difference.

**Control, a known-answer test.** [`../tools/census/hotness.rs`](../tools/census/hotness.rs)
replays the official measurement and attributes every charge to the op that incurred it. It asserts
`sum(charge) == sim.stats.toffoli_gates` and prints avgT for comparison against `eval_circuit`:

```
GATE ok: attributed=11623826906 == toffoli_gates=11623826906
avgT=1288101.386        (eval_circuit reports 1288101.386)
```

**This control fired.** A first run drew 9,216 shots instead of 9,024 and produced
`avgT=1288114.585`, which looks plausible and is wrong. The gate caught it.

**Result.** The ceiling on cooling, granting a perfect oracle condition to all 1,343,361 gates at
once, is `charge − fire` summed over the stream: **8,914,930,315 shot-charges = 987,913 avgT =
76.70% of the score.** Very large, and unreachable.

The fire-rate mode is **0.25**, exactly the rate at which a Toffoli with two independent uniform
controls fires, with 60.1% of unconditional gates in [0.20, 0.30). Those gates are not wasteful:
three quarters of a Toffoli's charge is the price of the shots where its controls are not both 1.

And that is the part no condition can remove. `PushCondition` takes a **classical** bit; firing is a
function of the **quantum** controls; the classical bits in this circuit are `Hmr` measurement
outcomes, independent of them. That is why the 8.2% of gates that *are* conditioned sit at hotness
0.4999, a fair coin. The probability that a given gate's ~2,256 firing shots all fall inside a given
fair coin's 4,512 true shots is **2⁻²²⁵⁶ ≈ 10⁻⁶⁷⁹**. Manufacturing the bit instead means
*measuring* the controls, an `Hmr`, which destroys the qubit, to save at most 0.75 of one Toffoli.

**Closed: the candidate class is empty for a structural reason, not a failed search.**

---

## 2. Census sampling: certify dead by observation

**Approach.** Sample many random on-curve inputs; a gate whose effect mask never fires is dead.
This is what [`../tools/census/census.rs`](../tools/census/census.rs) and the shipped
`deep_strip_keys.rs` do.

**Control, a known-answer test.** Replaying the occupancy tripwire against a per-gate dump
reproduces `build_circuit` exactly: **dead 12,292 accepted / 251 stale, downgrade 3,923 / 0**,
matching the build's own log line. So the *keying* is provably correct.

**This control also fired**, in the useful direction: it isolated the failure to the certification
predicates rather than the addressing. Of the shipped keys the tripwire accepts, applied in a
circuit that passes 9,024/9,024, our census claims **3,076 dead keys fire (25.02%)** and
**1,674 downgrades are violated (42.67%)**. The shipped table demonstrably yields 0/0/0, so the
census over-observes.

**Result, and the mechanism.** A sampler observes *firing*; it has no access to *why* a gate does
not fire. If a gate is quiet because of a data invariant, meaning a fact that is always true about
what the registers can hold, the census cannot represent that, and can only report the observed
rate at whatever depth it ran. Two censuses at different depths, or over different input
distributions, then disagree about exactly the rarest-firing gates, which is precisely the
population in dispute.

**This identifies the mechanism behind the 25%/43% gap**, an open question carried across
[`census-miner-validation.md`](census-miner-validation.md),
[`HANDOFF-2026-08-03-remine-2.md`](HANDOFF-2026-08-03-remine-2.md) and
[`census-stream-provenance.md`](census-stream-provenance.md), which had already ruled out the
stream difference as the cause. The gap is not a bug in our miner and not a defect in the shipped
table: **it is the signature of certifying by observation something that is true by invariant.**

> **Scope note.** This resolves *our* open question, not upstream's. `06-research-status.md`'s
> open-problems section lists unrestricted exact-eight joint synthesis and the controlled-addition
> factor-two gap; it does not list a census over-observation gap, and upstream's own table works
> for them. The overlap is that their `Deep-strip localization` row concludes "transfer failures
> originate upstream, not in the final deep-strip table", which is consistent with this, but a
> different claim.

---

## 3. Affine relations over GF(2): certify dead by form

**Approach.** A `CCX` never fires if `q(c1) & q(c2) = 0` identically. Two syntactic cases decide it
without knowing values: a control is the constant 0, or the controls are **complementary**
(`c1 = ¬c2`, so one is always the opposite of the other). Complementary flag pairs are what
binary-GCD sign/branch logic ought to produce.

Two rungs were built. [`../tools/census/constzero.rs`](../tools/census/constzero.rs) is a
three-valued constant lattice; [`../tools/census/affine.rs`](../tools/census/affine.rs) carries a
full affine form per qubit (`constant XOR (XOR of atoms)`, atoms XOR-hashed into a `u128`), with
`X`/`CX`/`Swap`/`R`/`Hmr` propagating **exactly** under provably-`AllOnes` conditions and `CCX`
targets taking a **hash-consed AND term** keyed on the control forms. That is an XOR-of-AND graph in
which identical subexpressions collapse rather than becoming opaque unknowns.

**Control, a planted signal.** `affine.rs --positive-control` builds a synthetic stream where
`CX(q0→q1); X(q1)` makes `q1 = ¬q0`, then places a `CCX(q1,q0,q2)` and a `CCZ(q1,q0,q5)` on that
complementary pair, plus a `CCX(q3,q0,q4)` on unrelated controls:

```
CERTIFIED never-firing gates : 2
  op 2 kind 13 reason c1=!c2
  op 4 kind 14 reason c1=!c2
POSITIVE CONTROL PASS
```

Both planted pairs detected; the unrelated gate correctly not certified. `constzero.rs` carries the
matching non-vacuity evidence: 6,294,228 ops tracked under provably-`AllOnes` conditions, and a
Zero population moving 63 → 320 → 598 across the stream as ancillas are allocated and uncomputed.

**Result: zero gates certified, by either rung.**

| diagnostic | value |
|---|---|
| CCX total | 1,338,625 |
| CCX with a constant-1 control | **0** |
| CCX with equal controls | **0** |
| distinct AND terms | 1,226,517 |
| of those, reused via hash-consing | 6,354 |
| **CCX/CCZ whose controls share a single atom** | **0** |
| **certified** | **0** |

Not one gate in the circuit has controls that are affinely related at all: not equal, not
complementary, not even sharing one atom. Hash-consing recovers 6,354 of 1,338,625, so those
1,226,517 nonlinear terms are genuinely distinct subexpressions.

The constant rung's zero has its own clean explanation: `build()` already runs a CONSTPROP pass
(`dropped=144, folded_cx=23, aff_drop=9`), so that certificate class was emptied before the stream
was final.

**Closed: the complementary-flag intuition is not borne out.**

---

## The unifying finding

All three approaches reason about the **form** of a value:

| approach | what it inspects | why it fails here |
|---|---|---|
| cooling | which classical bit gates the charge | firing is quantum; classical bits are independent coins |
| census | the observed fire rate at depth N | an invariant is not visible in a sample at any depth |
| affine | the algebraic expression for each wire | 1.23 M distinct nonlinear terms, no two controls related |

**This circuit computes modular inversion and modular multiplication.** Essentially every value is a
nonlinear function of the inputs, so there is no exploitable form to inspect: affine structure
survives only through `CX`/`X` chains that no `CCX` interrupts, and no such chain reaches any gate's
control pair. The 46,134 gates are quiet because of **what their controls can be**, not because of
how those controls are written or how often they were watched.

That single sentence covers three separate negative results, and it is the finding that matters:
**the cheap certification routes are not merely unproductive, they are the wrong kind of argument.**
The 46,134 are not low-hanging.

## What would actually certify one

A **semantic** argument over the data invariants of the binary-GCD engine, covering register bit
ranges, mutual exclusion of branch flags, and the loop invariant relating `u`, `v` and the schedule
width. The workable shape:

1. encode **one divstep** as a transition relation (the engine is a loop, so one step is small);
2. discharge the candidate invariant over that step with a bounded model checker or SMT solver;
3. lift by induction over the 261 divsteps;
4. a gate whose control pair is excluded by the invariant is then certified for **all** inputs, so
   it is stream-agnostic, λ-free, and safe to delete.

This is also the only route that would close the census gap from the other side, since the same
invariant is what the sampler cannot see. It is a research project rather than an overnight job,
and it is now the only identified route from the 46,134 to a submission.

## Standing

Nothing was removed. No configuration below 1,486,468,554 was produced, so the grind trigger did not
fire. Deleting any of the 46,134 on 9,024-shot evidence would be exactly the stream-specificity
error `04-traps.md` §1 documents. The rule is **prove before removing**, and none of the three
approaches proved anything.

## Reproducing

```bash
mkdir -p examples
cp tools/census/hotness.rs tools/census/constzero.rs tools/census/affine.rs examples/
cargo build --release --offline --example hotness --example constzero --example affine
./target/release/examples/affine --positive-control          # must print POSITIVE CONTROL PASS
./target/release/examples/hotness /tmp/head                  # must print GATE ok + matching avgT
./target/release/examples/constzero --check /tmp/head.hot.tsv
./target/release/examples/affine    --check /tmp/head.hot.tsv
rm -rf examples
```

Run the controls first. Every `--check` requires that no certified gate fired on any of the 9,024
shots and exits non-zero otherwise.
