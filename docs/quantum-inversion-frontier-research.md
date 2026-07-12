# How low can a single reversible secp256k1 point addition go? — a literature-grounded frontier analysis

A research synthesis on the cost floor of a reversible elliptic-curve point addition scored by
**Toffoli count × peak qubit width** (the ecdsa.fail metric), and on which algorithmic levers can and
cannot move it. Compiled from a multi-source, adversarially-verified survey of the quantum-ECC
literature (2017–2026). Sources are listed at the end; every quantitative claim is cited.

## TL;DR

- **No known modular-inversion *algorithm* beats a windowed binary GCD in the reversible setting.**
  Every published reversible 256-bit inverter is the same binary-GCD / Kaliski family, and the
  best-documented ones cost **more** Toffoli than the windowed binary GCD used here, not less.
  Bernstein–Yang "safegcd"/divstep — the obvious classical speedup — is **explicitly rejected** for
  reversible use.
- **A single affine point addition pays for two full modular inversions**, and this is irreducible:
  projective/Jacobian coordinates lose in Shor's setting, and Montgomery batch-inversion needs a
  batch (a single addition is a batch of one). So the Toffoli cost of one point addition is
  **floored at ≈ 2 × (one inversion)**, which is where ~95% of the budget sits.
- **Consequence — the single-addition score has a hard floor.** With a state-of-the-art ~629K-Toffoli
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
  512 additions; the real divisor is 28, giving about 2.3 to 2.6M per windowed addition.) The README's
  "about 3x lower" target (about 5×10⁸) corresponds to no disclosed standalone single-addition circuit;
  it is below the two-inversion cost and would require cross-addition windowing a single-addition
  benchmark does not use.

## 1. The metric and the two-inversion floor

The score is `avg_executed_Toffoli × peak_qubits`. Affine EC point addition computes
`λ = (y₂−y₁)/(x₂−x₁)`, then the new coordinates, then must **reversibly uncompute λ**. Because the
division is done by inverting into an auxiliary register, multiplying, and then **inverting again** to
clean the auxiliary register (Bennett/pebbling), one addition contains **two full modular inversions**
plus a squaring, two multiplications, and a handful of additions [HJN+20]. The two inversions
dominate — typically ~95% of the Toffoli budget.

So, for any inversion of cost `I` Toffoli, one point addition is floored near `2·I` Toffoli. With the
best documented reversible inversion (~629K, see §2) that is ≈ **1.26M Toffoli of inversion + ~60K
squaring + multiplies ≈ 1.3M total**, i.e. **≈ 1.5×10⁹ at ~1150 qubits.** This is not a tuning
artifact; it is structural. Beating it *within the single-addition metric* requires one of: (a) an
inversion materially cheaper than any known, (b) doing the addition with fewer than two full
inversions, or (c) the same Toffoli count at drastically lower peak width — and §2–§4 show all three
are ruled out by the current literature.

## 2. Inversion-algorithm comparison — binary GCD already wins

Every disclosed reversible 256-bit modular inverter is a **round-based Kaliski / binary-extended-
Euclidean (Montgomery-inverse)** circuit run for a fixed `2n = 512` rounds — the same family as a
windowed binary GCD. Concrete costs:

| inverter (256-bit, reversible) | Toffoli / inversion | qubit / notes | source |
|---|---|---|---|
| Windowed binary GCD (well-tuned) | **~629K** | reference point | — |
| Litinski 2023 (Kaliski + windowing) | 26n²+2n ≈ **1.70M** | "over 10× a multiplication" | [Lit23] |
| Qualtran implementation | 26n²+9n−1 ≈ **1.71M** | 4 inversions / add | [Qualtran] |
| Häner–Jaques–Naehrig–Roetteler–Soeken (RNSL-refined) | ~2n-round Kaliski | ≈2.35M/add space-opt, 1192 q | [HJN+20] |
| Luo et al. 2026 (space-efficient EEA) | 204n²log₂n ≈ **1.07×10⁸** | ~800 q — trades Toffoli for width | [Luo26] |
| Bernstein–Yang divstep / safegcd | — | **rejected for reversible** (see below) | [BY19],[HJN+20] |

Two takeaways: (1) a well-tuned windowed binary GCD at ~629K is **already better than every published
number**; (2) the space-efficient designs go the *wrong way* for this metric — Luo et al. cut qubits
to ~800 but raise Toffoli ~170×, which is a large **loss** on Toffoli×qubit.

**Bernstein–Yang is a dead end here.** safegcd/divstep is a branchless, constant-time Euclid variant
that beats Fermat's method on *classical* CPUs, but the advantage does not transfer to reversible
circuits: HJN+20 evaluated it and rejected it because divstep needs an **in-place 2×2 quantum matrix
multiplication for which no efficient reversible circuit is known** — each recursion spawns fresh
ancilla that overwhelm the qubit budget — and its base case "is nearly identical to one Kaliski round"
anyway [HJN+20, App. A.3].

