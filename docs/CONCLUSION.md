# Optimization campaign: conclusion

> ## ⚠ Superseded, read this first
>
> **Written 2026-07-11 against commit `422f21d`. Its headline verdict is wrong.**
>
> This document concluded that no lever was available and that only a research-scale rewrite
> could lower the score. In the 21 days that followed, upstream landed **25 accepted
> score-lowering submissions**. This fork was rebased onto `8af8a6f` on 2026-08-02, measuring
> **1,487,590,242** on `801dd20` (1,289,073.125 executed Toffoli × **1154** qubits), and again onto
> `ed4b529` on 2026-08-03: the current head `6909d15` measures **1,486,468,554**
> (1,288,101.386 × 1154, 9,024/9,024 clean), against the 1.52e9 quoted below.
>
> Calling that "tuning I missed" would undersell what actually happened. Upstream is a single
> autonomous agent (`yukon-autoresearch[bot]`) running a Darwin-Gödel-Machine loop: it mutates
> `src/point_add/` in disposable worktrees, screens candidates on a 512/2,048/8,192/9,024 shot
> ladder, and promotes only what a full verifier passes. It ran that loop **while λ was still low
> enough for a clean seed to be cheap**, and spent the resulting throughput climbing the
> aggressiveness curve. `ITERS` moved 258 → 261 on 2026-07-26, the same day 8 submissions landed.
> The pace then fell to 3 on 08-01 as λ rose. That is the shape of the campaign: not a better
> lever, but a search loop run hard against a constraint that was cheap early and is expensive now.
>
> So the real error here is not "I missed a lever." It is that this document was scoring levers
> on Toffoli × qubits while the thing actually governing progress was λ and the cost of a seed
> search, a quantity it never measures or mentions. See
> [`upstream-search-economics.md`](upstream-search-economics.md).
>
> **The original text is kept intact on purpose.** Each superseded claim is marked in place and
> itemized in [Lever verdict audit](#lever-verdict-audit). What I got wrong, and why, is the
> most useful part of this file.
>
> New material from the 2026-08-02 session: [`lambda-measurement.md`](lambda-measurement.md),
> [`upstream-search-economics.md`](upstream-search-economics.md), [`data/`](data/).

A write-up of an effort to lower the score of the reversible secp256k1 point-addition circuit in this
repository (metric: average executed Toffoli count times peak qubit width). The circuit itself is the
community-contributed frontier from the challenge repository; it was reproduced and validated locally.
The material here is the profiling and the analysis of which optimizations are available. The detailed
analyses live alongside this file in `docs/`.

## Verdict

The circuit runs at about 1.32M Toffoli times 1152 qubits (about 1.52e9) for one bare affine point
addition, which is the operation the ecdsa.fail harness scores. That is below both reference Pareto
points listed in the top-level README. The analysis found no available lever that lowers the score:
each lever tried was a dead end, a measured regression, or a correctness break. About 95% of the
Toffoli budget is two modular inversions, which reversible affine point addition requires. In the
literature surveyed in `quantum-inversion-frontier-research.md`, no reversible modular-inversion
implementation has a lower Toffoli count than the windowed binary GCD used here. The remaining option
is a ground-up rewrite of the inversion (a jump-k GCD engine) with an uncertain ceiling.

> **⚠ Superseded (2026-08-02).** Three errors in the paragraph above.
>
> 1. *"about 1.32M Toffoli times 1152 qubits (about 1.52e9)"*: that was `422f21d`. The head is
>    now `801dd20` at **1,289,073.125 × 1154 = 1,487,590,242**, measured locally, 9,024/9,024
>    clean. Peak went **up** by 2 qubits while Toffoli fell 2.40%; upstream bought Toffoli with
>    qubits and came out 2.23% ahead.
> 2. *"found no available lever that lowers the score"*: false. Upstream landed 25
>    score-lowering submissions in the next 21 days. Levers 1, 3 and 11 below were misjudged.
> 3. *"The remaining option is a ground-up rewrite"*: false. None of the 25 was a rewrite.
>
> The paragraph also never mentions λ, the intrinsic error rate. Every lever here was scored on
> Toffoli × qubits alone, when λ is what actually decides whether a change can ship. See
> [`lambda-measurement.md`](lambda-measurement.md).

The published figures cited below are for different operations or scopes than one bare affine point
addition, so a direct score ranking is not like-for-like. Scopes are stated in "Comparison to
published figures".

## Where the cost is

Profiling (`profiling-notes.md`) attributes the about 1.32M executed Toffolis:

| bucket | share | what |
|---|---|---|
| Modular inversion (`tlm_inverse`) | ~47.6% | binary-GCD division `λ = dy/dx` |
| Modular "multiply" (`tlm_forward_multiply`) | ~47.6% | the reversible uncomputation of λ, a second division |
| Modular squaring | ~4.5% | Karatsuba plus symmetric partial products |
| Coordinate add/sub | <0.4% | small |

About 95% is two modular inversions. Reversible affine EC point addition needs two inversions per
addition (compute λ, then reversibly uncompute it). By the time λ is uncomputed the inputs have been
overwritten by the outputs, so λ can only be re-expressed as a division of the outputs. This was
checked against the literature and re-derived from the addition formula.

## Every lever, and its verdict

The `verdict` column is the original 2026-07-11 text, unedited. The `2026-08-02` column is the
re-audit against `src/point_add/memory/` and the rebased head; details in
[Lever verdict audit](#lever-verdict-audit).

| # | lever | verdict | 2026-08-02 | detail |
|---|---|---|---|---|
| 1 | GCD per-iteration width tightening | needs an unproven bit-growth bound | ❌ **refuted** | `schedule-widths.md`, `constprop-bitgrowth-feasibility.md` |
| 2 | apply-swap truncation to live width | unsafe; the swapped register is a full-width accumulator | ✅ stands | `apply-swap-analysis.md` |
| 3 | extend structural-dead-gate skip tables | saturated; about 10.6K already skipped, CONSTPROP reaches a fixpoint at plus 269 | ⚠ superseded | `dead-gate-analysis.md` |
| 4 | teach CONSTPROP a bit-growth invariant | about 0 net; the deadness is already captured by the tables and width schedule | ✅ stands | `constprop-bitgrowth-feasibility.md` |
| 5 | enable disabled F-fold squaring schedule | measured regression: plus 3,458 Toffoli, peak 1153 above 1152, correctness FAILED | ⚠ half-suspect | `squaring-analysis.md` §7 |
| 6 | in-place squaring rewrite (remove the unbuild) | about 6 to 9K ceiling (dual-use terms tie), not pursued | ✅ stands (untested) | `squaring-analysis.md` |
| 7 | replace `tlm_forward_multiply` with a cheap multiply | not available; it is the second inversion, not a naive multiply | ✅ stands | this file, "Where the cost is" |
| 8 | a cheaper inversion algorithm | none found; binary GCD has a lower reversible Toffoli count than Litinski/Qualtran/RNSL, and Bernstein-Yang divstep is rejected for reversible use | ✅ stands | `quantum-inversion-frontier-research.md` |
| 9 | mine the disclosed frontier (Schrottenloher/Qarton) | the disclosed figures are for windowed additions and a full 28-addition attack, not one bare addition (see comparison below) | ✅ stands | `quantum-inversion-frontier-research.md` §4 |
| 10 | windowed multiply / HJN swap-rounds | amortization-dependent; a loss for one addition | ✅ stands | `quantum-inversion-frontier-research.md` §5 |
| 11 | lower the peak-qubit target (Pareto axis) | env knobs lose (1088: plus 11.9% Toffoli) or break correctness (1216: FAIL); peak is fixed to 1152 by the baked schedules | ❌ **refuted in part** | measured this campaign |

Both axes of the score were tested. At peak 1088 emitted Toffoli rose 11.9%; at peak 1216 emitted
Toffoli fell 0.87% and correctness FAILED. The per-step vent schedules are tuned to 1152.

> **⚠ Superseded (2026-08-02).** The two measurements are real, and they did move `TLM_TARGET_Q`,
> confirmed by the +11.9% at 1088 matching the ~2,590 Toffoli/qubit marginal cost in
> `memory/05-qubit-reduction.md` Step 4. The *conclusion* drawn from them is wrong. The last
> sentence is false: the schedules are not tuned to 1152, and the head now runs at peak **1154**.

<a name="lever-verdict-audit"></a>
## Lever verdict audit (2026-08-02)

Seven of eleven verdicts stand. Four moved. Each entry quotes the original claim and states what
refuted it.

### ❌ Lever 1, refuted

> *"needs an unproven bit-growth bound"*

No bound is needed. `memory/05-qubit-reduction.md` Step 5 **measures** the lever: narrowing
`SCHED_J2`'s tail by N=160 entries, with `TLM_TARGET_Q` lowered in lockstep, gives **−0.49%** on
the score product. Narrowing shrinks the GCD registers, so the walk's adders, comparators and
cswaps all get cheaper, so it improves both axes at once. The real gate is λ (9.67 classical at
N=160 vs ~7.25 shipped), not a proof. `GAP_J2` must move with it to preserve
`s = SCHED_J2[i] − cmp_window(i) = −1`; break that coupling and the divstep channel goes from
8.36 to 4,646 mismatches. N=258 does break, because the *early* entries are a genuinely tight
magnitude bound, and the slack is in the tail.

I tested the cap and the schedule separately, concluded neither worked, and never tested them
together. That combination is the whole lever.

### ⚠ Lever 3, superseded but not wrong

> *"saturated; about 10.6K already skipped, CONSTPROP reaches a fixpoint at plus 269"*

Correct for `422f21d`, and *more* saturated now. Measured on `801dd20`,
`apply_deep_strip_identity` removes **12,292/12,543** dead keys and downgrades **3,923/3,923**
CCX to CX/CZ, so every candidate in the table is taken.

Two corrections to how I described it. The `cond & q1 & ~q2` downgrade is a **census-mined
table** (`deep_strip_keys::DOWNGRADE_KEYS`), not a build-time predicate. And the
"1,193 → 2,050 on the re-mine" figure in `memory/03-proven-floors.md` is an older census
generation that upstream has since re-mined well past.

The 251 keys the tripwire discards on the unmodified head are drift residue, not a lever: the
stale count scales with how far the stream has moved from its census (251 here against 6,241 in
`memory/05` Step 6 after a structural change). Re-mining is infrastructure for future work, not a
0.02% win to bank.

### ⚠ Lever 5, half-suspect

> *"measured regression: plus 3,458 Toffoli, peak 1153 above 1152, correctness FAILED"*

The +3,458 Toffoli is a gate count and stands. The **`correctness FAILED` is not trustworthy**:
it was measured before `deep_strip_keys.rs`'s occupancy tripwire existed, so any perturbation to
the stream desynced ordinal-keyed drop tables and silently deleted live Toffolis. `memory/03`
carries the same caveat about its own pre-tripwire results: *"every pre-tripwire 'impossible'
verdict is suspect."* Needs re-running. "peak 1153 above 1152" is also a stale reference frame,
the cap is now 1154.

### ❌ Lever 11, refuted in part

> *"peak is fixed to 1152 by the baked schedules"*

False. Peak tracks `TLM_TARGET_Q` directly. The B0 owner census in `memory/01-architecture.md`
Layer 3 sums to 1152 with **124 qubits at `gidney.rs:1206` marked "BORROWED, fills to the cap"**,
a vent pool, not persistent state. `memory/05` Step 4 measures peaks of 1153/1152/1151 by moving
the cap alone, and the head now runs at **1154** (traced statically through
`build()` → `mod.rs:2033`, then confirmed by measurement).

What *does* survive is "the dial alone loses", independently reproduced by `memory/05` Step 4.
Lowering the cap without narrowing the schedule costs ~2,590 Toffoli/qubit against a ~1,188
break-even. I measured that correctly and then over-generalized it into a structural floor that
does not exist.

The `1216: FAIL` is suspect for the same reason as lever 5: pre-tripwire.

## Comparison to published figures

Published figures for related work, with their scopes. These are different operations or scopes than
one bare affine point addition, so they are not a direct score ranking.

- This circuit: about 1.32M Toffoli, 1152 qubits, for one bare affine point addition (the ecdsa.fail
  metric). *(⚠ 2026-08-02: now 1,289,073 Toffoli × 1154 qubits = 1,487,590,242 on `801dd20`. The
  comparison scopes discussed in this section are unaffected by the update.)*
- Schrottenloher 2026 (arXiv 2606.02235): the disclosed per-windowed-addition figures are 2^21.19
  (about 2.34M) Toffoli at 1192 qubits (space-optimized) and 2^20.83 (about 1.82M) Toffoli at 1446
  qubits (gate-optimized), from Table 1. A windowed addition selects one of 2^w = 2^16 precomputed
  multiples (window w = 16, Section 2) and uses table lookups; the paper states a lookup of 2^w values
  costs 2^w Toffoli (Section 2), and the full-attack formula includes 3 times 2^16 Toffoli of lookup
  per addition (Table 2), so it is a heavier operation than one bare addition. The 2^25.78 Toffoli /
  1462 qubit figure is the full Shor attack on secp256k1 (28 point additions, Table 2), not one
  addition. Its per-addition Toffoli count is about 6.5 to 10% below and its qubit count about 1.5%
  above Babbush et al. (Section 1). Each addition performs two full modular inversions with no
  cross-addition amortization.
- Google Pareto points: the 2,700,000 / 1175 and 2,100,000 / 1425 figures are listed in the challenge
  README (Reference numbers) and attributed there to Google. The Babbush et al. paper (2026, ePrint
  2026/625) publishes full-attack resource estimates (at most 1200 qubits and 90M Toffoli, or 1450
  qubits and 70M Toffoli) and withholds the circuits behind a zero-knowledge proof, so these
  per-addition figures are not read off a disclosed circuit.

Because Schrottenloher's figures are for windowed additions or the full attack, and Babbush's circuits
are not disclosed, none of these is a bare single-addition circuit directly comparable to the 1.32M /
1152 figure. The README's "about 3x lower" target is **~5e8 of score**, meaning Toffoli × qubits.
That is not the ~8.5e8 *trials per clean seed* of [`lambda-6909d15.md`](lambda-6909d15.md), which is
a different quantity at a similar size. It corresponds to no disclosed standalone single-addition
circuit: it sits below the two-inversion cost, and reaching it would require cross-addition
windowing that a single-addition benchmark does not use.

## Approaches considered and why each did not apply

- QFT / phase-gradient arithmetic (carry-free addition via Fourier rotations): the gate set is
  Clifford plus Toffoli only, with no rotation or T gate, so Fourier-basis arithmetic is not
  expressible.
- Clifford-is-free exploitation (only CCX/CCZ are counted): the measurement-vent trick already
  converts every ventable AND into Clifford gates, which is why this inversion is about 629K (measured;
  see `profiling-notes.md`) rather than the roughly 1.7M of the surveyed Kaliski implementations
  (Litinski, Qualtran; see `quantum-inversion-frontier-research.md`). It is already applied.
- Cheap-multiply uncompute of the second inversion: does not work because the inputs are overwritten
  by the outputs, so λ can only be re-derived as a division of the outputs.
- Inversion-free projective/Jacobian coordinates: cost more Toffoli than two affine inversions, and do
  not give the unique point representation Shor's algorithm requires.
- Fermat / addition-chain inversion: x^(p-2) is about 256 squarings, about 16M Toffoli, roughly 25
  times more.

## The remaining option (research-scale)

Jump-4 or adaptive-jump binary GCD (from k-ary GCD and Bernstein-Yang batched divstep). Larger jumps
cut the iteration count from 258 toward about 130, which reduces the per-iteration overhead (compare,
tape, control; about 10 to 13% of the two GCDs). The core add-work is roughly conserved, so the
estimated ceiling is about 5 to 10% and uncertain. It is a ground-up rewrite (`JUMP=2` is asserted and
the codec and schedules are jump-2-specific), with no env knob and a build-plus-eval loop of a few
minutes. Everything cheaper than this was tried and did not lower the score.

> **⚠ Superseded (2026-08-02).** The last sentence is false. Everything cheaper was *not* tried:
> lever 1's schedule-narrowing combined with a matching cap reduction was never tested, and
> `memory/05-qubit-reduction.md` Step 5 measures it at −0.49%. `ITERS` is also no longer 258. It
> is 261 on the current head, which changes the iteration-count arithmetic in this paragraph.
>
> Jump-k may still be a real lever; the error is calling it *the remaining* one.

## Bottom line

The circuit sits at the two-inversion cost of reversible affine point addition, in a gate model
(Clifford plus Toffoli, no rotations) that does not admit the Fourier-arithmetic shortcut. In the
literature surveyed in `quantum-inversion-frontier-research.md`, no reversible modular-inversion
implementation has a lower Toffoli count than the binary GCD used here. Lowering the score further
would require a research-scale change, most plausibly a jump-k GCD engine, rather than tuning of the
current circuit.

> **⚠ Superseded (2026-08-02).** The last sentence is the central error of this document.
> Twenty-five score-lowering submissions landed in the following three weeks, none of them a
> jump-k rewrite, all of them tuning. What I mistook for a structural floor was the limit of
> what I had measured.
>
> The thing that actually limits progress is not Toffoli count. It is **λ**, the intrinsic error
> rate, which this document never considers. Measured at **λ_total = 20.04** on `801dd20` (P(clean seed) ≈ 2.0e-9)
> and **20.560** on the current head `6909d15` (≈ 1.2e-9), statistically the same figure on both.
> Any change that lowers the score re-rolls the Fiat-Shamir seed, so it
> cannot ship until a clean nonce is found. That, not the inversion algorithm, is what gates
> progress. See [`lambda-6909d15.md`](lambda-6909d15.md) for the current figure,
> [`lambda-measurement.md`](lambda-measurement.md) for the method, and
> [`upstream-search-economics.md`](upstream-search-economics.md).

---

*Detailed analyses: `profiling-notes.md`, `gcd-engine-study.md`, `schedule-widths.md`,
`apply-swap-analysis.md`, `dead-gate-analysis.md`, `constprop-bitgrowth-feasibility.md`,
`squaring-analysis.md`, `quantum-inversion-frontier-research.md`.*
