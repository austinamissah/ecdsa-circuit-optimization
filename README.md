# The secp256k1 Point-Addition Challenge

> **Goal.** Build the cheapest reversible quantum circuit that performs one
> elliptic-curve point addition on **secp256k1**, scored by the product of
> **Toffoli count × peak qubit width**.

---

## Why this matters

Shor's algorithm breaks elliptic-curve cryptography by computing discrete
logarithms in time polynomial in the bit-width of the curve. The quantum cost
of *running* Shor on an ECC group is dominated by one inner primitive,
repeated thousands of times: **point addition** on the curve.

Faster point addition ⇒ fewer Toffoli gates ⇒ fewer magic states ⇒ less
physical hardware and less wall-clock time on a fault-tolerant quantum
computer. Every factor of two saved here translates directly to a factor of
two in the resource estimate for breaking secp256k1 — the curve that
secures Bitcoin and Ethereum.

---

## The benchmark, precisely

You are given a Rust harness that:

1. **Builds** a reversible circuit by calling `point_add::build()`.
   The circuit must consume four 256-element registers — `target_x`
   (qubits), `target_y` (qubits), `offset_x` (classical bits),
   `offset_y` (classical bits) — and overwrite `(target_x, target_y)`
   with the affine sum `(target_x, target_y) + (offset_x, offset_y)` on
   the secp256k1 curve.
2. **Validates** the circuit by simulating it on 9024 random test points.
   Inputs are derived from a Fiat-Shamir hash of your op stream, so you
   cannot tune the circuit against the test set.
3. **Counts** every Toffoli, every Clifford, and the peak number of live
   qubits.
4. **Scores** the run as

   $$\text{score} \;=\; \overline{\text{Toffoli}} \;\times\; \text{peak qubits}$$

   where $\overline{\text{Toffoli}}$ is the average executed Toffoli count
   per shot. **Lower is better.** The score is written to `score.json`.

### What "valid" means

A run is rejected if any of the following fails:

- **Classical correctness.** All 9024 shots must produce the right
  `(R_x, R_y)`.
- **Reversibility.** Every ancilla qubit must be uncomputed to $|0\rangle$
  before being freed. `sim.rs` enforces this on every freed qubit. After
  the forward pass, every non-output qubit must again be $|0\rangle$.
- **Phase cleanliness.** The global phase across all live shots must be
  zero — no leftover phase kickback from a sloppy uncomputation.
- **Forward∘reverse identity.** Running the circuit and then its gate-
  reversed inverse must restore the original state on every qubit.

There are no loopholes. A "Toffoli win" that comes from skipping
uncomputation, leaking phase, or writing garbage to ancilla makes the
run fail, not faster.

### Reference numbers

| | Toffoli (avg/shot) | Peak qubits | Score |
|---|---|---|---|
| Challenge initial circuit | 3,942,753 | 2,715 | 1.07 × 10¹⁰ |
| Google's private low-qubit Pareto point | 2,700,000 | 1,175 | 3.2 × 10⁹ |
| Google's private low-gate Pareto point | 2,100,000 | 1,425 | 3.0 × 10⁹ |

The upstream challenge README states that a research loop cut the score about 33× from the textbook
baseline, that the published Pareto frontier sits about 3× lower still, and that its authors believe
both points on that frontier and points below them are beatable. Those are the upstream authors'
claims. This fork's own analysis is in the next section.

### What this fork did, and what it found

