# Where the Toffolis are spent — `src/point_add/` recon

**Nothing was modified, built, or run.** All findings are from reading source. The two headline numbers — ~1,398,187 emitted CCX and 1,320,763 avg *executed* Toffolis — differ because executed count only tallies CCX/CCZ whose condition stack is active per shot (`sim.rs:82-86`); emitted is the static gate count.

## 0. The live path (important framing)

```
build()                                   src/point_add/mod.rs:1656
  → configure_q1153_second512_submission_defaults()   (sets ~100 TLM_* env defaults)
  → trailmix_ludicrous::build_trailmix_ludicrous_ops()   mod.rs:1915  ← THE CIRCUIT
       → B::new(); ec_add::ec_add(...)          trailmix_ludicrous/mod.rs:359,369
       → constprop::run(...)                    (classical gate-cancellation pass)
  → single_ccx_fanout::rewrite_first_target_fanout(...) loop   mod.rs:1930  ← post-pass
```

Two big sibling trees are **dead relative to the live circuit**:

- `src/point_add/rounds/dialog/` (mod/compressed/config, ~9,700 lines) — reached only by `build_builder()` (mod.rs:1464, calls `configure_ecdsafail_submission_route`), which `build()` never calls, plus env-gated `*_SELFTEST` diagnostics. The `DIALOG_GCD_ACTIVE_ITERATIONS=258` etc. configure this dead engine; the live 258 is the unrelated `ITERS` constant in `schedule.rs:4` (they coincide).
- `src/point_add/arith/` — mostly dead **except `multiply.rs`**, whose `square_addsub_vented` is the live squaring backend (called from `square.rs:103`). `modular.rs`/`adder.rs`/`compare.rs`/`const_arith.rs` are superseded by trailmix's own `arith.rs`/`comparator.rs`/`gidney.rs`.

## 1. Call hierarchy — one point addition

`ec_add()` (`trailmix_ludicrous/ec_add.rs:275`) is a straight-line 8-phase sequence computing the affine point-add (dx = x2−Qx; λ = dy/dx; etc.), each phase tagged with `set_phase`:

| # | phase tag | routine | leaf / algorithm |
|---|-----------|---------|------------------|
| 1 | `tlm_coord_x_sub` | `coord_addsub` | `mod_sub_vented` |
| 2 | `tlm_coord_y_sub` | `coord_addsub` | `mod_sub_vented` |
| 3 | **`tlm_inverse`** | `mod_mul_inverse_in_place(Inverse)` | **binary-GCD** fwd+rev sweep |
| 4 | `tlm_coord_add3x` | `coord_add3x` | classical ×3 mod q + `mod_add` |
| 5 | **`tlm_square`** | `mod_square_sub_pm_secp256k1_symmetric` | **symmetric partial-product square** |
| 6 | **`tlm_forward_multiply`** | `mod_mul_inverse_in_place(Forward)` | **binary-GCD** fwd+rev sweep |
| 7 | `tlm_coord_y_sub_final` | `coord_addsub` | `mod_sub_vented` |
| 8 | `tlm_coord_rsub_final` | `coord_rsub` | `mod_rsub_vented_loaded` |

**Key structural fact:** modular **inversion and modular multiply are the same routine** — `mod_mul_inverse_in_place` (`gcd.rs:1413`) run with `Direction::Inverse` vs `Direction::Forward`. Each call does a `forward_gcd_jump` + `reverse_gcd_jump` (`gcd.rs:715`, `:962`), each looping `ITERS = 258` times. So **4 full GCD sweeps = 1,032 GCD jump-steps per point addition** (matching the length-1032 `GCD_SUB_K`/`GCD_BRANCH` schedules).

## 2. What each major sub-routine does

