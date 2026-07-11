# How submission to ecdsa.fail works (from the repo)

Read-only recon of what this repository states about submitting to the official
ecdsa.fail platform (as opposed to the local `benchmark.sh` / `cargo run` path).
Sources are quoted with file references. Nothing was installed, run, or submitted
to produce this.

## 1. The submission workflow

The only documented workflow is in **`README.md:80-115`** ("How to play"), using the
**ECDSA Fail CLI** (`ecdsafail`). Order of operations, as written:

```bash
# 1. Install the CLI  (README.md:86)
curl -fsSL https://api.ecdsa.fail/install.sh | sh

# 2. Create an API key from the top-right menu   (README.md:90 — web UI step, not a command)

# 3. Log in  (README.md:93)
ecdsafail login <api-key>

# 4. Clone the benchmark  (README.md:99)
ecdsafail clone

# 5. Improve your circuit   (edit src/point_add/)

# 6. Run and submit  (README.md:106-107)
ecdsafail run
ecdsafail submit
```

There is also a **local-only** path (not a submission): `cargo run --release -- --note "what I tried"`
(`README.md:113`) — it "builds the circuit, validates it, scores it, and appends one
row to `results.tsv` … writes `score.json`". This is the local harness, equivalent to
`benchmark.sh`; it does **not** talk to the platform.

## 2. Authentication

- **Mechanism:** an **API key**, passed to `ecdsafail login`:
  > `ecdsafail login <api-key>` — `README.md:93`

  (`<api-key>` is the README's own literal placeholder — you substitute your real key
  when you type the command.)
- **Where the key comes from:** created in the web UI —
  > "Create an API key from the top-right menu." — `README.md:90`
- **Where the credential is stored / how it is sent on the wire:** **the repo does not say.**
  The `ecdsafail` CLI is not part of this repo (it is installed separately via the curl
  script), so there is no config-path, environment-variable, HTTP-header, or token-file
  reference anywhere in the tree. A search for `.ecdsa`, `api-key`, `token`,
  `Authorization`, `Bearer`, `X-API`, and home-directory config paths returns only the
  five README command lines above. So from the repo alone you **cannot** tell whether
  `ecdsafail login` writes the key to a config directory, a dotfile, an OS keyring, or an
  environment variable. That must be checked from the CLI itself once installed (e.g.
  `ecdsafail login --help`, or by reading `install.sh`).

## 3. How the CLI is installed

It **is a curl-pipe-to-shell**:
> `curl -fsSL https://api.ecdsa.fail/install.sh | sh` — `README.md:86`

The script at `https://api.ecdsa.fail/install.sh` is **not vendored in this repo**, so it
cannot be read here — fetch and read it before piping it to `sh`. The README also adds a
general caution:
> "Benchmarks are run in hardened processes and we recommend using caution when running." — `README.md:148`

## 4. What artifact gets submitted, and where scoring happens

The CLI is not in the repo, so `submit`'s exact payload is not documented in prose — but
the **git history is strongly indicative**. Every accepted submission appears as a commit:

```
422f21d Accept submission 39e28ee8-7c15-47c0-8171-7c75522c8a57
d44cad3 Accept submission 71f51157-...
...
```

Inspecting one (`git show --stat 422f21d`), authored by a bot identity
**`Yukon <yukon@example.invalid>`**, it touches **only files under `src/point_add/`**
(`arith/`, `rounds/dialog/`, `trailmix_ludicrous/`, `mod.rs`, `venting.rs`, `emit.rs`,
`single_ccx_fanout.rs`). It touches **none** of the harness (`circuit.rs`, `sim.rs`,
`weierstrass_elliptic_curve.rs`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain`).

That matches the editable/frozen contract in **`README.md:126-136`**:
> "You may modify **anything inside `src/point_add/`** … You may **not** touch the harness:
> `src/main.rs`, `src/circuit.rs`, `src/sim.rs`, `src/weierstrass_elliptic_curve.rs` …
> `Cargo.toml`, `Cargo.lock`, `rust-toolchain` … `results.tsv`."

**Inference (flagged as inference, not stated):** the submitted artifact is your
**`src/point_add/` source**, and the platform **rebuilds and scores server-side** —
because `ops.bin` and `score.*` are **gitignored** (`.gitignore:5-7`: `score.*`,
`ops.bin`) and never appear in the accept-submission commits. So the local
`ops.bin`/`score.json` are clearly *not* what gets uploaded; the server reconstructs them
from source. The README does not literally state "server-side rebuild," so treat this as a
well-supported inference to confirm on the site.

## 5. Account / registration step

Partially implied, not spelled out. Step 2 — "Create an API key from the **top-right menu**"
(`README.md:90`) — presupposes you already have a **web account logged into ecdsa.fail**
(there is a UI with a top-right menu). The repo describes **no sign-up/registration flow**
at all. So there is an out-of-CLI web step (log in / reach the menu to mint the key), but
how you create the account itself is not covered by the repo — check ecdsa.fail directly.

## What the repo does NOT make clear (verify on ecdsa.fail / the CLI)

1. **Where `ecdsafail login` stores the API key** (config file path, environment variable,
   or keyring) — nothing in-repo. Check `ecdsafail login --help` or `install.sh`.
2. **The exact contents of `install.sh`** — not vendored; read it before running the pipe.
3. **`run` vs `submit` semantics** — whether `ecdsafail run` scores locally and `submit`
   uploads, what `submit`'s payload is, and whether it requires a clean/committed tree.
   Only inferable, not stated.
4. **Server-side rebuild + scoring** — strongly inferred from gitignored artifacts plus
   source-only accept commits, but not explicitly documented.
5. **Account registration** — the web sign-up step preceding "create an API key" is not
   described.
6. **Minor doc drift (not a blocker):** the README refers to `src/main.rs` and
   `cargo run --release -- --note` (`README.md:113,131`), but this repo has **no
   `src/main.rs`** — the actual binaries are `src/bin/build_circuit.rs` and
   `src/bin/eval_circuit.rs` (`Cargo.toml:9-15`), driven by `benchmark.sh`. So the
   README's local-run command may not work as written; the CLI's `ecdsafail clone`
   presumably provisions whatever layout it expects, which could differ from this checkout.
