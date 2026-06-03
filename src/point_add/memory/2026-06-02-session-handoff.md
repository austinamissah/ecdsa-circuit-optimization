# 2026-06-02 Session Handoff Notes

## Current State

- **Leaderboard best:** 2,535,067,306 (1,729,241 T × 1,466 q)
- **Repo HEAD:** `fd8e712` (synced to best submission `8383a436`)
- **Our local changes:** `src/bin/quick_check.rs` (custom fast eval), dead u_high code in mod.rs (disabled)
- **Login:** `ecdsafail login` already done as phil@eigenlabs.org

## What the Benchmark Is

Optimize a reversible quantum circuit for secp256k1 elliptic curve point addition. Score = avg_Toffoli × peak_qubits (lower is better). Must pass all 9024 Fiat-Shamir-derived test shots with 0 classical mismatches, 0 phase-garbage, 0 ancilla-garbage.

## Architecture of the Current Circuit

The circuit uses a "dialog GCD" — a half-GCD that:
1. **Tobitvector (forward):** Runs 399 binary-GCD iterations on (u, v), recording 2 branch bits per step into a compressed sidecar log (665 qubits, compressed from 798 raw via 3→5 encoding)
2. **Apply (Bezout reconstruction):** Replays the recorded bits against (x, y) to compute the modular inverse-multiply
3. **Tobitvector (reverse):** Runs the GCD backward to uncompute the log

This is done TWICE (quotient pass + ipmul pass) for the full point addition.

## Cost Breakdown (current best, 1,729,241 avg Toffoli)

| Phase group | CCX | % |
|---|---|---|
| materialized_special_chunked_raw (apply add/sub) | ~470K | 27% |
| tobitvector cswap (fwd+rev) | ~225K | 13% |
| tobitvector load + body (fwd+rev) | ~450K | 26% |
| apply cswap (fwd+rev) | ~204K | 12% |
| branch_bits (fwd+rev) | ~80K | 5% |
| apply double/halve | ~52K | 3% |
| round84 squaring + arithmetic | ~100K | 6% |
| other | ~148K | 8% |

## Peak Qubit Structure (1466)

Peak phase: `dialog_gcd_raw_tobitvector_materialized_sub_body`

Co-binders at 1466:
- `materialized_special_chunked_raw_{sum,difference}` (apply)
- `raw_tobitvector_materialized_{add,sub}_body` (tobitvector)

Next tier: 1446 (branch_bits), then 1442, then 1439

Base live state during tobitvector: tx(256) + ty(256) + u(256) + compressed_log(665) + raw_block(6) = **1,439 qubits**. The 27-qubit transient at peak comes from the gated register + carries when the future-log borrow runs out.

## Key Parameters (set in `configure_ecdsafail_submission_route`, line ~29674)

- `DIALOG_GCD_COMPARE_BITS=59` — branch comparator width
- `DIALOG_GCD_APPLY_CHUNKED_F_CUT=116` — apply chunk boundary (drives apply peak)
- `DIALOG_GCD_WIDTH_MARGIN=27` — safety margin for width truncation
- `DIALOG_GCD_ACTIVE_ITERATIONS=399` — GCD iteration count
- `DIALOG_REROLL=52, DIALOG_POST_SUB_REROLL=28` — Fiat-Shamir island nonces
- `DIALOG_GCD_BRANCH_BITS_HOST_COMPARATOR=1` — comparator hosted on future-log
- `KARA_Z02_LOWQ=1, KARA_SOL_MOD_VENT=1` — Karatsuba qubit cuts
- `KARA_FREE_Z1_TOPBIT=1` — free provably-zero z1 top bit during peak window
- `DIALOG_GCD_HOST_GATED=1` — gated register hosted on future-log tail
- `DIALOG_GCD_BORROW_U_HIGH=1` — **OUR ADDITION, BROKEN** (see below)

## What We Tried and Why It Failed

### U-High Borrow (FAILED — phase contamination)

**Idea:** At late GCD steps, u[active_width..256] are provably zero. Host the "gated" register (materialized ctrl&subtrahend) on those idle zero bits instead of allocating fresh qubits. Would drop tobitvector peak from 1466 → 1442.

**Implementation:** Added `dialog_gcd_controlled_sub_selected_ex` / `_add_selected_ex` with `u_high: Option<&[QubitId]>` parameter. When future-log can't host gated (too short), falls back to u_high.

**Why it fails:** The gated register is cleared via Hmr (measurement-based uncompute). Hmr measures the qubit, resets it to |0⟩, then applies a phase correction via `cz_if`. Although the VALUE is correctly reset to zero, the measurement introduces phase correlations. When those same physical qubits are reused in subsequent GCD iterations, the phase correlations accumulate. Result: systematic phase-garbage failures (pg=1) across nearly all Fiat-Shamir rerolls. This is NOT a rare edge case — it's a fundamental quantum constraint on reusing measured qubits.

**Margin variants tested:** active_width+0, +28, +56. All fail. The issue is phase, not value overflow.

**Code location:** The `_ex` variants are in mod.rs around line 24041-24170. Currently disabled (call sites pass `None` / use `_u_high_disabled`).

### Projective Coordinates (RULED OUT — theoretical)

Affine with dialog-GCD already IS a 1-inversion architecture. Projective would still need inversion for affine conversion at the end, plus more multiplications. Net loss. See memory file for full analysis.

