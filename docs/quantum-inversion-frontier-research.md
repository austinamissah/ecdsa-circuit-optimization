# How low can a single reversible secp256k1 point addition go?, a literature-grounded frontier analysis

A research synthesis on the cost floor of a reversible elliptic-curve point addition scored by
**Toffoli count × peak qubit width** (the ecdsa.fail metric), and on which algorithmic levers can and
cannot move it. Compiled from a multi-source, adversarially-verified survey of the quantum-ECC
literature (2017–2026). Sources are listed at the end; every quantitative claim is cited.

## TL;DR

- **In the surveyed literature, no reversible 256-bit modular-inversion implementation has a lower
  Toffoli count than a windowed binary GCD.** Every published reversible 256-bit inverter is the same
  binary-GCD / Kaliski family, and the best-documented ones cost more Toffoli than the windowed binary
  GCD used here, not less. Bernstein-Yang safegcd/divstep, a faster classical modular inversion, is
  rejected for reversible use (see §2).
- **A single affine point addition pays for two full modular inversions**, and this is irreducible:
  projective/Jacobian coordinates lose in Shor's setting, and Montgomery batch-inversion needs a
  batch (a single addition is a batch of one). So the Toffoli cost of one point addition is
  **floored at ≈ 2 × (one inversion)**, which is where ~95% of the budget sits.
- **Consequence, the single-addition score has a hard floor.** With a state-of-the-art ~629K-Toffoli
  inversion, one point addition costs ≈ 1.3M Toffoli, i.e. **≈ 1.5×10⁹ at ~1150 qubits.** A design
  already at that point is at the *standalone single-addition frontier*.
- **No disclosed academic circuit reports a bare single-addition figure below the two-inversion cost.**
  Mining the disclosed Schrottenloher 2026 circuit (Qarton source public): its full secp256k1 attack
  is 28 windowed point additions, each about 2^21.19 (about 2.34M) Toffoli at 1192 qubits
  (space-optimized) or about 1.82M at 1446 qubits (gate-optimized), and each addition performs two
  full modular inversions with no cross-addition amortization. A windowed addition selects one of 2^16
  precomputed multiples and includes a lookup table, so it is a heavier operation than one bare
  addition; these are not bare single-addition scores. (An earlier back-of-envelope "about 140 to 180K
  per addition" was an artifact of dividing the 70 to 90M full-attack total by a mis-estimated 350 to
  512 additions; the real divisor is 28, giving about 2.0 to 2.6M per windowed addition.) The README's
  "about 3x lower" target (about 5×10⁸) corresponds to no disclosed standalone single-addition circuit;
  it is below the two-inversion cost and would require cross-addition windowing a single-addition
  benchmark does not use.

## 1. The metric and the two-inversion floor

The score is `avg_executed_Toffoli × peak_qubits`. Affine EC point addition computes
`λ = (y₂−y₁)/(x₂−x₁)`, then the new coordinates, then must **reversibly uncompute λ**. Because the
division is done by inverting into an auxiliary register, multiplying, and then **inverting again** to
clean the auxiliary register (Bennett/pebbling), one addition contains **two full modular inversions**
plus a squaring, two multiplications, and a handful of additions [HJN+20]. The two inversions
dominate, typically ~95% of the Toffoli budget.

So, for any inversion of cost `I` Toffoli, one point addition is floored near `2·I` Toffoli. With the
best documented reversible inversion (~629K, see §2) that is ≈ **1.26M Toffoli of inversion + ~60K
squaring + multiplies ≈ 1.3M total**, i.e. **≈ 1.5×10⁹ at ~1150 qubits.** This is not a tuning
artifact; it is structural. Beating it *within the single-addition metric* requires one of: (a) an
inversion materially cheaper than any known, (b) doing the addition with fewer than two full
inversions, or (c) the same Toffoli count at drastically lower peak width, and §2–§4 show all three
are ruled out by the current literature.

## 2. Inversion-algorithm comparison, binary GCD already wins

Every disclosed reversible 256-bit modular inverter is a **round-based Kaliski / binary-extended-
Euclidean (Montgomery-inverse)** circuit run for a fixed `2n = 512` rounds, the same family as a
windowed binary GCD. Concrete costs:

| inverter (256-bit, reversible) | Toffoli count | qubit / notes | source |
|---|---|---|---|
| binary GCD division phase here (inversion + a multiply) | ~666K emitted / ~629K executed | measured for this circuit | `profiling-notes.md` |
| Litinski 2023 (Kaliski), inversion only | 26n²+2n = **1,704,448** | "over 10 times more expensive than a multiplication"; a multiply is 2.25n²+9n | [Lit23] App. C5 (verified) |
| Qualtran implementation, inversion only | 26n²+9n−1 ≈ **1.71M** | reported; not re-verified this pass | [Qualtran] |
| Häner–Jaques–Naehrig–Roetteler–Soeken (RNSL) | ~2n-round Kaliski | ≈2.35M/add space-opt, 1192 q; reported | [HJN+20] |
| Luo et al. 2026 (space-efficient EEA) | 204n²log₂n ≈ **1.07×10⁸** | ~800 q, trades Toffoli for width; reported | [Luo26] |
| Bernstein–Yang divstep / safegcd | n/a | **rejected for reversible** (see below) | [BY19],[HJN+20] |