- **Binary-GCD modular divide (`gcd.rs`)** — a windowed (JUMP=2) extended-binary-GCD / Kaliski-style modular inverse: per iteration, a conditional right-shift, a truncated `v<u` compare-and-swap (`comparator.rs`), and a controlled modular subtract (`apply_step_*` → `controlled_mod_sub_vented`). The transform history is compressed onto a *tape* by `codec.rs::DialogCodec`. `Direction::Forward` **replays** the recorded GCD transform onto `y` to realize the field multiply λ·dx — there is no schoolbook multiplier; multiply *is* a replayed GCD. Driven entirely by pre-baked per-step schedules in `schedule.rs`.
- **Modular squaring (`square.rs` → `arith/multiply.rs`)** — `symmetric_square_into_prod` computes the n(n−1)/2 cross terms with add/sub cancellation (`square_addsub_vented`), then secp256k1 fast reduction (p = 2^256−2^32−977) via shifted-F windows.
- **Modular add/sub (`arith.rs`)** — `mod_add`/`mod_sub_vented`/`mod_rsub_vented_loaded`: conditional-add-of-p over vented Cuccaro carry chains; coordinate constants folded in classically.
- **Comparator (`comparator.rs`)** — truncated top-k chunked ripple `≥` (schedule `CMP_K`) producing the GCD swap-decision bit.
- **Adder / MCX primitives (`gidney.rs`, `mcx.rs`)** — Gidney measurement-based / Cuccaro-hybrid controlled adders with vent (measure-and-reset) carry management, and Khattar–Gidney log-depth multi-controlled-X / increment. **These are the innermost Toffoli producers**, invoked inside every controlled add and every partial product.
- **`fused.rs`** — fuses the GCD apply-step's fold + shift into one carry sweep (schedule `FOLD_SCHED`).
- **`constprop.rs`** — post-emission classical optimizer (prunes dead controls, inverse pairs); reports Toffolis *removed*, not per-routine.

## 3. Invocations per point addition

- Modular inversion: **1×**; modular multiply: **1×** → **4 GCD sweeps / 1,032 jump-steps**.
- Comparator: ~**1,032×** (once per GCD step; `CMP_K` has 1,032 entries).
- Controlled mod add/sub inside GCD (`apply_step_*`): ~**1,032×**.
- Gidney/Cuccaro + MCX primitives: **thousands** (nested inside every controlled add and every squaring partial product) — the dominant Toffoli source.
- Modular squaring: **1×** (internally ~256·255/2 ≈ 32,640 partial-product terms).
- Coordinate mod add/sub: **5×** (phases 1,2,4,7,8).
- `TLM_FFG_MAX_G=47` caps the fast-fanout-Gidney window in `add_f_window_hybrid` (`arith.rs:1300`), live.

**Expectation:** the 4 GCD sweeps (inversion + multiply) should dominate the Toffoli budget, squaring second, coordinate add/sub a thin slice. Measurement should confirm this.

## 4. Instrumentation — the answer to "can we already get a per-routine breakdown?"

**Yes, and it's on the live path: `TRACE_TLM_CCX=1`.** In `build_trailmix_ludicrous_ops` (`trailmix_ludicrous/mod.rs:465-490`) this walks `circ.phase_transitions` (the op-index boundaries laid down by every `set_phase` call), slices `circ.ops` per phase, counts `kind==CCX`, and prints a **ranked table with per-phase count, %, and cumulative %** (top 30), then `TLM_CCX_TOTAL`:

```
TLM_CCX phase=<name> ccx=<n> pct=<..> cum=<..>
TLM_CCX_TOTAL <grand> phases=<k>
```

That is exactly the per-sub-routine Toffoli map you want, keyed by the `tlm_inverse` / `tlm_forward_multiply` / `tlm_square` / `tlm_coord_*` phase names above.

Supporting/adjacent hooks:

- `B::counted_phase_rows: Vec<PhaseResource>` (`mod.rs:106`) — already stores per-phase `toffoli_ops`/`ccx_ops`/`ccz_ops`, but only populated in `count_only` builds and never printed.
- `TRACE_TLM_PROFILE` / `TRACE_PHASE_ACTIVE` — per-phase *active-qubit* maxima (the width axis of the score).
- `TRACE_OP_SITES` — attributes each op to its emitting `file:line` (finer than phase).
- `CONSTPROP …` lines report Toffolis *removed* globally; `SINGLE_CCX_FANOUT: SUMMARY` reports whole-circuit op deltas. Neither attributes per routine.
- **Not on the live path:** `TRACE_PHASES` (the other per-phase report) lives in `build_builder()`, which drives the dead dialog engine — it would report nothing for the real circuit. Don't use it.

**Two caveats about `TRACE_TLM_CCX`:**

1. It counts **`CCX` only** (kind 13), excluding `CCZ` (kind 14) — the executed Toffoli metric counts both. If the circuit emits CCZ, add it for parity.
2. It runs on the **pre-`constprop`, pre-fanout** op stream, so its grand total will exceed the final 1,398,187 emitted / 1,320,763 executed. It's the right *relative* attribution map, but not the post-optimization absolute count. There is no existing hook that attributes the *final executed* count per routine (`SimStats` is two global scalars, `sim.rs:8-10`).

