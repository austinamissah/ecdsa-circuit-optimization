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

Faster point addition means fewer Toffoli gates, which means fewer magic states,
which means less physical hardware and less wall-clock time on a fault-tolerant
quantum computer. Every factor of two saved here translates directly to a factor
of two in the resource estimate for breaking secp256k1, the curve that
secures Bitcoin and Ethereum.

---

## The benchmark, precisely

You are given a Rust harness that:

1. **Builds** a reversible circuit by calling `point_add::build()`.
   The circuit must consume four 256-element registers: `target_x`
   (qubits), `target_y` (qubits), `offset_x` (classical bits), and
   `offset_y` (classical bits). It must overwrite `(target_x, target_y)`
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
  zero, with no leftover phase kickback from a sloppy uncomputation.
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

> **Read this first: every score in this file is a snapshot, and the circuit itself gets replaced.**
> This is an open, live benchmark. On 2026-08-23 alone the leaderboard moved six times and the
> whole construction changed underneath the analysis: sections 1 to 5 below, and most of
> [`docs/`](docs/), measure a trailmix/dialog circuit at 1,154 qubits and score 1,486,468,554 that
> no longer ships. It was replaced by a fixed-depth ping-pong division, which by that evening was
> at 1,267 qubits and 1,154,731,130, and it will have moved again by the time you read this.
>
> Nothing here is rewritten to look current. Each measurement names the commit it was taken on, and
> is kept as a record of what was true then. Upstream keeps its own notes the same way. **What is
> meant to survive is the method and the lessons, not the numbers**, and the lessons are the point:
> see §6 for two conclusions of mine that were wrong, and §7 for what happened when I tried to
> compete on the leaderboard directly. Current-construction detail lives in
> [`docs/pingpong-2026-08-23.md`](docs/pingpong-2026-08-23.md).