Accounting caveat: these figures come from different papers with different circuit decompositions and
gate-count conventions, so this is an order-of-magnitude comparison, not a controlled benchmark. The
in-circuit number is the measured Toffoli of the `tlm_inverse` phase, which computes the division
`dy/dx` (an inversion plus a multiply); the literature rows are inversion only. Even so, the one
division here (~666K emitted) is below Litinski's inversion-only figure (1,704,448), and a Litinski
division adds a further ~2.25n²+9n ≈ 150K for the multiply. Two points: (1) in this table the binary
GCD used here has the lower Toffoli count; (2) the space-efficient designs go the wrong way for this
metric (Luo et al. cut qubits to about 800 but raise Toffoli about 170 times, a loss on
Toffoli times qubit).

**Bernstein–Yang is a dead end here.** safegcd/divstep is a branchless, constant-time Euclid variant
that beats Fermat's method on *classical* CPUs, but the advantage does not transfer to reversible
circuits: HJN+20 evaluated it and rejected it because divstep needs an **in-place 2×2 quantum matrix
multiplication for which no efficient reversible circuit is known**, each recursion spawns fresh
ancilla that overwhelm the qubit budget, and its base case "is nearly identical to one Kaliski round"
anyway [HJN+20, App. A.3].

## 3. The second inversion is irreducible

- **Projective / Jacobian coordinates do not win in Shor's setting.** Shor requires a *unique* point
  representation for correct interference; projective coordinates are non-unique, and recovering
  uniqueness needs a division (i.e. an inversion), "it is an open problem to provide a unique
  projective representation with division-free arithmetic" [HJN+20]. For prime fields, projective
  arithmetic also loses the metric outright (Weierstrass projective ~1327n²+1008n, Edwards projective
  ~685n²+392n, vs affine ~432n², plus more qubits) [HJN+20],[Coord25].
- **Montgomery batch inversion does not help a single addition.** It inverts `k` values with `3k−3`
  multiplications + 1 inversion, a per-inversion win only *asymptotically in the batch size*. A
  single reversible point addition is a **batch of one**, so there is no amortization; Litinski
  further notes that under a `qubits×Toffoli` cost function, trading qubits for a sub-2× Toffoli
  reduction *increases* the score [Lit23].

## 4. What the disclosed circuits cost, and their scopes (mined directly)

The ecdsa.fail README cites a published Pareto frontier about 3x below a ~1.5×10⁹ single-addition
score (about 5×10⁸). To check whether that is a standalone single-addition figure, we mined the one
fully disclosed circuit, Schrottenloher 2026, whose Qarton source is public
(`gitlab.inria.fr/capsule/qarton-projects/ec-point-addition`).

Published figures and their scopes. These are different operations or scopes, so they are not a
direct score ranking:

| figure | Toffoli | qubits | scope | source |
|---|---|---|---|---|
| this repository's circuit | ~1.32M total; inversion phase ~666K emitted / ~629K executed | 1152 | one bare affine point addition (ecdsa.fail metric), reproduced and measured locally | `profiling-notes.md` |
| Schrottenloher 2026, space-opt | 2^21.19 ≈ 2.34M | 1192 | one windowed point addition (includes a 2^w=2^16 lookup table) | [Sch26] Table 1 |
| Schrottenloher 2026, gate-opt | 2^20.83 ≈ 1.82M | 1446 | one windowed point addition | [Sch26] Table 1 |
| Schrottenloher 2026, full attack (gate-opt) | 2^25.78 ≈ 57M | 1462 | full Shor attack, 28 windowed additions | [Sch26] Table 2 |
| Google private Pareto point (low-qubit) | 2,700,000 | 1175 | point-addition figures listed in the challenge README and attributed there to Google; the Babbush paper's circuits are withheld behind a zero-knowledge proof, so these are not read off a disclosed circuit | README.md; [Bab26] |
| Google private Pareto point (low-gate) | 2,100,000 | 1425 | as above | README.md; [Bab26] |