## 5. Surprises / opportunities (flagged, not implemented)

1. **Multiply and inversion share one GCD engine, run 4× total.** Any Toffoli saved per GCD jump-step is multiplied by ~1,032. This is almost certainly where >70% of the budget lives — the single highest-leverage target. Optimize the GCD inner loop (comparator + controlled mod-sub + Gidney adder) before anything else.
2. **Multiply-as-replayed-GCD is unusual.** A dedicated windowed modular multiplier *might* be cheaper than replaying a full 258-step GCD transform for `tlm_forward_multiply` — worth measuring the multiply phase's share to see if a purpose-built multiplier could undercut it.
3. **Emitted (1.40M) vs executed (1.32M) gap ≈ 78k Toffolis** are emitted but not executed on average (condition-gated). Worth checking whether any phase emits many never-executed CCX that could be dropped outright.
4. **Dead code is large** (`rounds/dialog` ~9.7k lines + most of `arith/`). Not a scoring factor, but it makes the live path hard to see and risks editing the wrong module — the trap here is that `arith/modular.rs`, `arith/adder.rs`, and the whole `dialog` tree *look* like the arithmetic but aren't live.
5. **The circuit is schedule-driven** (`schedule.rs` bakes `SCHED_J2`, `GCD_SUB_K`, `CMP_K`, `FOLD_SCHED`, `FFG_G`…). Much "optimization" here is retuning these tables, not rewriting logic — cheap to experiment with, but means correctness depends on the built-in self-tests (`TLM_SQ_SELFTEST`, `*_SELFTEST`).

## Recommended next step (to actually measure)

Run the builder once with `TRACE_TLM_CCX=1` (add `CCZ` to the count if any appears) to get the ranked per-phase CCX table, then collapse the phase names into four buckets — inversion, multiply, square, coord add/sub — to get the headline breakdown. That's the smallest read-only-adjacent measurement; it needs only setting an env var when invoking the build binary, no source change. If you later want the breakdown on the *final* (post-constprop/fanout, executed) circuit, that does require a code change — the natural one is emitting durable `DebugPrint` marker ops (kind 17, which the evaluator already preserves) at `set_phase` boundaries and segmenting the simulator's executed count between them.

## Measurement: TRACE_TLM_CCX per-phase breakdown

Ran `TRACE_TLM_CCX=1 ./target/release/build_circuit` (the same binary the benchmark builds; `ops.bin` rewritten as a side effect). Results below.

### CCZ check — TRACE_TLM_CCX undercounts

The circuit emits **both** CCX and CCZ, so `TRACE_TLM_CCX` (which counts kind 13 = CCX only) undercounts the true Toffoli-class total. Live `c.ccz(...)` emission sites: `trailmix_ludicrous/gidney.rs:1321,1380` (carry-erase steps) and `trailmix_ludicrous/arith.rs:624,663,1115`. Counting op kinds directly from the final `ops.bin` (9,784,075 records):

| kind | | count |
|---|---|---|
| 13 | CCX | 1,397,851 |
| 14 | CCZ | 5,341 |
| | **CCX + CCZ (true Toffoli-class)** | **1,403,192** |

