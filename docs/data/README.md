# Raw measurement data

Primary data behind [`../lambda-measurement.md`](../lambda-measurement.md). These are the
measurements themselves, not a summary. Keep them so the analysis can be re-derived or challenged.

## New-baseline arms (2026-08-03, after the `ed4b529` rebase)

| file | contents |
|---|---|
| `arms-newbase-2026-08-03.tsv` | the six full-harness arms behind [`../rebase-2026-08-03-upstream-ed4b529.md`](../rebase-2026-08-03-upstream-ed4b529.md): label, strip, delta, ops, peak q, avgT, score, classical/phase/ancilla, and a distinct `md5 ops.bin` per arm |

The per-gate hotness dump behind [`../gate-hotness-census.md`](../gate-hotness-census.md) is
**deliberately not checked in**: 1,343,361 rows, ~11 MB compressed, and 53 s to rebuild:

```bash
mkdir -p examples && cp tools/census/hotness.rs examples/
cargo build --release --offline --example hotness
./target/release/examples/hotness /tmp/head      # writes /tmp/head.hot.tsv
rm -rf examples          # transient; do not commit it
```

Check the `GATE ok` line and that the printed `avgT` matches `eval_circuit` before using the dump.

## Stream-provenance dumps

Behind [`../census-stream-provenance.md`](../census-stream-provenance.md), taken 2026-08-03. All
four are produced by [`../../tools/census/dump_gates.rs`](../../tools/census/dump_gates.rs) under
`SUB4_APPLY_STRIP=0`.

| file | contents |
|---|---|
| `census-vs-head.gates.diff.gz` | the 354-hunk unified diff between the census and head gate streams, net −978 CCX |
| `stream-walk-by-commit.tsv` | ops / gates / distinct tuples at each of the 18 commits from `d9ef3e9` to HEAD |

**Three SHAs in `stream-walk-by-commit.tsv` no longer resolve.** The 14 upstream commits do, but
this fork's three were rewritten by the 2026-08-03 rebase onto `ed4b529`
([`../rebase-2026-08-03-upstream-ed4b529.md`](../rebase-2026-08-03-upstream-ed4b529.md)), which
gave every commit above `upstream/main` a new hash. The file records the hashes as they were when
the walk ran, which is why they are left alone. Current equivalents, matched by subject:

| in the TSV | now | subject |
|---|---|---|
| `b1c8f84` | `36bea26` | Lift the ITERS cap: `SCHED_J2`/`GAP_J2` hold their terminal entry past the end |
| `9f34bb9` | `38853ea` | `TLM_SCHED_J2_DELTA=2`: λ_classical 15.34 → 5.79 |
| `7d844fa` | `37c33b8` | harness-order mode, and why the hypothesis is now doubtful |

The measurements themselves are unaffected: the walk rebuilt each commit and recorded its stream,
and those trees are unchanged by a rehash.

The two full gate dumps the diff was taken from are **deliberately not checked in**, being
~12 MB each compressed and about a minute apiece to rebuild. Regenerate them with the commands
below rather than carrying 24 MB of derived data in the tree.

### Regenerating the gate dumps

The instrument is [`../../tools/census/dump_gates.rs`](../../tools/census/dump_gates.rs), a cargo
*example* so it can be dropped into any historical checkout without touching `Cargo.toml`.

```bash
# --- head (9,070,297 ops / 1,360,635 CCX+CCZ) ---
mkdir -p examples && cp tools/census/dump_gates.rs examples/
cargo build --release --offline --example dump_gates
SUB4_APPLY_STRIP=0 ./target/release/examples/dump_gates /tmp/head
rm -rf examples          # transient; do not commit it

# --- the census stream (9,073,163 / 1,361,613) ---
# d9ef3e9 predates the tool, so the example is copied in from THIS checkout.
git worktree add /tmp/wt-census d9ef3e9 --detach
mkdir -p /tmp/wt-census/examples
cp tools/census/dump_gates.rs /tmp/wt-census/examples/
( cd /tmp/wt-census \
  && cargo build --release --offline --example dump_gates \
  && SUB4_APPLY_STRIP=0 ./target/release/examples/dump_gates /tmp/census )
git worktree remove --force /tmp/wt-census

# --- rebuild census-vs-head.gates.diff (drop the ordinal/occupancy columns:
#     they shift globally, so diffing them buries the structural change) ---
cut -f2-6 /tmp/census.gates.tsv > /tmp/census.gt
cut -f2-6 /tmp/head.gates.tsv   > /tmp/head.gt
git diff --no-index --unified=0 --minimal /tmp/census.gt /tmp/head.gt > /tmp/census-vs-head.gates.diff
```

`SUB4_APPLY_STRIP=0` is load-bearing: the census sees the **unstripped** stream. Passing `-` as the
output prefix prints the summary counts only and skips the ~950 MB of files, which is the form the
18-commit stream walk uses.

**This recipe is verified, not assumed.** Run verbatim on 2026-08-03 it rebuilds 9,070,297 /
1,360,635 and 9,073,163 / 1,361,613, and the resulting diff is byte-identical to the committed
`census-vs-head.gates.diff.gz` in its body: the git blob hashes in the two hunk headers agree, so
the intermediate `.gt` files match as well. Only the `a/`,`b/` paths differ, since the committed
copy was taken from a scratch directory.

Columns in each gate dump: `opidx, kind, q_control2, q_control1, q_target, c_condition, ordinal,
tuple_occupancy`, the last three keyed exactly as `apply_deep_strip_identity` expects.