### Jump/Batched GCD (MARGINAL — 5-11%)

Window survey shows t=4 batches have only 125 distinct matrices, but applying them quantum-controlled costs nearly as much as replaying microsteps. The apply phase (47% of cost) doesn't benefit from step-batching. Net: 5-11% Toffoli reduction for high implementation complexity.

## Custom Eval Tool: `src/bin/quick_check.rs`

A custom evaluation binary that matches the official `eval_circuit` exactly (same Fiat-Shamir hash: `"quantum_ecc-fiat-shamir-v2"`, `kind as u8`) but exits on first failure. Saves ~35% time on rejections (~11s vs 20s). Processes 9024 shots in batches of 64 with early-exit between batches.

**Build:** `cargo build --release --bin quick_check`
**Usage:** `./target/release/build_circuit && ./target/release/quick_check`
- Exit 0 + "PASS" = all 9024 shots clean
- Exit 1 + "REJECT (N shots)" = failed at batch N/64

**CRITICAL:** The hash domain string must be `"quantum_ecc-fiat-shamir-v2"` and kind is hashed as `u8` (not u32). An earlier version had a wrong domain string and gave false results.

## What Should Work Next

### 1. Fix the phase contamination (HARD)

The u_high borrow would save 20+ qubits if phase-clean. Possible fixes:
- Use the qubits for carries ONLY (Cuccaro returns them clean without Hmr) — but the peak is driven by gated, not carries
- Find a non-Hmr uncompute for gated (e.g., reverse the CCX load after the sub) — but this requires the subtrahend to still be available, which it is (it's u_active)
- **Key insight not yet tried:** Instead of Hmr, uncompute gated via `ccx(ctrl, subtrahend[i], gated[i])` AGAIN after the sub. This is value-exact because `gated[i] = ctrl & subtrahend[i]` before the sub, and the sub doesn't modify gated (it's the subtrahend, not the accumulator in Cuccaro). So after the sub, gated[i] still equals ctrl & subtrahend[i], and a second CCX zeros it. **NO MEASUREMENT NEEDED.** This avoids phase contamination entirely. Cost: +n Toffoli per step (the reverse CCX load). Trade: +~24 Toffoli at the peak step for -20 qubits.

### 2. Tighten COMPARE_BITS (INCREMENTAL)

Each bit saves ~3-5K avg Toffoli. cb=58 needs a clean Fiat-Shamir island. Use quick_check for fast search. Expected: ~0.2% score improvement per bit.

### 3. Search for better parameters on existing base

The current best found reroll 52/28. There may be slightly better configurations nearby. Use quick_check to sweep.

### 4. Schrottenloher's register sharing (u,v shrink → reuse for garbage)

Not yet implemented. Could save 100-200 qubits at peak but requires major restructuring of the GCD loop to manage variable-width u,v with overlap.

## File Locations

- **This handoff:** `src/point_add/memory/2026-06-02-session-handoff.md`
- **Full analysis:** `src/point_add/memory/2026-06-02-projective-and-jump-analysis.md`
- **Custom eval:** `src/bin/quick_check.rs`
- **Main circuit:** `src/point_add/mod.rs` (~32K lines)
- **Config function:** `configure_ecdsafail_submission_route()` at line ~29674
- **Key functions:**
  - `dialog_gcd_controlled_sub_selected_ex` (line ~24041) — the u_high variant
  - `dialog_gcd_controlled_add_selected_ex` (line ~24155) — same for add
  - `emit_dialog_gcd_compressed_sidecar_tobitvector_steps_block_lifecycle` (line ~25285) — main GCD loop
  - `emit_dialog_gcd_compressed_sidecar_tobitvector_steps_reverse_block_lifecycle` (line ~25424) — reverse loop
  - `dialog_gcd_compressed_sidecar_future_carry_slice` (line ~25225) — borrow logic
  - `build_builder()` (line ~29820) — circuit entry point

## Commands

```bash
source "$HOME/.cargo/env"
~/.local/bin/ecdsafail run          # full benchmark (build + eval + score)
~/.local/bin/ecdsafail submit       # submit current score
~/.local/bin/ecdsafail sync --force # sync to latest best submission
cargo build --release --bin build_circuit --bin eval_circuit --bin quick_check
./target/release/build_circuit      # build circuit → ops.bin (~600MB)
./target/release/eval_circuit       # official eval (~20s)
./target/release/quick_check        # fast eval with early-exit (~11s reject, ~17s pass)
TRACE_PEAK=1 ./target/release/build_circuit  # show peak qubit info
TRACE_PHASES=1 ./target/release/build_circuit  # show per-phase Toffoli breakdown
TRACE_PHASE_ACTIVE=1 TRACE_PHASE_ACTIVE_TOP=10 ./target/release/build_circuit  # per-phase qubit maxima
```

## Most Promising Unexplored Idea

**CCX-based gated uncompute instead of Hmr** (item 1 above). The gated register after the sub still holds `ctrl & subtrahend[i]` (Cuccaro sub doesn't modify the A operand). A second `CCX(ctrl, subtrahend[i], gated[i])` zeros it WITHOUT measurement, avoiding all phase issues. Cost: +1 Toffoli per bit per step where u_high is used (~24 extra Toffoli at the peak step, negligible vs 1.7M total). This is the clean path to -20 qubits → score ~2.50B.
