# Feasibility: teaching CONSTPROP a binary-GCD bit-growth invariant

Read-only. No source changed. Question: could CONSTPROP prove more Toffoli controls constant-zero by
injecting a bit-growth invariant (`|u|,|v| < 2^{w(i)}` at iteration `i`) that its local dataflow
can't currently see, and is the prize worth the math?

## Bottom line

The mechanism is sound and CONSTPROP is the right (index-safe, re-verifiable) vehicle. **But the
realistic net-new Toffoli yield is small, plausibly a few hundred to low thousands, quite possibly
near zero, because the two things that would harvest this deadness already run:** (a) the register
width schedule `SCHED_J2[i]` already truncates each GCD adder to `current_n` bits (the bulk of the
bit-growth savings is *baked into the schedule*), and (b) the structural-dead tables already remove
~10,600 more dead carries *inside* `current_n`, including explicit high-bit ranges that are
literally bit-growth statements (e.g. GIDNEY `(2591, 54, 253)` = "bits 54–253 of this adder are
dead"). A bit-growth-enhanced CONSTPROP would only find the residual those two layers *missed*,
bounded by the gap between `SCHED_J2[i]` and the true bound `w(i)`, which the schedule's smooth
~1-bit/iteration decline suggests is small. The **primary value of the idea is robustness, not
score**: a sound bit-growth pass could *replace* the ~10,600 unproven, index-fragile baked table
entries with re-derived, provable removals, same Toffoli count, far less fragility.

## 1. How CONSTPROP works and where its knowledge ends

- **Lattice:** per qubit/bit, a three-valued `Val ∈ {Zero, One, Unknown}` (`constprop.rs:7-14`),
  with transfer functions `xor_val` (`constprop.rs:78-84`), `and_val` (`:86-92`), join `merge`
  (`:94-96`, disagreement → `Unknown`). A separate **affine** layer (`analyze_affine`,
  `constprop.rs:294`) tracks each qubit as an XOR-set of input symbols to catch complementary/equal
  controls.
- **Seed (the soundness anchor):** every qubit and bit starts `Zero`; only the declared circuit
  inputs are set `Unknown` (`constprop.rs:99-106`):
  ```rust
  q: vec![Zero; num_q], b: vec![Zero; num_b], ...
  for &q in input_qubits { a.q[q.0] = Unknown; }
  ```
  So a fresh ancilla is known-zero until something writes it; a qubit derived from inputs becomes
  `Unknown` the moment a data-dependent gate touches it.
- **What it drops:** a CCX is deleted only when a control's `Val` is *exactly* `Zero`
  (`DropZeroCtrl`, `constprop.rs:124-131`); folds when a control is `One` (`:132-156`); otherwise it
  propagates `xor_val(tgt, and_val(c1,c2))` (`:157-164`).
- **The knowledge boundary, why it can't see bit-growth:** a high-limb carry is produced by
  `ccx(u[j], v[j], carry)` (body adder, `gidney.rs:1202`; compare, `comparator.rs:692`). The high
  limbs `u[j]`, `v[j]` were written earlier by data-dependent shifts/subtracts of input-derived
  values, so CONSTPROP holds them at `Unknown`; hence `and_val(Unknown,Unknown) = Unknown`, the
  carry is `Unknown`, and the downstream control is `Unknown`, **not dropped**. CONSTPROP reasons
  about *constant/affine* facts of individual qubits; it has **no notion of the integer magnitude of
  a multi-qubit register**. The fact that `u,v < 2^{w(i)}` (so `u[j]=v[j]=0` for `j ≥ w(i)`) is a
  *range* invariant of the binary-GCD algorithm, entirely outside its model. That gap is exactly
  what the idea proposes to fill by injection.

## 2. Which controls would become provably zero, and how many gates

The GCD body adder runs on the live slice `u[..current_n]`, `v[..current_n]` with
`current_n = SCHED_J2[i]` (`gcd.rs:823, 827-828`; bits `≥ current_n` are already freed at
`gcd.rs:754-761`, so they're *not even processed*). The bit-growth invariant only helps for bit
positions **inside** `[w(i), current_n)`, i.e. only if `SCHED_J2[i]` is *looser* than the true
bound `w(i)`. For each such provably-zero high limb `j`, these controls become provably zero:

| gate | file:line | control that dies when `u[j]=v[j]=0` |
|---|---|---|
| threaded-add forward carry | `gidney.rs:1202/1211` | `ccx(u[j], v[j], co)`, both controls 0 |
| threaded-add sum write | `gidney.rs:1237/1266` | `ccx(ctrl, v[j], u[j])`, depends on high `v[j]` |
| compare top carry | `comparator.rs:692` | `ccx(u[j], v[j], next)` (within `cmp_eff=GAP_J2[i]`) |
| fused fold carry | `fused.rs:1025` | `ccx(y[j], ci, next)` for high fold limbs |

Per iteration, roughly `2·(current_n − w(i))` dead body-adder carries (forward + sum), plus a few
compare/fold high carries. Across all 1,032 iteration-steps (258 × 4 sweeps), the **gross** count
scales with the average margin `current_n − w(i)`:

- margin 1 bit → ~2,060 gross carries; 2 bits → ~4,130; 3 bits → ~6,190; 5 bits → ~10,320.

For scale: the GCD body processes ~141,800 operand-bit-positions per point addition
(`Σ SCHED_J2 = 35,453` per sweep × 4), average `current_n ≈ 137`. So even a 2-bit average margin is
only ~3% of body-adder carries, and that gross figure is *before* subtracting what the tables
already remove.

## 3. Minimal interface CONSTPROP would need

CONSTPROP would need to be **told**, per iteration, a *known-zero width*: "at this point in the op
stream, register R has bits `[w, len)` equal to `Zero`." Minimal form:

- **Fact:** a set of `(QubitId, Val::Zero)` assertions for the high limbs of `u` and `v` at each
  iteration boundary, i.e. clamp `a.q[q] = Zero` for `q ∈ u[w(i)..current_n] ∪ v[w(i)..current_n]`.
- **Keyed to:** the iteration index `i` (or, since CONSTPROP walks a flat op stream with no phase
  markers, an injected marker/boundary op, the harness already has a no-op `DebugPrint` kind the
  evaluator preserves, which `set_phase` boundaries could emit; see `docs/profiling-notes.md`).
- **Form:** a table `w: [usize; ITERS]` (the tight bound) plus the qubit ids of the `u`/`v`
  registers at each boundary, fed into `analyze` so the clamp is applied before the adder's carries
  are evaluated. CONSTPROP then propagates `and_val(Zero, _) = Zero` naturally and drops the carries.
- **Soundness obligation:** `w(i)` must be a *proven* upper bound on the register's bit-length for
  all inputs, a theorem about the jump-2 binary GCD, not a measurement. If `w(i)` is ever too small,
  CONSTPROP would drop a live gate. (The existing `CONSTPROP_VERIFY` empirical re-sim,
  `constprop.rs:844-909`, would catch a wrong clamp on sampled inputs but is not a proof.)

Note this is strictly *more* than the schedule already encodes: `SCHED_J2[i]` is the *register
width* actually allocated; `w(i)` would be the *provable magnitude bound*. The prize is exactly the
gap `SCHED_J2[i] − w(i)`, and only the part of that gap not already removed by the tables.

## 4. Honest yield estimate

**The prize is very likely small.** Three facts bound it:

1. **The width schedule already captures the primary bit-growth truncation.** The adder never
   touches bits `≥ current_n` (`gcd.rs:754-761, 827-828`). So this is not "the adder wastes 256-bit
   work", it already works at `current_n ≈ 137` average. The only residual is the `[w(i),
   current_n)` sliver.

2. **The baked tables already remove ~10,600 structural-dead carries, including bit-growth ones.**
   Counting the tables: ~5,977 carry-bits from the `*_DEAD_RANGES` tables + ~3,801 exact
   `*_REMAINDER_KEYS` + ~800 whole-call skips ≈ **10,600 gates** already suppressed. And they *are*
   bit-growth-shaped: GIDNEY `GIDNEY_THREAD_FWD_DEAD_RANGES` contains `(2591, 54, 253)`, `(5, 0,
   123)`, `(0, 53, 122)`, wide high-bit ranges of single adder calls, exactly "these high limbs are
   provably zero." So the offline analysis that produced the tables *already did a bit-growth
   deadness pass*; a CONSTPROP invariant would re-derive much of the same set.

3. **CONSTPROP runs *after* the tables (`mod.rs:499`), on survivors only.** The ~10,600
   table-suppressed gates are absent from CONSTPROP's input. So a bit-growth CONSTPROP can only find
   gates that (a) survived the tables *and* (b) are bit-growth-dead, i.e. deadness the offline
   analysis **missed**. If the schedule is near-tight and the tables are thorough, that residual is
   ~0.

**Realistic net-new estimate: ~0 to ~2,000 Toffolis**, most likely at the low end (hundreds). A
score-chasing submission almost certainly tuned `SCHED_J2` close to the tight bound and let the
tables mop up the interior, so the uncaught `SCHED_J2 − w(i)` margin is probably 0–1 bits on average
(~0–2,000 gross, minus overlap with the tables). Getting into the multi-thousand range would require
`SCHED_J2` to be loose by 3–5 bits *and* the tables to have systematically missed it, unlikely given
how aggressively both are tuned.

**The decisive cheap experiment** (before writing any CONSTPROP code): derive the provable jump-2
GCD bit-bound `w(i)`, compare it to `SCHED_J2[i]` (data in `docs/schedule-widths.md`), and for the
iterations where `SCHED_J2[i] > w(i)`, check whether the corresponding `(call, bit)` positions are
*already* in the dead tables. The count of `[w(i), current_n)` positions **not** already in a table
is the exact net-new prize. If that count is small (likely), the math isn't worth it for score.

**Where the idea *is* worth it:** as a **robustness** refactor, not a score play. A sound,
per-build, `CONSTPROP_VERIFY`-checked bit-growth pass could *replace* the ~10,600 baked, uncommented,
index-fragile table entries (`docs/dead-gate-analysis.md`) with provable removals, identical
Toffoli count, but immune to the emission-order fragility that currently makes every inner-loop edit
dangerous. That converts an unproven, brittle optimization into a proven, self-maintaining one. If
the motivation is "make the existing savings safe," this is the right project; if it's "find new
Toffolis," the ceiling above says temper expectations and run the cheap experiment first.