**What is mine and what is not.** The circuit is not mine. It came from the challenge, built by the
community, and as of `6909d15` it also carries **13,909 lines across 50 files** under
`src/point_add/` written by another contestant, not by us. See
[What is not this fork's work](#what-is-not-this-forks-work). What is mine is the measuring and
the analysis: building the circuit locally, finding where its cost goes, checking the published
comparisons against the papers they came from, listing what could be improved, and then, when that
list turned out to be partly wrong, measuring why.

#### 1. The score is not what limits you. λ is.

The benchmark builds its 9,024 test inputs by hashing the circuit's own op stream. So **any change
that lowers the score also re-rolls the test inputs.** That means a circuit is never simply passing
or failing. It has a built-in failure *rate*, called λ here, and to ship anything you have to search
for an input seed that it happens to pass completely.

To be clear about what that does and does not mean: **λ does not block anyone, it prices them, and
the price has been going up.** Improvements land all the time, 25 of them in three weeks (§2). Each
one just has to be paid for with a seed search, and λ sets that bill. You can watch the bill rise in
the leader's own record: eight submissions on one day while λ was still low, down to three on a day
three weeks later (§3). Nothing stopped working. It got more expensive.

Measured on the current head `6909d15` over 199 independent seeds, each one a full benchmark run:
**λ_total = 20.560** (95% CI 18.007 to 23.016), so P(clean seed) ≈ 1.2 × 10⁻⁹, which is about
**8.5 × 10⁸ trials per usable seed**. At the throughput actually measured on the laptop used (183
full benchmark runs per hour), that is on the order of **500 years of real time**. The confidence
interval spans a factor of about 150 in that figure, so read it as an order of magnitude rather
than an exact number.

**And λ has not moved.** The same measurement on `801dd20` gave 20.04. The bootstrapped difference
is **+0.525, 95% CI −2.626 to +3.632**, and 37% of the resamples put the difference at zero or
below. (A bootstrap resamples the data many times over to see how much a number wobbles.) On this
evidence the two heads have the same λ_total. Between those two heads upstream accepted **eight
submissions**, every one of them lowering the score, and the failure rate came out statistically
unchanged. That is the stronger result: λ is not drifting in either direction as a side effect of
score-lowering work, which is what you would expect of a number that nobody is selecting on (§3).
It is a separate axis, and nothing has touched it.

This changes the problem. Lowering the score is the easy half. The hard half is that every
improvement then has to be paid for by searching for a seed. Method and full numbers:
[`docs/lambda-6909d15.md`](docs/lambda-6909d15.md) for the current head and
[`docs/lambda-measurement.md`](docs/lambda-measurement.md) for the method, with the raw per-trial
data in [`docs/data/`](docs/data/).

**Reproduction.** As of the 2026-08-03 rebase onto upstream `ed4b529`, which is `upstream/main`
`6909d15` and has an identical `src/` tree, the circuit builds locally at
**1,288,101.386 Toffoli × 1,154 qubits = 1,486,468,554**, passing 9,024 of 9,024 shots. Every λ
figure above is measured on that head. Figures elsewhere in `docs/` that are measured on the older
`801dd20` head (λ_total 20.04, about 5.0 × 10⁸ tries per seed at 205 runs per hour) name that head
where they appear.

#### 2. My first conclusion was wrong, and the record of that is kept

The first pass, written against commit `422f21d`, measured 1,320,763 × 1,152 (about 1.52 × 10⁹),
listed eleven possible improvements, found all eleven blocked, and concluded that **no improvement
was available**, and that only a full rewrite at research scale could lower the score.

In the three weeks that followed, upstream landed **25 accepted submissions that lowered the
score**, and none of them was a rewrite. So the conclusion was wrong.

The original claims are kept word for word in [`docs/CONCLUSION.md`](docs/CONCLUSION.md) and marked
where they stand, with a [lever verdict audit](docs/CONCLUSION.md#lever-verdict-audit) that quotes
each claim that is now out of date and says what proved it wrong. **Seven of the eleven verdicts
still hold; four do not.** Lever 11 shows the mistake most clearly. The *measurements* were right,
but I turned them into a hard limit that does not exist. The peak qubit count follows a
configuration cap, and 124 of those qubits come from a borrowed pool that grows to fill whatever
cap is set.

#### 3. How the leaderboard leader actually operates

The leader is a single automated program (`yukon-autoresearch[bot]`) landing a submission every 0.88
days, and it ships its search code inside its own submissions. Reading that code explains the pace
without any clever trick. It uses a **512 / 2,048 / 8,192 / 9,024 shot ladder** that throws out bad
seeds early (7.2× cheaper at λ = 20), plus a cost per try roughly 50× below a plain full run. It was
also searching at a much lower λ for most of the campaign.

That last point is visible in the pace itself. **Eight submissions landed on 2026-07-26**, the same
day `ITERS` moved 258 → 261 and made the circuit more aggressive. **Three landed on 2026-08-01**,
three weeks later, with λ higher. Same program, same method, fewer submissions per day: the shape of
the campaign is a rising-λ ramp, which is §1 showing up as a schedule.

Worth noting: **λ appears nowhere in what it selects on.** It is treated as a pass/fail gate and
never improved. Details: [`docs/upstream-search-economics.md`](docs/upstream-search-economics.md).

#### 4. Measurement traps that quietly ruin results

Several findings are about *method*, and they apply outside this project. The main one: the
benchmark script runs the build under `sudo`, and sudo's `env_reset` **quietly removes the
environment variable** that picks the seed. So every trial measures the default and returns an
identical file. It looks exactly like "my change had no effect". It also comes and goes, because
sudo's saved credentials expire partway through a run.

The only thing that caught it was a rule taken from the other contestant's notes: *a null result is
only a result if the output hash changed.* That rule is used throughout the work here.

#### 5. The cheap ways to prove a gate is dead are the wrong kind of argument

On this fork's head `6909d15`, 46,134 gates, which is 3.35% of the score, fire on **none** of the
9,024 official shots. The bar to beat is 0.886 avgT (avgT is 1,288,101.386, so `round(avgT) ≤
1,288,100` wins). Every count in this section is on that head. The upstream version of this work
(PR #27 below) re-measures them on the certified frontier `cf5aa02`, where the bar is 0.802. Either
way, certifying *one* gate dead would be a submission. The obstacle is §1 again: the test inputs are
hashed from the op stream, so "never fired on this draw" is a certificate that cannot survive its
own use. Three ways to get one that does survive were tried. All three are now closed, and each is
paired with a **positive control**, a case where the answer is known in advance, showing the
instrument detects what it reports absent. Two of those controls caught real defects before the
negative results were believed.

- **Cooling.** Charge a gate on fewer shots instead of making it fire less often. The ceiling is
  real and very large: 76.7% of the score is charge on shots where the gate never fires. It is also
  out of reach. Charging is controlled by a **classical** bit, firing depends on the **quantum**
  controls, and in this circuit the two are independent. The chance that one gate's roughly 2,256
  firing shots all land inside a given fair coin's true shots is **2⁻²²⁵⁶**. The set of candidates
  is empty for a structural reason, not because the search failed.
- **Census sampling.** Certify a gate dead by observing it. Our census says 25% of the shipped dead
  keys fire, in a circuit that demonstrably passes all 9,024 shots, so the census over-observes. The
  mechanism is the finding: a sampler sees *firing*, and has no access to *why* a gate is quiet, so
  a data invariant, something that is always true of the data, stays invisible to it at any depth.
  That also resolves the 25% versus 43% gap left open across three earlier documents.
- **Affine relations over GF(2).** Certify a gate dead by its form, since a `CCX` never fires if its
  two controls are complementary, meaning one is always the opposite of the other. Zero gates
  certified. **Not one gate in the circuit has controls that are affinely related at all**, not
  equal, not complementary, not even sharing a single atom, across 1,338,625 CCX and 1.23M distinct
  nonlinear terms.

The three are one result. All three reason about the **form** of a value, and this circuit computes
modular inversion and modular multiplication, so essentially every value is a nonlinear function of
the inputs and there is no exploitable form left to inspect. The 46,134 gates are quiet
because of *what their controls can be*, not because of how those controls are written or how long
anyone watched them. What would actually prove one dead is an argument about what the binary-GCD
loop **means**, proved for a single divstep and then extended by induction across all 261. That is a
research project on its own, and it is now the only route identified. Nothing was removed:
[`docs/syntactic-certification-is-exhausted.md`](docs/syntactic-certification-is-exhausted.md).

#### 6. Two more conclusions of mine were wrong, and one of them was refuted in twenty minutes

Section 2 records the first pass getting it wrong. The same thing happened twice more on the new
construction, and both are worth reading for the shape of the error rather than the numbers.

**A search cost quoted from too few samples.** λ, the expected number of failing shots per draw,
sets how many nonces you must try to find a valid one, and the cost is `e^λ`. I measured λ from
**eight** draws with a standard deviation of 6.0, exponentiated it, and published "about 2.9 × 10⁸
draws, 25 to 37 days, not viable on this hardware". Measured properly, from sixty draws, λ is
17.717 with a 95% confidence interval of 16.80 to 18.63, which is about **107 hours**, interval 43
to 267. Off by six to eight times, and in the direction that abandons an affordable plan. The
arithmetic was never wrong; taking a point estimate from a small sample and putting it through an
exponential was. The fix cost two minutes of compute.

**A negative result about one method, published as a negative result about the surface.** The
squaring step is the one component upstream flags as never fully optimized, with a specific
algorithm, Toom-3, named as the open lead. I priced Toom-3, found it capped near 1.3% because its
recombination needs expensive reversible divisions, and then wrote that *the square* was bounded.
Twenty minutes later a submission landed that added a second level of Karatsuba to the square and
took 2,650 Toffoli out of it. Karatsuba's recombination is additions and subtractions, with none of
the expensive division that had sunk Toom-3, which is to say the very criterion I used to rule one
option out was the criterion that selected the option I never tried.

The structural model I had built was correct, and it *predicted* the win: the square computes each
partial product and then uncomputes it, so anything that shrinks the core is worth roughly twice
its apparent saving. That argues for attacking the algorithm, not against it. Right model, wrong
conclusion read off it.

Both are corrected in [`docs/pingpong-2026-08-23.md`](docs/pingpong-2026-08-23.md) and in the
retraction note published upstream.

#### 7. I tried to land a submission, and lost to the clock rather than the arithmetic

Worth recording because the failure is structural, not a slip, and it is the thing anyone arriving
with one machine should understand before spending a week on it.

**The mechanism.** The 9,024 test inputs are a hash of the circuit's own op stream, so any change
to the circuit re-rolls them. A submission is therefore two things: an improvement, and a **nonce**,
a 48-bit tweak to an identity tail that re-rolls the draw until one happens to pass all 9,024
shots. Finding that nonce is a lottery, and its cost is set by lambda, the expected number of
failing shots per draw. At the configuration I targeted, lambda was 17.175 (95% CI 15.95 to 18.40),
so roughly **2.9 x 10^7 draws** per clean nonce.

**The attempt.** I found a real improvement: an interleaving checkpoint (`SUB4_PP_R2`) mistuned by
33 rounds, worth 322 executed Toffoli, about **-0.038%**, confirmed on two independent proxies at
unchanged qubit width. Then I built the pipeline to grind a nonce for it and ran it for seven
hours: about 1.8 million draws, 6.2% of the expected search, 413 survivors of the classical
pre-filter, 263 of those confirmed against the real scorer, **none clean**. That is exactly the
expected yield at 6%, so nothing went wrong statistically.

**What went wrong was the clock.** At this machine's throughput a clean nonce averages ~86 hours.
Over that window the leaderboard drifts about 1.14%/day, so the target has to be worth more than
about 5% to survive its own grind. Mine was worth 0.038%. It needed the field to go quiet for days;
the longest stall that day was 6.5 hours, and when it ended the new frontier was already below my
target, so a clean nonce would have been rejected on arrival.

**Why the others can do it.** Their published notes are explicit: H200 GPU pods, coordinated agent
fleets, and staged shot ladders that discard a bad nonce after 256 shots instead of 9,024. That is
two to three orders of magnitude more throughput than one laptop. It is not a cleverness gap, and
no amount of better filtering closes it: the number of draws is set by the circuit, not by the
screener.

**So this fork does not compete on throughput, and says so.** What one machine is good for is
measuring carefully, finding the things that are cheap to check and expensive to assume, and
writing them down. Several of those went back upstream as solver notes, including one improvement
handed over precisely because someone with a fleet can spend it in an hour and I cannot. The
leaderboard will keep moving; the intention here is to keep watching it, keep measuring, and keep
the record of what was learned and what was wrong.

#### Contributed upstream

Three of the findings above went back to the challenge repository instead of staying in this fork.
All three are open at the time of writing:

- **[Layr-Labs/ecdsafail-challenge#23](https://github.com/Layr-Labs/ecdsafail-challenge/issues/23)**:
  the sudo trap from §4. `benchmark.sh` prefers the `sudo` sandbox path, `env_reset` removes the
  environment variable that picks the seed, and the run quietly measures the default while reporting
  success.
- **[PR #27](https://github.com/Layr-Labs/ecdsafail-challenge/pull/27)**: the certification result
  from §5, with its three control tests.
- **[PR #28](https://github.com/Layr-Labs/ecdsafail-challenge/pull/28)**: the λ correction from §1,
  replacing the out-of-date 23.29 in `memory/02-lambda.md` with 20.560 on the current head, plus the
  stability result.

Both pull requests were rebased and scoped on 2026-08-23 to say which construction they measured,
since the circuit has moved on. Three solver notes were also published through the challenge's own
notes system: the schedule-narrowing cliff and the corrected search arithmetic, a configuration trap
plus a small improvement handed over to anyone with the throughput to grind a nonce for it, and the
retraction described in §6.

#### Where this goes from here

This is a log, not a campaign. The benchmark is live and adversarial, the leaderboard moves several
times a day, and §7 is the honest account of what happened when one machine tried to race fleets on
rented datacenter GPUs. I am not going to win that race and am not going to pretend otherwise.

What one machine can do, and what this fork will keep doing when I come back to it:

- **Watch the frontier and read the promotions.** Every accepted submission ships its own source,
  so the diffs say what actually worked. Four lines of a promotion explained more than a day of my
  own speculation did.
- **Measure the things that are cheap to check and expensive to assume.** Most of what is written
  down here cost minutes to establish and would have cost days to guess at wrongly. Several
  measurements closed off ideas that looked large in the score column and were unreachable.
- **Keep the record honest, including the wrong parts.** Sections 2, 6 and 7 are all things I got
  wrong or lost at, kept in place and marked, because a corrected mistake is more use to the next
  reader than a tidy narrative. Two of the retractions were published upstream as well.
- **Hand over what I cannot spend.** One measured improvement went upstream as a solver note
  specifically because someone with throughput can cash it in an hour and I cannot.

If you are reading this months later, the scores will be wrong and quite possibly the whole
construction will have been replaced again. That is expected. The commit each measurement names is
what makes it still readable.

#### Where the cost actually is

About 95% of the budget is the two modular inversions that reversible affine point addition needs.
Across the papers surveyed in
[`docs/quantum-inversion-frontier-research.md`](docs/quantum-inversion-frontier-research.md), no
reversible modular-inversion implementation has a lower Toffoli count than the windowed binary GCD
used here.

Published figures for related work cover different operations or different scopes, so they do not
compare directly to one bare addition. Schrottenloher 2026 reports per-windowed-addition and
full-attack figures, and a windowed addition includes a 2^16-entry lookup table. The Google and
Babbush figures are resource estimates with the circuits held back behind a zero-knowledge proof.
See [`docs/quantum-inversion-frontier-research.md`](docs/quantum-inversion-frontier-research.md)
for the scopes.

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
./benchmark.sh --note "what I tried"
```

That single command builds the circuit, validates it, scores it, and
appends one row to `results.tsv` with timestamp, git commit, Toffoli,
Clifford, qubits, op count, OK/FAIL, and your note. The score is also
written to `score.json` in the format

```json
{ "score": 10704574395, "metrics": { "toffoli": 3942753, "qubits": 2715 } }
```

`benchmark.sh` runs the two binaries in order: `build_circuit` produces
`ops.bin` (sandboxed, since it is the one that compiles in your code from
`src/point_add/`), then `eval_circuit` re-simulates that op stream, checks it,
and writes the score. Any arguments you pass are forwarded to `eval_circuit`,
which is what makes `--note` work. If the build cannot find `cargo` or a C
compiler, run `./setup.sh` first.

Use `./benchmark.sh` rather than a bare `cargo run`. This crate defines two
binaries and no default, so `cargo run --release` cannot pick one and exits with
an error. To drive them by hand, name the binary and keep the order:

```bash
cargo run --release --bin build_circuit    # writes ops.bin (unsandboxed)
cargo run --release --bin eval_circuit -- --note "what I tried"
```

Note that this local layout is what the repository ships. The challenge CLI's
`ecdsafail clone` provisions its own checkout, which may lay the harness out
differently; if the commands above do not match what you get from the CLI,
follow the CLI's copy.

### What you can edit

You may modify **anything inside `src/point_add/`**. Split it into
submodules, rewrite primitives, swap algorithms, refactor freely.

You may **not** touch the harness:

- `src/lib.rs`, `src/circuit.rs`, `src/sim.rs`, and
  `src/weierstrass_elliptic_curve.rs`, which are the contract.
- `src/bin/build_circuit.rs` and `src/bin/eval_circuit.rs`, the two binaries
  `benchmark.sh` drives.
- `Cargo.toml`, `Cargo.lock`, and `rust-toolchain`, so no new dependencies.
- `results.tsv` directly (the harness appends to it for you).
- `benchmark.sh` itself, which decides how the two binaries are run.

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
The reversible circuit itself is the community's work from the challenge repository.
My part was building and checking it locally, measuring where its cost is and what
actually holds progress back, and writing up both the findings and the errors. I did
that work with Claude Code.

Every number quoted from this fork's own measurements can be reproduced from the repo:
the score from `./benchmark.sh`, and the λ figures from the raw per-trial data in
[`docs/data/`](docs/data/). Figures quoted from outside papers were checked against
their sources; see `docs/quantum-inversion-frontier-research.md` for where each one
came from. Where a claim is an inference rather than a measurement, the documents say so.

### What is not this fork's work

Beyond the circuit itself, the 2026-08-02 rebase onto upstream `8af8a6f` first brought in a large
body of another contestant's material, and the 2026-08-03 rebase to the current head brought in a
great deal more. It ships inside `src/point_add/` because that is the challenge's `editablePaths`
root. As of `6909d15` that is **13,909 lines across 50 files**, none of it ours:

- **`src/point_add/memory/01-06` and `README.md`** (776 lines): their working notes on the
  circuit's architecture, its intrinsic error rate, proven floors, traps, the qubit program, and
  the research status of the verifier-centered work. Credited to whoever wrote the accepted
  submissions; the upstream commits are authored by `yukon-autoresearch[bot]`.
- **`src/point_add/memory/repro/`** (39 files, 12,686 lines): their retained working code. This
  includes the Darwin-Gödel-Machine search controller `dgm_search.py` (2,848 lines) and
  `test_dgm_search.py` (507 lines), which port mechanisms from
  [jennyzzt/dgm](https://github.com/jennyzzt/dgm) (pinned at `a565fd2`), as its own header states.
  The 2026-08-03 rebase added 59 files and about 10,000 lines here, covering world models,
  joint-codec synthesis, and verifier-ceiling work, which the earlier description predates.

Our documents cite these heavily and say so at each point. `docs/` is this fork's work.
`src/point_add/` is not.

## Credits

This benchmark harness was adapted from code Google published with
["Securing Elliptic Curve Cryptocurrencies against Quantum Vulnerabilities:
Resource Estimates and Mitigations"](https://research.google/pubs/securing-elliptic-curve-cryptocurrencies-against-quantum-vulnerabilities-resource-estimates-and-mitigations/)
and its [companion Zenodo dataset](https://zenodo.org/records/19597130).
Thanks to the authors for releasing the code that made this benchmark possible.

Thanks to [Kirk Baird](https://github.com/kirk-baird) from SigmaPrime for reviewing the codebase.

The analysis in `docs/` leans heavily on the working notes and search controller that
arrived with the upstream rebase, in `src/point_add/memory/`, which is another
contestant's work, not mine. Several of this fork's findings build on theirs, and the
documents cite them at each point. See
[What is not this fork's work](#what-is-not-this-forks-work).

Project write-up: **[amissah.net](https://amissah.net)**.