Facts from the Schrottenloher circuit (quotes and locations verified against the paper; see
Verification below):
- Its full secp256k1 attack is 28 windowed point additions ([Sch26] Section 2, "In the secp256k1
  case, this means that only 28 point additions are necessary"), total 2^26.11 ≈ 72M Toffoli
  (space-opt) / 2^25.78 ≈ 57M (gate-opt) at 1208 / 1462 qubits ([Sch26] Table 2).
- Each addition performs two full modular inversions, with no cross-addition amortization. Its
  inversion is a binary Extended Euclidean Algorithm of about 1.413n ≈ 361 iterations ([Sch26]
  Section 3.1). The paper gives no isolated per-inversion Toffoli count, so no per-inversion
  comparison to the figures here is drawn.
- A windowed addition selects one of 2^w = 2^16 precomputed multiples, with window w = 16 ([Sch26]
  Section 2, "The value is w=16"). The paper states a lookup of 2^w values costs 2^w Toffoli ([Sch26]
  Section 2, "A table lookup of 2^w values costs 2^w Toffoli gates"), and the full-attack formula
  includes 3·2^16 Toffoli of lookup per addition ([Sch26] Table 2, 28×(2^21.19+3·2^16)). Windowing
  reduces the number of additions in the full attack; it does not make an individual addition cheaper;
  each windowed addition is heavier than a bare one because of the lookup.

Correction to a common back-of-envelope: an earlier inference put the disclosed per-addition cost at
"~140 to 180K Toffoli" by dividing the 70 to 90M full-attack total by a mis-estimated ~350 to 512
additions. The disclosed divisor is 28, giving about 2.0 to 2.6M per windowed addition (Table 1 gives
2^20.83 to 2^21.19 Toffoli of arithmetic, plus the 3·2^16 lookup), consistent with two full inversions
per add.

Conclusion: the disclosed figures are per-windowed-addition (Schrottenloher), the full 28-addition
attack, or resource estimates with withheld circuits (Babbush); none is a bare single-addition figure
directly comparable to ~1.32M / 1152. The README's "about 3x lower" target (about 5×10⁸) corresponds
to no disclosed standalone single-addition circuit; being below the two-inversion cost, it would
require cross-addition windowing a single-addition benchmark does not use, or an inversion with a
lower reversible Toffoli count than any in the surveyed literature.

## 5. The one real lever (constant-factor, uncertain)

If any single-addition improvement is available, the literature points to **windowed modular
arithmetic inside the existing affine + two-inversion structure**, not a new inversion algorithm or
coordinate system:
- **Windowed out-of-place Montgomery multiplication**, ~2.25n²+9n ≈ **150K Toffoli** at window size 16
  [Lit23], cheaper internal multiplies for the point-addition's non-inversion arithmetic.
- **The HJN+20 swap-based single-round Kaliski reformulation**, folding the pseudo-inverse doubling
  correction into the division rounds [HJN+20], a cheaper per-round inversion primitive.

These attack *constant factors*, and their benefit at a **fixed peak-qubit cap is unproven**, windowed
multiplication needs precomputed lookup tables (more qubits), and a design already at its width cap
cannot spend them for free. This is a research/prototyping direction with an uncertain ceiling, not a
quick win.

## 6. Recommendation

1. **Do not switch inversion algorithm or coordinate system.** In the surveyed literature the binary
   GCD used here (about 629K, measured) has a lower Toffoli count than the published inverters with
   stated per-inversion figures (Litinski about 1.70M, Qualtran about 1.71M; §2). Divstep is rejected
   for reversible use, and projective coordinates cost more Toffoli and do not give Shor's required
   unique representation.
2. **The disclosed figures have been mined (§4).** They are per-windowed-addition or full-attack
   (Schrottenloher) or resource estimates with withheld circuits (Babbush), not bare single-addition
   figures directly comparable to about 1.32M / 1152. The circuit here is at the two-inversion cost of
   affine point addition.
3. **The remaining constant-factor lever is real but small and unproven at a fixed peak.** Window-16
   out-of-place Montgomery multiplication (~150K Toffoli [Lit23]) and the HJN+20 swap-based Kaliski
   round could shave the *non-inversion* arithmetic and per-round inversion cost, but windowed
   multiplication needs lookup tables (more qubits) and a design at its width cap cannot spend them
   for free. Prototype only if a genuine standalone-single-addition improvement is the goal, with an
   uncertain ceiling.
4. **Verify the challenge's target metric.** The README's "≈3× lower" (~5×10⁸) matches no disclosed
   standalone single-addition circuit and lies below the two-inversion floor; before investing,
   confirm whether ecdsa.fail scores a truly isolated addition (in which case ~1.5×10⁹ is at/near the
   floor) or something windowed/amortized.

## Verification and provenance

Figures checked directly against the primary source in this pass, with quotes and locations so a
reader can confirm each:

- **Schrottenloher [Sch26]** (arXiv:2606.02235): window w = 16 and "A table lookup of 2^w values costs
  2^w Toffoli gates" (Section 2); "only 28 point additions are necessary" (Section 2); per-addition
  2^21.19 (space) / 2^20.83 (gate) Toffoli at 1192 / 1446 qubits (Table 1); full attack
  28×(2^21.19+3·2^16) = 2^26.11 and 28×(2^20.83+3·2^16) = 2^25.78 at 1208 / 1462 qubits (Table 2);
  "around 1.5% increase" qubits and "between 6.5% and 10% reduction" Toffoli vs Babbush (Section 1);
  inversion iteration count ≃ 1.413n (Section 3.1); the paper gives no isolated per-inversion Toffoli
  count.
- **Litinski [Lit23]** (arXiv:2306.08585, PDF): inversion "total cost of this inversion operation is
  26n² + 2n Toffolis" and "over 10 times more expensive than a multiplication" (Appendix C5, block
  repeated 2n times, 13n Toffoli per block); multiplication "a total Toffoli count of 2.25n² + 9n";
  the cost function "nQ · nTof" and that trading qubits for a small Toffoli reduction is "not a
  favorable trade-off" in a baseline architecture; batch inversion "3k − 3 modular multiplications and
  a single modular inversion".
