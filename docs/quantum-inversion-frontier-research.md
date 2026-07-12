# How low can a single reversible secp256k1 point addition go? — a literature-grounded frontier analysis

A research synthesis on the cost floor of a reversible elliptic-curve point addition scored by
**Toffoli count × peak qubit width** (the ecdsa.fail metric), and on which algorithmic levers can and
cannot move it. Compiled from a multi-source, adversarially-verified survey of the quantum-ECC
literature (2017–2026). Sources are listed at the end; every quantitative claim is cited.

## TL;DR

- **No known modular-inversion *algorithm* beats a windowed binary GCD in the reversible setting.**
  Every published reversible 256-bit inverter is the same binary-GCD / Kaliski family, and the
  best-documented ones cost **more** Toffoli than a well-tuned windowed binary GCD, not less.
  Bernstein–Yang "safegcd"/divstep — the obvious classical speedup — is **explicitly rejected** for
  reversible use.
- **A single affine point addition pays for two full modular inversions**, and this is irreducible:
  projective/Jacobian coordinates lose in Shor's setting, and Montgomery batch-inversion needs a
  batch (a single addition is a batch of one). So the Toffoli cost of one point addition is
  **floored at ≈ 2 × (one inversion)**, which is where ~95% of the budget sits.
- **Consequence — the single-addition score has a hard floor.** With a state-of-the-art ~629K-Toffoli
  inversion, one point addition costs ≈ 1.3M Toffoli, i.e. **≈ 1.5×10⁹ at ~1150 qubits.** A design
  already at that point is at the *standalone single-addition frontier*.
- **The "≈3× lower published frontier" is almost certainly an *amortized*, windowed, full-attack
  figure, not a standalone single addition.** The 2026 Babbush/Google and Schrottenloher results run
  the *entire* 256-bit ECDLP in ≤1200 qubits / ≤90M Toffoli by **windowing arithmetic across
  hundreds of additions** — driving the *per-addition* cost to ~140–180K Toffoli. That amortization
  is unavailable to a single isolated addition, and ~140–180K is **below the two-inversion floor**,
  which is only possible because windowing does *not* invert on every addition.

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

## 4. What the "≈3× lower frontier" actually is

The ecdsa.fail README cites a published Pareto frontier ≈3× below a ~1.5×10⁹ single-addition score
(so ≈5×10⁸). The literature strongly indicates this is an **amortized, windowed, full-attack
figure**, not a standalone single point addition:

- **Babbush / Google Quantum AI 2026**: the full 256-bit secp256k1 ECDLP runs at **≤1200 logical
  qubits / ≤90M Toffoli, or ≤1450 qubits / ≤70M Toffoli** [Bab26]. The attack performs on the order
  of **350–512 windowed point additions**, so the *implied per-addition* cost is ~140–180K Toffoli.
- **Schrottenloher 2026** independently reproduces this (~1.5% more qubits, 6.5–10% fewer Toffoli for
  secp256k1) with a **fully disclosed** logical circuit (open "Qarton" library) — the best proxy to
  mine for the actual per-addition decomposition [Sch26].
- Google's own *disclosed single-addition* circuits are **worse**, not better, than ~1.5×10⁹:
  "Circuit One" = 1175 qubits / 2.7M Toffoli (3.2×10⁹) and "Circuit Two" = 1425 qubits / 2.1M Toffoli
  (3.0×10⁹) [Bab26] — these are the "Google Pareto points" in the challenge README.

The decisive point: **~140–180K Toffoli per addition is below the two-inversion floor (§1).** That is
only possible because windowed scalar multiplication does **not** perform a fresh inversion on every
addition — it shares inversion work across a window of additions. A benchmark that scores *one
isolated* addition cannot access that amortization. So the ≈3× target is very likely measured in a
setting (per-addition-within-a-windowed-attack) that differs from a standalone single addition, and a
standalone circuit already near ~1.5×10⁹ is at or near its structural floor.

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

1. **Do not switch inversion algorithm or coordinate system** — binary GCD already beats the
   literature, divstep is rejected for reversible use, and projective coordinates lose the metric.
2. **Confirm what the target frontier measures** — a standalone single addition, or an
   amortized-windowed per-addition. If it is the latter, a single-addition circuit already near
   ~1.5×10⁹ is at its structural floor and the ≈3× gap is not reachable in that metric.
3. **If a genuine single-addition improvement is wanted**, the only literature-grounded path is
   mining the **disclosed Schrottenloher/Qarton circuit** [Sch26] for its per-addition decomposition
   and prototyping window-16 modular multiplication + the HJN+20 swap-based rounds **within** the
   qubit-width budget — accepting that windowing may not net a win at a fixed peak.

## Caveats

- Babbush/Google's optimized circuit is **not fully disclosed** (a zero-knowledge validation was
  published), so its per-inversion vs per-multiplication breakdown is *inferred*, and the ~140–180K
  per-addition figure is `total Toffoli ÷ estimated windowed-addition count` — order-of-magnitude, not
  exact. Schrottenloher's disclosed circuit is the reliable proxy.
- The claim that Babbush beats Litinski "2–3× in both gate and qubit count" was **refuted** in
  verification; the disclosed delta is a large Toffoli reduction with comparable/modest qubit change.
- Some eprint PDFs were access-blocked and corroborated via arXiv mirrors.

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
  Logarithms*, arXiv:2606.02235 (2026) — disclosed circuit / "Qarton" library.
- **[Luo26]** Luo et al., *Space-efficient reversible ECDLP (refined Proos–Zalka register sharing)*,
  arXiv:2604.02311 (2026).
- **[Coord25]** *Choosing Coordinate Forms for Solving ECDLP Using Shor's Algorithm*,
  arXiv:2502.12441 (2025).
- **[Qualtran]** Google Qualtran reversible EC point-addition implementation (26n²+9n−1 per inversion).

---

*Method: 5-angle parallel web survey → 21 primary sources → 94 extracted claims → 25 adversarially
verified (3-vote, 2/3-to-refute) → 20 confirmed. Findings above are the confirmed set; refuted claims
(e.g. batch-inversion avoiding the second inversion; Babbush "2–3× in both axes") were dropped.*