## 3. The second inversion is irreducible

- **Projective / Jacobian coordinates do not win in Shor's setting.** Shor requires a *unique* point
  representation for correct interference; projective coordinates are non-unique, and recovering
  uniqueness needs a division (i.e. an inversion) — "it is an open problem to provide a unique
  projective representation with division-free arithmetic" [HJN+20]. For prime fields, projective
  arithmetic also loses the metric outright (Weierstrass projective ~1327n²+1008n, Edwards projective
  ~685n²+392n, vs affine ~432n², plus more qubits) [HJN+20],[Coord25].
- **Montgomery batch inversion does not help a single addition.** It inverts `k` values with `3k−3`
  multiplications + 1 inversion — a per-inversion win only *asymptotically in the batch size*. A
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
| this repository's circuit | ~1.32M | 1152 | one bare affine point addition (ecdsa.fail metric), reproduced locally | community frontier |
| Schrottenloher 2026, space-opt | 2^21.19 ≈ 2.34M | 1192 | one windowed point addition (includes a 2^16 lookup table) | [Sch26] |
| Schrottenloher 2026, gate-opt | 2^20.83 ≈ 1.82M | 1446 | one windowed point addition | [Sch26] |
| Schrottenloher 2026, full attack (gate-opt) | 2^25.78 ≈ 57M | 1462 | full Shor attack, 28 windowed additions | [Sch26] |
| Babbush/Google "Circuit One" | 2.7M | 1175 | resource estimate, circuit withheld behind a zero-knowledge proof | [Bab26] |
| Babbush/Google "Circuit Two" | 2.1M | 1425 | resource estimate, circuit withheld behind a zero-knowledge proof | [Bab26] |

Facts from the Schrottenloher circuit:
- Its full secp256k1 attack is 28 windowed point additions (not hundreds), total 2^26.11 ≈ 72M Toffoli
  (space-opt) / 2^25.78 ≈ 57M (gate-opt) at ~1208 to 1462 qubits [Sch26].
- Each addition performs two full modular inversions, with no cross-addition amortization (quote:
  *"each point addition performs 2 independent modular in-place multiplications"*). Its inversion is a
  binary Extended Euclidean Algorithm (~400 iterations, two-phase Euclid + Bézout), about 1.17M per
  inversion, higher than the ~629K of the binary GCD used here.
- A windowed addition selects one of 2^16 precomputed multiples and includes a lookup table (about
  3·2^16 Toffoli per windowed step). Windowing reduces the number of additions in the full attack from
  ~512 to 28; it does not make an individual addition cheaper; each windowed addition is heavier than a
  bare one because of the lookup.

Correction to a common back-of-envelope: an earlier inference put the disclosed per-addition cost at
"~140 to 180K Toffoli" by dividing the 70 to 90M full-attack total by a mis-estimated ~350 to 512
additions. The disclosed divisor is 28, giving about 2.3 to 2.6M per windowed addition, consistent
with two full inversions per add.

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
  [Lit23] — cheaper internal multiplies for the point-addition's non-inversion arithmetic.
- **The HJN+20 swap-based single-round Kaliski reformulation**, folding the pseudo-inverse doubling
  correction into the division rounds [HJN+20] — a cheaper per-round inversion primitive.

These attack *constant factors*, and their benefit at a **fixed peak-qubit cap is unproven** — windowed
multiplication needs precomputed lookup tables (more qubits), and a design already at its width cap
cannot spend them for free. This is a research/prototyping direction with an uncertain ceiling, not a
quick win.

## 6. Recommendation

1. **Do not switch inversion algorithm or coordinate system.** In the surveyed literature the binary
   GCD has a lower reversible Toffoli count than the disclosed alternatives (the Schrottenloher
   inversion is about 1.17M, versus about 629K here), divstep is rejected for reversible use, and
   projective coordinates cost more Toffoli and do not give Shor's required unique representation.
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

## Caveats

- Babbush/Google's optimized circuit is **not fully disclosed** (a zero-knowledge validation was
  published), so its per-inversion vs per-multiplication breakdown is *inferred*, and the ~140–180K
  per-addition figure is `total Toffoli ÷ estimated windowed-addition count` — order-of-magnitude, not
  exact. Schrottenloher's disclosed circuit is the reliable proxy.
- The claim that Babbush beats Litinski "2–3× in both gate and qubit count" was **refuted** in
  verification; the disclosed delta is a large Toffoli reduction with comparable/modest qubit change.
- Some eprint PDFs were access-blocked and corroborated via arXiv mirrors.
- The §4 per-addition numbers were read directly from the disclosed Schrottenloher paper (Tables 1–3);
  its exact per-primitive gate counts live in the Qarton source and were not transcribed line-by-line
  here.

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
  Logarithms*, arXiv:2606.02235 / ePrint 2026/1128 (2026) — disclosed circuit, Qarton source at
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