- **Babbush/Google [Bab26]** (ePrint 2026/625): "Shor's algorithm for this problem can execute with
  either ≤ 1200 logical qubits and ≤ 90 million Toffoli gates or ≤ 1450 logical qubits and ≤ 70
  million Toffoli gates"; circuits withheld: "we use a zero-knowledge proof to validate these results
  without disclosing attack vectors".

Figures cited to a source but not independently re-verified in this pass, treat as reported values:
the Qualtran per-inversion 26n²+9n−1, the HJN+20 ≈2.35M/add and coordinate-cost formulas, and the
Luo et al. 204n²log₂n and qubit figures.

The 2,700,000 / 1175 and 2,100,000 / 1425 figures are listed in the challenge README (this repo,
"Reference numbers") and attributed there to Google's private Pareto points; the Babbush paper's
circuits are withheld, so these are not read off a disclosed circuit.

## Caveats

- The in-circuit inversion figure is the measured Toffoli of the `tlm_inverse` phase, which is a
  division (inversion plus a multiply); the literature rows are inversion only. Cross-paper Toffoli
  comparisons use different circuit decompositions and gate-count conventions, so treat them as
  order-of-magnitude, not a controlled benchmark.
- The claim (seen in secondary coverage) that Babbush beats Litinski "2 to 3 times in both gate and
  qubit count" did not hold up on checking; the disclosed delta is a large Toffoli reduction with a
  comparable or modest qubit change.
- Some eprint PDFs were access-blocked and were corroborated via arXiv or the extracted PDF text.

## Sources

- **[Lit23]** D. Litinski, *How to compute a 256-bit elliptic curve private key with only 50 million
  Toffoli gates*, arXiv:2306.08585 (2023).
- **[HJN+20]** Häner, Jaques, Naehrig, Roetteler, Soeken, *Improved Quantum Circuits for Elliptic
  Curve Discrete Logarithms*, IACR ePrint 2020/077.
- **[RNSL17]** Roetteler, Naehrig, Svore, Lauter, *Quantum Resource Estimates for Computing Elliptic
  Curve Discrete Logarithms*, arXiv:1706.06752 / ePrint 2017/598 (2017).
- **[BY19]** Bernstein, Yang, *Fast constant-time gcd computation and modular inversion*, IACR ePrint
  2019/266 (2019).
- **[Bab26]** Babbush et al. (Google Quantum AI), *Securing Elliptic Curve Cryptocurrencies against
  Quantum Vulnerabilities: Resource Estimates and Mitigations*, IACR ePrint 2026/625 (2026).
- **[Sch26]** Schrottenloher, *Optimized Point Addition Circuits for Elliptic Curve Discrete
  Logarithms*, arXiv:2606.02235 / ePrint 2026/1128 (2026), disclosed circuit, Qarton source at
  `gitlab.inria.fr/capsule/qarton-projects/ec-point-addition`.
- **[Luo26]** Luo et al., *Space-efficient reversible ECDLP (refined Proos–Zalka register sharing)*,
  arXiv:2604.02311 (2026).
- **[Coord25]** *Choosing Coordinate Forms for Solving ECDLP Using Shor's Algorithm*,
  arXiv:2502.12441 (2025).
- **[Qualtran]** Google Qualtran reversible EC point-addition implementation (26n²+9n−1 per inversion).

---

*Method: 5-angle parallel web survey → 21 primary sources → 94 extracted claims → 25 adversarially
verified (3-vote, 2/3-to-refute) → 20 confirmed. Findings above are the confirmed set; refuted claims
(e.g. batch-inversion avoiding the second inversion; Babbush "2–3× in both axes") were dropped.*