**Integrity gate.** Replaying the occupancy tripwire against the regenerated head dump must
reproduce `build_circuit` exactly: 12,292 dead accepted / 251 stale, 3,923 downgrades / 0 stale. If
a re-derivation does not reproduce those four numbers it is not reading the shipped stream. The
census dump's own gate is its size: 1,361,613 gates, matching `deep_strip_keys.rs`'s header.

## `lambda-sweep-6909d15.tsv`

The λ sweep on **`upstream/main` `6909d15`** (`src/` identical to accepted submission `ed4b529`;
score 1,486,468,554 = 1,288,101.386 executed Toffoli × 1154 qubits), taken 2026-08-04. Behind
[`../lambda-6909d15.md`](../lambda-6909d15.md).

Same 202-trial design and same columns as `lambda-sweep-801dd20.tsv` below, so the two heads are
directly comparable. Two differences, both deliberate:

- `base` = **200321420125**, the nonce *this* head bakes in at `src/point_add/mod.rs:2384`. It is
  not `801dd20`'s `62000008397024`. The positive control must be the head's own shipped nonce;
  using the other head's would have failed the control and voided the sweep.
- 6 workers, not 14, because the machine loses its desktop session under a 14-worker load. Cost only 11%
  of throughput (183 vs 205 trials/hour).

### Integrity properties (check these before trusting any re-analysis)

- **199 distinct md5 values across the 199 non-control nonces.** Equal hashes for distinct nonces
  mean the tail edit never reached the stream: issue #23 / `04-traps.md` §1.
- **3 control rows**, all `nonce = 200321420125`, all `0/0/0`, all md5
  `ef30945f3afcb369192ea32897232d2f`, matching upstream's shipped artifact. A dirty control voids
  the sweep.
- 0 parse failures; `ancilla` is 0 in every row.

## `lambda-sweep-6909d15-nonces.tsv`

The trial list as generated (`block`, `nonce`), in construction order, sharded round-robin across
workers, so it does not match row order in the results file.

## `lambda-sweep-driver-6909d15.sh`

The driver that produced it, adapted from `lambda-sweep-driver.sh`. The two load-bearing details
described under that file apply here unchanged (the `sudo` shim forcing the `setpriv` path, and
per-worker builds so `eval_circuit` does not race one `results.tsv`). Worker isolation was verified
empirically before launch rather than assumed: a trial run in `w01` grew `w01/results.tsv` and left
`w00`'s untouched.

## `lambda-sweep-801dd20.tsv`

The λ sweep on the rebased upstream head `801dd20` (score 1,487,590,242 = 1,289,073.125 executed
Toffoli × 1154 qubits), taken 2026-08-02.

202 trials. Each row is one **full `./benchmark.sh` run**, meaning a build plus a 9,024-shot `eval_circuit`.
No custom screen was used, deliberately: `src/point_add/memory/04-traps.md` §4 documents a
lazy-XOF screening bug that reported false clean results and cost its author a 1,344-vCPU grind.

| column | meaning |
|---|---|
| `block` | `A` = contiguous nonces `base+0..99`; `B` = spread at 2^40 stride; `CTRL` = control repeat |
| `nonce` | the `SUB4_TAIL_NONCE` value (`src/point_add/mod.rs:2384`) |
| `exit` | `benchmark.sh` exit status (`1` = correctness failed, expected for a dirty seed) |
| `classical` | classical mismatches out of 9,024 shots |
| `phase` | phase-garbage batches |
| `ancilla` | ancilla-garbage batches (identically 0; guaranteed by construction, see `02-lambda.md`) |
| `avgT` | average executed Toffoli, W=64 harness order, read from `eval_circuit` only |
| `md5` | md5 of the generated `ops.bin` |

`base` = 62000008397024, the nonce baked into the shipped head.

### Integrity properties (check these before trusting any re-analysis)

- **199 distinct md5 values across the 199 non-control nonces.** Equal hashes for distinct nonces
  would mean the tail edit never reached the stream, the failure mode in `04-traps.md` §1. It is
  what caught the sudo/`env_reset` problem described in `../lambda-measurement.md`.
- **3 control rows**, all `nonce = 62000008397024`, all `0/0/0`, all md5 `f5c5f98258ddb7a0b1f250750ad1c6d2`,
  matching the shipped artifact exactly. If a control row is dirty the whole sweep is void.
- 0 parse failures; `ancilla` is 0 in every row.

## `lambda-sweep-801dd20-nonces.tsv`

The trial list as generated (`block`, `nonce`), in construction order. The sweep driver shards this
round-robin across workers, so it does not match row order in the results file.

## `lambda-sweep-driver.sh`

The driver that produced the results. Two things in it are load-bearing rather than incidental:

1. It forces `benchmark.sh` onto its `setpriv --no-new-privs bwrap` path with a `sudo` shim that
   exits non-zero. `benchmark.sh` prefers `sudo -n bwrap`, and sudo's `env_reset` strips
   `SUB4_TAIL_NONCE` before `build_circuit` ever sees it. Same bwrap flags either way, and the
   control artifact is byte-identical.
2. It gives each worker its own repo copy with its own build, because `eval_circuit` writes
   `results.tsv` to the `CARGO_MANIFEST_DIR` baked in at compile time. Sharing one build would race
   14 workers onto one file.

Re-running it needs the 14 worker trees; it is recorded for method, not as a turnkey script.