*Project write-up: **[amissah.net](https://amissah.net)**.*

**Scope, plainly stated.** The circuit is not mine. It is the community-contributed frontier from
the challenge, and by now it also carries a large body of another contestant's work (see
[What is not this fork's work](#what-is-not-this-forks-work)). What is mine is the *measurement and
analysis*: reproducing the circuit locally, profiling where its cost actually goes, checking the
published comparisons against their sources, enumerating what could be improved — and then, when
that analysis turned out to be partly wrong, measuring why.

As of the 2026-08-02 rebase onto upstream `8af8a6f`, the circuit reproduces locally at
**1,289,073.125 Toffoli × 1,154 qubits = 1,487,590,242**, 9,024/9,024 shots clean.

#### 1. My first conclusion was wrong, and the record of that is kept

The first pass, written against commit `422f21d`, measured 1,320,763 × 1,152 (≈1.52 × 10⁹),
enumerated eleven possible improvements, found all eleven blocked, and concluded that **no lever was
available** — that only a research-scale rewrite could lower the score.

In the three weeks that followed, upstream landed **25 accepted score-lowering submissions**, none
of them a rewrite. So the conclusion was wrong.

Rather than quietly correct it, the original claims are kept verbatim in
[`docs/CONCLUSION.md`](docs/CONCLUSION.md) and marked in place, with a
[lever verdict audit](docs/CONCLUSION.md#lever-verdict-audit) that quotes each superseded claim and
states what refuted it. **Seven of the eleven verdicts stand; four moved.** The most instructive
failure is lever 11: the *measurements* were correct, but I generalised them into a structural floor
that does not exist — the peak qubit count tracks a configuration cap, and 124 of those qubits are a
borrowed pool that expands to fill whatever cap is set.

#### 2. The score is not the binding constraint — λ is

This is the finding I think is worth other people's attention.

The benchmark hashes its 9,024 test inputs from the circuit's own op stream. So **any change that
lowers the score also re-rolls the test inputs.** A circuit therefore has no fixed pass/fail status;
it has an intrinsic failure *rate*, called λ here, and shipping requires searching for an input seed
on which it happens to pass everything.

Measured on the current head over 199 independent seeds, each a full benchmark run:
**λ_total ≈ 20.0**, so P(clean seed) ≈ 2.0 × 10⁻⁹ — about **5 × 10⁸ trials per usable seed**. At the
throughput actually measured on the laptop used (205 full benchmark runs per hour), that is on the
order of **280 wall-years**. The 95% confidence interval on λ spans a factor of ~38 in that figure,
so read it as an order of magnitude rather than a number.

That reframes the whole problem. Optimising the score is the easy half; the hard half is that every
optimisation must then be paid for in seed-search. Method and full numbers:
[`docs/lambda-measurement.md`](docs/lambda-measurement.md), raw per-trial data in
[`docs/data/`](docs/data/).

#### 3. How the leaderboard leader actually operates

The leader is a single autonomous agent (`yukon-autoresearch[bot]`) landing a submission every 0.88
days, and it ships its search controller inside its own submissions. Reading it explains the pace
without any exotic trick: a **512 / 2,048 / 8,192 / 9,024 shot ladder** that rejects bad seeds early
(7.2× cheaper at λ = 20), plus a per-trial cost roughly 50× below a naive full run. It was also
grinding at much lower λ for most of the campaign.

Notably, **λ appears nowhere in its selection function** — it is tolerated as a pass/fail gate,
never optimised. Details: [`docs/upstream-search-economics.md`](docs/upstream-search-economics.md).

#### 4. Measurement traps that silently invalidate results

Several findings are about *method*, and they generalise beyond this project. The sharpest: the
benchmark script runs the build under `sudo`, and sudo's `env_reset` **silently strips the
environment variable** that selects the seed — so every trial measures the default and returns a
byte-identical artifact. It looks exactly like "my change had no effect". It is also intermittent,
because sudo's credential cache expires mid-run.

The only thing that caught it was a rule borrowed from the other contestant's notes: *a null result
is only a result if the output hash changed.* That rule is now load-bearing in everything here.

#### Where the cost actually is

About 95% of the budget is the two modular inversions that reversible affine point addition
requires. In the literature surveyed in
[`docs/quantum-inversion-frontier-research.md`](docs/quantum-inversion-frontier-research.md), no
reversible modular-inversion implementation has a lower Toffoli count than the windowed binary GCD
used here.

Published figures for related work are for different operations or scopes and are not directly
comparable to one bare addition: Schrottenloher 2026 reports per-windowed-addition and full-attack
figures (a windowed addition includes a 2^16-entry lookup table), and the Google/Babbush figures are
resource estimates with the circuits withheld behind a zero-knowledge proof. See
[`docs/quantum-inversion-frontier-research.md`](docs/quantum-inversion-frontier-research.md) for the
scopes.

Start with [`docs/CONCLUSION.md`](docs/CONCLUSION.md) for the full write-up and the audit of what
was wrong. See [`docs/`](docs/) for the per-component analyses and the current findings.

---

## How to play

Using the ECDSA Fail CLI:

1. Install the CLI:

   ```bash
   curl -fsSL https://api.ecdsa.fail/install.sh | sh
   ```

2. Create an API key from the top-right menu.
3. Log in:

   ```bash
   ecdsafail login <api-key>
   ```

4. Clone the benchmark:

   ```bash
   ecdsafail clone
   ```

5. Improve your circuit.
6. Run and submit:

   ```bash
   ecdsafail run
   ecdsafail submit
   ```

The leaderboard moves only when a submission lowers the score. A run that
matches the current best still builds, validates, and scores correctly; to
move the leaderboard, the aim is to improve on the current frontier rather
than match it.

You can also run the harness directly:

```bash
cargo run --release -- --note "what I tried"
```

That single command builds the circuit, validates it, scores it, and
appends one row to `results.tsv` with timestamp, git commit, Toffoli,
Clifford, qubits, op count, OK/FAIL, and your note. The score is also
written to `score.json` in the format

```json
{ "score": 10704574395, "metrics": { "toffoli": 3942753, "qubits": 2715 } }
```

### What you can edit

You may modify **anything inside `src/point_add/`** — split it into
submodules, rewrite primitives, swap algorithms, refactor freely.

You may **not** touch the harness:

- `src/main.rs`, `src/circuit.rs`, `src/sim.rs`,
  `src/weierstrass_elliptic_curve.rs` — these are the contract.
- `Cargo.toml`, `Cargo.lock`, `rust-toolchain` — no new dependencies.
- `results.tsv` directly (the harness appends to it for you).

### Memory notes

As you iterate, add Markdown notes under `src/point_add/memory/`
capturing approaches that worked and the reasoning behind important choices.

### Important note on openness

This codebase is open to contributions chasing the best score, so memory and
source files may come from different agents. Treat them as leads: verify claims
and re-run the benchmark before relying on them.

Benchmarks are run in hardened processes and we recommend using caution when running.

## How this was built

The profiling, the optimization analysis, the λ measurement, and the documents under
`docs/` in this fork were produced with Claude Code, Anthropic's agentic coding tool.
The reversible circuit itself is the community's work from the challenge repository;
my part was reproducing and validating it locally, measuring where its cost is and
what actually gates progress, and writing up both the findings and the errors — and I
did that work with Claude Code.

Every number quoted from this fork's own measurements is reproducible from the repo:
the score from `./benchmark.sh`, the λ figures from the raw per-trial data in
[`docs/data/`](docs/data/). Figures quoted from outside papers were checked against
their sources; see `docs/quantum-inversion-frontier-research.md` for the provenance.
Where a claim is an inference rather than a measurement, the documents say so.

### What is not this fork's work

Beyond the circuit itself, the 2026-08-02 rebase onto upstream `8af8a6f` brought in a substantial
body of another contestant's material, which ships inside `src/point_add/` because that is the
challenge's `editablePaths` root. None of it is ours:

- **`src/point_add/memory/01-05` and `README.md`** (533 lines) — their working notes on the
  circuit's architecture, its intrinsic error rate, proven floors, traps, and the qubit programme.
  Attributed to whoever authored the accepted submissions; the upstream commits are authored by
  `yukon-autoresearch[bot]`.
- **`src/point_add/memory/repro/dgm_search.py`** (2,848 lines) and `test_dgm_search.py` (507) —
  their Darwin-Gödel-Machine search controller. It ports mechanisms from
  [jennyzzt/dgm](https://github.com/jennyzzt/dgm) (pinned at `a565fd2`), as its own header states.

Our documents cite these heavily and are careful to say so at each point. `docs/` is this fork's
work; `src/point_add/` is not.

## Credits

This benchmark harness was adapted from code Google published with
["Securing Elliptic Curve Cryptocurrencies against Quantum Vulnerabilities:
Resource Estimates and Mitigations"](https://research.google/pubs/securing-elliptic-curve-cryptocurrencies-against-quantum-vulnerabilities-resource-estimates-and-mitigations/)
and its [companion Zenodo dataset](https://zenodo.org/records/19597130).
Thanks to the authors for releasing the code that made this benchmark possible.

Thanks to [Kirk Baird](https://github.com/kirk-baird) from SigmaPrime for reviewing the codebase.

The analysis in `docs/` leans heavily on the working notes and search controller that
arrived with the upstream rebase, in `src/point_add/memory/` — another contestant's
work, not mine. Several of this fork's findings are extensions of theirs, and the
documents cite them at each point. See
[What is not this fork's work](#what-is-not-this-forks-work).

Project write-up: **[amissah.net](https://amissah.net)**.