CCZ is **5,341 gates ≈ 0.38%** of the Toffoli-class total — invisible to the phase table. Those CCZ live inside the Gidney adders (inversion/multiply) and `arith.rs` (square), i.e. the same routines, just uncounted. (The simulator's *executed* Toffoli metric counts CCX and CCZ together, `sim.rs:164`.)

### Raw TLM_CCX phase table

`TLM_CCX_TOTAL = 1,398,456` CCX across **50 phases** (pre-`constprop`; `constprop` trims to 1,398,187, the fanout pass lands `ops.bin` at 1,397,851 CCX). The flag prints only the **top 30**, covering 99.67%:

```
tlm_apply_inverse_mod_sub_register   182776  13.07%   (cum 13.07)
tlm_apply_forward_mod_add_register   182715  13.07%   (cum 26.14)
tlm_multiply_gcd_reverse_body        162503  11.62%   (cum 37.76)
tlm_inverse_gcd_reverse_body         150155  10.74%   (cum 48.49)
tlm_apply_inverse_fold                82167   5.88%   (cum 54.37)
tlm_apply_forward_fold                81933   5.86%   (cum 60.23)
tlm_inverse_gcd_forward_body          80703   5.77%   (cum 66.00)
tlm_multiply_gcd_forward_body         69368   4.96%   (cum 70.96)
tlm_apply_inverse_swap                65792   4.70%   (cum 75.66)
tlm_apply_forward_swap                65536   4.69%   (cum 80.35)
tlm_inverse_gcd_forward_compare       46331   3.31%   (cum 83.66)
tlm_multiply_gcd_forward_compare      45516   3.25%   (cum 86.92)
tlm_inverse_gcd_forward_shift         35175   2.52%   (cum 89.43)
tlm_multiply_gcd_forward_shift        35174   2.52%   (cum 91.95)
tlm_apply_forward_mod_add_fold        14975   1.07%   (cum 93.02)
tlm_apply_inverse_mod_sub_fold        14722   1.05%   (cum 94.07)
square_c_sum_build                     9540   0.68%   (cum 94.75)
square_c_sum_unbuild                   9540   0.68%   (cum 95.44)
square_a_lo_build                      9402   0.67%   (cum 96.11)
square_a_lo_unbuild                    9402   0.67%   (cum 96.78)
square_b_hi_build                      9402   0.67%   (cum 97.45)
square_b_hi_unbuild                    9402   0.67%   (cum 98.12)
tlm_apply_forward_mod_add_clean        4902   0.35%   (cum 98.48)
tlm_apply_inverse_mod_sub_clean        4901   0.35%   (cum 98.83)
square_b_hi_apply_f_times_sub          4329   0.31%   (cum 99.14)
tlm_inverse_gcd_reverse_decode         1704   0.12%   (cum 99.26)
tlm_multiply_gcd_reverse_decode        1704   0.12%   (cum 99.38)
tlm_inverse_gcd_forward_codec          1533   0.11%   (cum 99.49)
tlm_multiply_gcd_forward_codec         1533   0.11%   (cum 99.60)
square_c_sum_apply_shifted_128_sub     1064   0.08%   (cum 99.67)
TLM_CCX_TOTAL 1398456 phases=50
```

**Note:** the printed phase names are *finer* than the four top-level `ec_add` tags. The GCD engine re-tags sub-phases as it runs, so `tlm_inverse` appears as `tlm_inverse_gcd_*` + `tlm_apply_inverse_*`, and `tlm_forward_multiply` appears as `tlm_multiply_gcd_*` + `tlm_apply_forward_*`. A literal prefix match on the four tags would match almost nothing; the collapse below is semantic (which `mod_mul_inverse_in_place` call each sub-phase belongs to).

### Four-bucket collapse

| bucket | sub-phases | CCX | % of TLM_CCX_TOTAL |
|---|---|---|---|
| **Inversion** (`tlm_inverse`) | `tlm_inverse_gcd_*` + `tlm_apply_inverse_*` | 665,959 | 47.6% |
| **Multiply** (`tlm_forward_multiply`) | `tlm_multiply_gcd_*` + `tlm_apply_forward_*` | 665,859 | 47.6% |
| **Square** (`tlm_square`) | `square_*` | 62,081+ | 4.4%+ |
| **Coordinate add/sub** (`tlm_coord_*`) | `tlm_coord_*` | ~0 shown | <0.3% |

Inversion and multiply are near-identical (same GCD engine run twice), each ~47.6%; together the two GCD passes are **~95.2%** of all Toffolis. Square is ~4.4%. **No `tlm_coord_*` phase appears in the top 30** — coordinate add/sub is entirely inside the unprinted 0.33% tail.

### Do the buckets sum to ~100%?

**Yes — effectively 100%.** The top-30 phases (99.67%, 1,393,899 CCX) split cleanly across only three of the four buckets (inversion, multiply, square); there is no `OTHER` bucket. The unprinted tail is just **4,557 CCX (0.33%)** across 20 phases, and by phase-name family those are residual `square_*` sub-phases plus the five `tlm_coord_*` phases — still the same four routines. Nothing meaningful is spent outside them.

Caveats (neither changes the conclusion): the 0.33% tail can't be split square-vs-coordinate exactly because `TRACE_TLM_CCX` hard-caps at top 30 (the remaining 20 phases would need a source change to print); and the 5,341 CCZ gates (~0.38% of Toffoli-class) sit outside this table but belong to the same buckets.

**Bottom line:** the Toffoli budget is ~95% the two GCD passes (inversion + multiply, ~666k each), ~4.5% squaring, <0.4% everything else. The GCD inner loop holds essentially all optimization leverage.
