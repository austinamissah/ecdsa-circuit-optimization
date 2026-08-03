# Raw measurement data

Primary data behind [`../lambda-measurement.md`](../lambda-measurement.md). These are the
measurements themselves, not a summary — keep them so the analysis can be re-derived or challenged.

## `lambda-sweep-801dd20.tsv`

The λ sweep on the rebased upstream head `801dd20` (score 1,487,590,242 = 1,289,073.125 executed
Toffoli × 1154 qubits), taken 2026-08-02.

202 trials. Each row is one **full `./benchmark.sh` run** — build plus a 9,024-shot `eval_circuit`.
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
| `avgT` | average executed Toffoli, W=64 harness order — read from `eval_circuit` only |
| `md5` | md5 of the generated `ops.bin` |

`base` = 62000008397024, the nonce baked into the shipped head.

### Integrity properties (check these before trusting any re-analysis)

- **199 distinct md5 values across the 199 non-control nonces.** Equal hashes for distinct nonces
  would mean the tail edit never reached the stream — the failure mode in `04-traps.md` §1. It is
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
