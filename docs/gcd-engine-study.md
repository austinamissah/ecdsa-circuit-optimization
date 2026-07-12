# The binary-GCD engine: a map for optimizing the inner loop

Read-only study of the engine that spends ~95% of the Toffoli budget. Scope:
`src/point_add/trailmix_ludicrous/{gcd,comparator,gidney,mcx,fused,arith}.rs` plus the
schedule tables and correctness harness. All line references are to files under
`src/point_add/trailmix_ludicrous/` unless another path is named. Nothing here is a plan
to change code, it is a map to decide from.

## Orientation

`mod_mul_inverse_in_place` (`gcd.rs:1413`) is the shared engine for **both** modular
inversion (`Direction::Inverse`) and modular multiply (`Direction::Forward`). Each call runs
a **forward sweep + reverse sweep**, each `ITERS = 258` iterations (`schedule.rs:4`). `ec_add`
calls it twice, so one point addition = **4 GCD sweeps ≈ 1,032 iteration-steps**.

- `Direction::Inverse` (`gcd.rs:1424`): `forward_gcd_jump(.., Some((y,tmp)))` then
  `reverse_gcd_jump(.., None)`. The apply (inverse mod-sub) load rides the **forward** pass.
- `Direction::Forward` (`gcd.rs:1437`): `forward_gcd_jump(.., None)` then
  `reverse_gcd_jump(.., Some((tmp,y)))`. The apply (forward mod-add) load rides the **reverse**
  pass. This is why the profiled phases come in mirror pairs (`tlm_inverse_*` / `tlm_multiply_*`,
  `tlm_apply_inverse_*` / `tlm_apply_forward_*`).

Field constant (`arith.rs:430-440`): `F = 2^32 + 977`, since `p = 2^256 − 2^32 − 977` ⇒
`2^256 ≡ F (mod p)`. Reduction folds an overflow bit back in by adding `F·overflow`.

**Register roles inside one sweep** (`gcd.rs:722-733`):
- `u`, the odd running value; `u[0]` is invariantly `|1⟩` ("parked known-one").
- `v`, the other value; after the shifts `v[0]` is invariantly `|0⟩` ("parked known-zero").
- `subtracted`, per-iteration parity bit (`v[0]`), also the swap control at `i==0`.
- `swap_flag`/`sf`, the `v<u` compare-and-swap decision for `i>0` (allocated lazily, `gcd.rs:787`).
- `s2`, `t1`, controls capturing the even/first right-shift so they invert exactly.
- Per-iteration live width `current_n = SCHED_J2[i]` (`gcd.rs:753`); comparator width
  `cmp_eff = GAP_J2[i]` (`gcd.rs:763`). `JUMP` is **asserted == 2** (`gcd.rs:717`), the codec/apply
  logic is hardwired for jump-2, so `JUMP` is not a tunable.

---

## 1. One GCD iteration, step by step, and which cost it produces

Every Toffoli in the engine is a `ccx`/`ccz`/`cswap`. Below, each forward step names its
`set_phase` label (the profiler's unit), what it computes, the qubits it touches, and where
the Toffolis come from. The reverse sweep mirrors it with one key substitution (§1.6).

### 1.1 Shift, `tlm_*_gcd_forward_shift` (`gcd.rs:748-777`), profiled ~2.5% each
- Width shrink: pops high qubits off `u`/`v` and `zero_and_free`s them (`gcd.rs:754-761`), this
  is where high limbs return to `|0⟩` as the schedule narrows.
- Two right-shifts of `v`, the first/second conditioned on `NOT v[0]` via `t1`/`s2`
  (`gcd.rs:766-775`); records parity `cx(v[0], subtracted)` (`gcd.rs:777`).
- **Toffoli source:** `controlled_right_shift` is a chain of `cswap` (`gcd.rs:634-646`), 1 Toffoli
  per limb. Plain `right_shift` is `swap`-only (no Toffoli, `gcd.rs:662-666`).

### 1.2 Compare, `tlm_*_gcd_forward_compare` (`gcd.rs:779-813`), profiled ~3.3% each
- Produces the swap-decision bit. `i>0`: allocates `sf` (`gcd.rs:787`) and calls
  `controlled_swap_decision_v_lt_u` → `controlled_swap_decision_lt_truncated`
  (`comparator.rs:768`), a **truncated top-k `v<u` comparison** over only `cmp_eff` limbs.
- Engine `compare_geq_chunked_middle_direct` (`comparator.rs:646-734`): low `split` bits ripple
  through one reused carry (`ccx` at `comparator.rs:677`), top `k` bits each get a fresh held
  carry (`ccx` at `comparator.rs:692`). The decision is written by one `ccx(ctrl,carry,target)`
  (`comparator.rs:790`). **Cost ≈ 2·split + k = 2n − k** for width `n=cmp_eff`; the held carries
  are uncomputed by measurement-vent (no reverse Toffoli, `comparator.rs:705-707`).
- Then the register compare-and-swap `for j in 1..current_n { cswap(swp,u[j],v[j]) }`
  (`gcd.rs:799-807`), 1 Toffoli/limb, and `park_known_one(u[0])` frees `u[0]` (`gcd.rs:808`).

### 1.3 Body, `tlm_*_gcd_forward_body` (`gcd.rs:815-834`), profiled ~5.8% fwd / ~11-12% rev
- The controlled modular subtract `v ← v − u` done as add-of-complement: X-wrap `v`
  (`gcd.rs:820`), `controlled_add_active(..)` (`gcd.rs:823`), X-unwrap (`gcd.rs:832`).
- `controlled_add_active` (`gcd.rs:1209`) bottoms out in
  `gidney::controlled_hybrid_add_capped_branch` under `with_dirty_vent_pool`
  (`gcd.rs:1239-1249`), **the principal Toffoli engine of the whole circuit.** It is a
  carry-lookahead add emitting ≈**1 CCX/bit forward + 1 CCX/bit for the controlled sum**, with
  interior carries either clean or borrowed-dirty (`gidney.rs:1032-1165`). The reverse-sweep
  body is the same adder in inverse mode and is the ~11-12% `_reverse_body` line.

### 1.4 Apply, `tlm_apply_{inverse,forward}_*` (`gcd.rs:836-873`), the biggest phases
Active only on the sweep carrying the apply load. Runs the co-processed modular arithmetic on
the `(xr,yr)` accumulator via `apply_step_reverse`/`apply_step_forward`, emitting sub-phases:
- **`..._fold`** (`gcd.rs:1324`), ~6% each. `fused::fused_double_cdouble[_reverse]`
  (`fused.rs:1845/1901`): merges the modular fold `y += F·overflow` with the register
  double/shift into one carry sweep. Fold-constant bits become per-position **controls** derived
  from two overflow bits `e,d` (`fold_ctl`, `fused.rs:915`); only ~1 Toffoli to build the `e∧d`
  control (`fused.rs:1138`), the rest are carry `ccx` (e.g. `fused.rs:1025,1214,1581,1663`),
  reverse legs vented (0 Toffoli).
- **`..._swap`** (`gcd.rs:1334`), ~4.7% each. `for j in 0..n { cswap(swp, x[j], y[j]) }`
  (`gcd.rs:1336`), 1 Toffoli/limb over the **full n=256** (unlike §1.2's swap, which is bounded
  to `current_n`). See candidate C.
- **`..._mod_sub_register` / `..._mod_add_register`** (`gcd.rs:1356` / `arith.rs:1672`),
  **~13% each, the single largest sub-phase.** `mod_sub_vented` (`arith.rs:1736`) /
  `mod_add` (`arith.rs:1672`): a vented Cuccaro/Gidney add of `x` into `y` with overflow into a
  single `anc`, then the fold, then a final conditional correction. Bottoms out in
  `gidney::controlled_hybrid_add_cout_refs` (`gcd.rs:1400-1408`). ≈**2·width Toffoli**.
- **`..._mod_sub_fold`** (`gcd.rs:1365`), ~1% each. Conditional `add_f_window_pub` of `F`
  (the FFG fold adder, `arith.rs:1159`, window `g` capped by `TLM_FFG_MAX_G=47`).
- **`..._mod_sub_clean`** (`gcd.rs:1375`), ~0.35% each. Measures `anc` (`hmr`+`zero_and_free`,
  `gcd.rs:1380`, **this is where `anc` returns to `|0⟩`**), then a condition-gated
  `compare_geq_chunked_middle` with a `cz` body (no data Toffoli).

### 1.5 Codec, `tlm_*_gcd_forward_codec` (`gcd.rs:875-931`), profiled ~0.1% each
Writes the iteration's 3-symbol decision `[subtracted, swap-decision, s2]` onto the **tape**,
compressed by `DialogCodec` (Step0→2 bits, Triple→3 syms into 7 qubits, Pair/Raw/Tail4Top32).
`compress_step0_with_t1` emits the only Toffoli here (`codec.rs:403`) and absorbs `t1` into the
tape rather than freeing it.

### 1.6 The reverse mirror (`gcd.rs:992-1173`)
Same body run backwards, iterating `i` from 257 down to 0, **with one crucial substitution: the
comparator is not re-run to recompute the swap decision, it is decoded from the tape**
(`tlm_*_gcd_reverse_decode`, `gcd.rs:994-1062`). `decompress_window` hands back the exact stored
bit (`gcd.rs:1015-1019`); the decoded tape qubits return to `|0⟩` at `gcd.rs:1060`. The comparator
then appears only in **vented uncompute** form, `swap_decision_uncompute_vented`
(`comparator.rs:860`), which runs conditioned on a measured flag and emits `cz` (not `ccx`) to
drive the flag ancilla back to `|0⟩`. This is why `_reverse_body` (the adder) is costly but there
is no `_reverse_compare` line, the reverse pass replaces recompute with decode.

### Cost map (per the TRACE_TLM_CCX measurement, % of all CCX)
| profiled sub-phase | code | algorithm | ~% each (×2 sweeps) |
|---|---|---|---|
| `apply_*_mod_sub/add_register` | `arith.rs:1672/1736` → `gidney` cout add | vented Cuccaro/Gidney add | **13.1%** |
| `*_gcd_reverse_body` | `gcd.rs:1105` → `controlled_hybrid_add_capped_branch` | carry-lookahead add (inverse) | **11-12%** |
| `apply_*_fold` | `fused.rs:1845` | fused fold+double | **5.9%** |
| `*_gcd_forward_body` | `gcd.rs:823` → same adder | carry-lookahead add | **5.8%** |
| `apply_*_swap` | `gcd.rs:1336` cswap×256 | full-width conditional swap | **4.7%** |
| `*_gcd_forward_compare` | `comparator.rs:646` | truncated top-k `v<u` | **3.3%** |
| `*_gcd_forward_shift` | `gcd.rs:634` cswap chain | conditional right-shift | **2.5%** |

---

## 2. Reversibility structure, what you must not break

The whole engine is a reversible computation whose ancilla must all return to `|0⟩` and which
must leak no phase. The measurement-vent pattern is used everywhere and is the subtlest part.

### The vent pattern (used in gidney, comparator, fused, arith)
A held carry ancilla `q` holding the pure AND `a·b` is returned to `|0⟩` by measurement instead
of an inverse Toffoli: `bit = hmr(q)` (H, measure, reset), `zero_and_free(q)`, then
`cz_if_bit(a,b,bit)` to cancel the random Z kickback (e.g. `gidney.rs:1114-1120`,
`comparator.rs:705-707`, `fused.rs`/`arith.rs` vented legs). **This trades one reverse CCX for a
measurement + one classically-conditioned CZ (0 Toffoli).** Two ways to break it:
1. **Skip the `cz_if_bit` correction** → a random Z survives ⇒ **phase leak** (fails the eval
   PHASE GARBAGE check).
2. **Skip the pre-measurement unthread `cx(carry-in, q)`** (e.g. `gidney.rs:1248`) → `q` is still
   entangled with the carry chain and measuring it **corrupts data** (fails CLASSICAL MISMATCH).

The **dirty-vent** variant (`gidney.rs:1121-1129`) restores a *borrowed* ancilla by conjugation
(`cx;ccx;cx`) rather than measurement; the two `cx` brackets plus the `ccx` must exactly invert
the forward bracket or the lender's qubit is returned dirty. Guarded by `debug_assert_ne!`
aliasing checks (`gidney.rs:1076-1078`).

### Invariants each step preserves
- `u[0] ≡ |1⟩` (odd) and, post-shift, `v[0] ≡ |0⟩`, the parked known-one/known-zero
  (`park_known_one` `gcd.rs:212`, `park_known_zero` `gcd.rs:233`). The controlled subtract keeps
  `u` = the current GCD candidate; `s2`/`t1` capture the shift controls so shifts invert exactly
  by the reverse left-shifts.
- The **tape** encodes, per iteration, exactly the information the reverse pass needs to avoid
  recomputing the comparator. `forward` builds `window_plan` climbing `win_idx` 0→len; `reverse`
  descends len→0 over the identical plan (`gcd.rs:736-741` / `972-977`). Reverse asserts the tape
  is fully drained (`assert!(tape.is_empty())`, `gcd.rs:1174`).

### Ancilla ledger, where each returns to `|0⟩`
| ancilla | allocated | returned to `|0⟩` |
|---|---|---|
| per-iter tape qubits (`slots`) | `gcd.rs:880` (fwd) | `gcd.rs:1060` (rev, `cur`) |
| comparator carries `c`/`next` | `comparator.rs:671/682` | inverse CCX (low) / HMR vent (top), `comparator.rs:705,715` |
| body adder interior carries | `gidney.rs:1088` clean / dirty pool | HMR vent (clean) / conjugation (dirty), `gidney.rs:1114-1129` |
| fold overflow `hi,hi2` + controls | `fused.rs:1852` / `1138` | `fused.rs:1879-1882` / `clear_and` HMR `fused.rs:927` |
| mod-sub/add `anc` | `arith.rs`/`gcd.rs:1354` | final conditional correction / `hmr` `gcd.rs:1380` |
| loop registers `u,v,subtracted,swap_flag,s2,t1` | loop head | teardown `gcd.rs:947-957` (fwd) / `1156,1160,1183-1189` (rev) |

Special case: `t1` is **absorbed into the tape** by `compress_step0_with_t1` in forward
(`gcd.rs:890`) and **recreated** by `decompress_step0_with_t1` in reverse (`gcd.rs:1015`), then
freed at `i==0`. Any change to the codec must preserve this or the tape/`t1` accounting desyncs.

---

## 3. Self-tests and guardrails, the tools to catch mistakes

### The ultimate net: the evaluator's four checks (`src/bin/eval_circuit.rs:278-345`)
Runs the built circuit on **9,024 points** whose inputs are a Fiat-Shamir hash of the op stream
(cannot be tuned against). It enforces:
1. **CLASSICAL MISMATCH**, output `(x,y)` must equal the true point-add result (`:305`).
2. **PHASE GARBAGE**, `global_phase` must be 0 across live shots (`:317`), catches vent
   phase leaks.
3. **ANCILLA GARBAGE**, after zeroing the 4 output registers, **every other qubit must be `|0⟩`**
   at end of forward (`:335-345`), catches any ancilla left dirty.
4. **Reversibility**, the forward+inverse must restore the input (per the file's stated checks).

Caveat to keep honest: passing eval means correct on 9,024 pseudo-random points, **not a proof for
all inputs.** A truncation that drops a bit only exercised by rare inputs could pass eval and still
be wrong. Width/vent trades are fully validated by eval (they are exact by construction); truncation
schedule changes are only *sampled* by it.

### Build-time self-tests (env-gated, fast, exact-equivalence)
Each builds a small circuit and simulates the optimized variant against a baseline, checking equal
result + clean ancilla/phase. Relevant to this engine:
- `TLM_SQ_SELFTEST` → `arith::square_addsub_selftest` (`mod.rs:1659`), the squaring add/sub backend.
- `SQUARE_WINDOW_SELFTEST` → `square_window_selftest` (`mod.rs:1967`), windowed square vs direct.
- `FOLD_FREED_TAIL_SELFTEST` → `fold_freed_tail_selftest` (`mod.rs:2121`), proves freeing the fold
  tail is equivalent to the baseline, ancilla+phase clean.
- `special_fold_park_selftest` (`mod.rs:2292`), proves parking low carries is equivalence-preserving.
- `TLM_FOLD_HMR_CONTROL_SELFTEST` → `gradual_fold_nonlinear_control_hmr_selftest` (`fused.rs:1966`)
, proves the HMR cleanup of nonlinear fold controls is exact (1 Toffoli, phase-clean). **This is
  the direct guardrail for the vent pattern.**
- `comparator.rs` `#[test]`s (`:895`), incl. `freed_predicate_lane_funds_one_held_carry`, asserts
  the direct comparator has exactly one fewer Toffoli than the flag form at equal width.
- Note: the `DIALOG_GCD_K5_*_SELFTEST` family tests the **dead** `rounds/dialog` engine, not this one.

### Internal assertions (fail fast at build)
- `assert_eq!(JUMP, 2)` (`gcd.rs:717`); tape-length asserts (`gcd.rs:958`, `967`, `1174`).
- `#[track_caller]` on `ccx`/`ccz` panics on control/target aliasing (`mod.rs:597-610`, `:60`).
- Width `assert_eq!` in every adder (`gidney.rs:1042,1763,1894`), MCX ancilla-count asserts
  (`mcx.rs:92-98`), comparator width/k asserts.
- **Call-index bookkeeping** (`reset_*_call_index`, thread-locals): every structural-dead skip
  table is keyed by emission-order call index. If emission order changes, the skip lists silently
  target the wrong gate. This is the highest-risk coupling in the codebase, see candidate D.

---

## 4. The schedule tables (`schedule.rs`), tunable vs correctness-critical

Loaded once per build by `load_schedule` (`mod.rs:261-303`) into a thread-local `Sched` struct,
with env-var override hooks (`LUD_EXTRA_FOLD_*`, `TLM_HYB_V_*`). Two categories:

**Structural / correctness data, do NOT edit blindly:**
- `ITERS=258`, `JUMP=2`, `PAD=20`, loop structure; `JUMP` is asserted, `ITERS` sets tape length.
  Changing `ITERS` changes the GCD depth and every tape/codec size.
- `SCHED_J2`, `GAP_J2`, per-iteration live width and comparator width. These bound the arithmetic
  to bits that are provably meaningful at each step; they are correctness-linked (too small drops
  live bits) but also the main lever (too large wastes gates). Tunable **downward only to the true
  minimal width**, which is not obviously known.
- `GCD_BRANCH` (`[u8;1032]`, values 0/1/2), selects the step *shape* per iteration. This encodes
  the GCD schedule structure; a wrong value emits the wrong circuit. Treat as fixed.

**Per-step width knobs, the experimentation surface (affect gate count, correctness-linked if cut
too far):**
- `GCD_SUB_K` (`[1032]`), `APPLY_COUT_K` (`[516]`), `CMP_K`, `FOLD_SCHED` (`[514]`), `HYB_V`
  (`[1558]`), `FFG_G`, `SQ_ROW_K`, these are per-call "k" values: how many bits to process, how
  many carries to hold/vent, which fold layout to pick, how wide the comparator/window is. They
  exploit that high bits are provably zero/known at a given step. **Larger = correct but costlier;
  smaller = fewer gates but wrong if it drops a live bit.** The current values are already tuned
  near the frontier. These are the honest tuning targets, validated (partially) by eval.

Env deltas layer on top without editing the tables: `LUD_EXTRA_FOLD_VENTS/_MIN_G/_MAX_G`
(`mod.rs:271-295`, FFG values capped at 53), `TLM_DIRTY_VENTS` (`gidney.rs:977`), `TLM_HYB_V_DELTA`
/`_CALL_DELTAS`/`_CALL_OVERRIDES` (`mod.rs:240-242`), the many `TLM_*_SKIP_*` dead-gate flags.

---

## 5. Optimization candidates (candidates only, not implemented)

**Framing first:** score = Toffoli × peak_qubits, and peak is pinned at **1152**
(`TLM_TARGET_Q=1152`, `SQUARE_PEAK_CAP=1152`). So a Toffoli reduction only helps if it does **not**
push peak above 1152. Vent-based reductions *trade width for Toffoli*: they only pay off in phases
that have width headroom below 1152. Pure wins come from removing genuinely-dead gates or
truncating provably-zero bits. Keep this in mind for each candidate.

**Candidate A, increase held-carry venting in the body adder (`HYB_V` / `TLM_DIRTY_VENTS`).**
The body add (`controlled_hybrid_add_capped_branch`, `gidney.rs`) is the single biggest emitter
(`_mod_sub/add_register` ~13% + `_reverse_body` ~11-12%). Every extra vent converts one reverse
CCX to a measurement (0 Toffoli). Adjust via `HYB_V` (`schedule.rs:84`) or `TLM_DIRTY_VENTS`.
*Risk to reversibility:* low, the vent pattern is exact and covered by
`gradual_fold_nonlinear_control_hmr_selftest` and the eval phase/ancilla checks. *But:* it raises
peak width, so it only helps where a phase sits below 1152; otherwise it raises the score.
*Validate:* build + eval, watch both avg Toffoli **and** peak qubits (must stay ≤1152).

**Candidate B, tighten the comparator/GCD widths (`CMP_K`, `GAP_J2`, `GCD_SUB_K`).**
The compare (~3.3% each) costs ≈`2·cmp_eff − k`; the body add cost scales with `current_n`. If any
of these per-step widths are conservative versus the true minimal width the GCD needs at that step,
shrinking them removes `~2` Toffoli per bit per step across 1,032 steps. *Risk:* **high**, too
small drops a live bit and silently corrupts rare inputs. *Validate:* eval catches wrong decisions
only if a test point exercises the dropped bits, so this needs a correctness argument about the
GCD's bit-growth bound per iteration, not just a green eval. Best treated as: derive the true bound,
then set the table to it.

**Candidate C, truncate the apply-swap to live width (`gcd.rs:1336`).**
`tlm_apply_*_swap` (~4.7% each) does `cswap(swp, x[j], y[j])` over the **full n=256**, whereas the
GCD-internal swap (§1.2) is already bounded to `current_n`. If the high limbs of the `(x,y)`
accumulator are provably equal (or zero) at that point in the schedule, those cswaps are dead.
*Risk:* medium, a nonzero high limb that gets skipped corrupts data. The mechanism already exists:
`gcd_forward_cswap_has_structurally_dead_gate` (`gcd.rs:800`) gates the GCD swap via a dead-gate
table; an analogous provable-dead table could cover the apply swap. *Validate:* eval + a structural
argument that the skipped limbs cannot differ.

**Candidate D, extend the structural-dead skip tables.**
Many `TLM_*_SKIP_*` flags drop gates proven dead at specific `(call_index, bit)` keys
(`gidney.rs`, `comparator.rs`, `fused.rs`, `arith.rs`). Genuinely-dead removals are **pure wins**
(fewer Toffoli, zero width cost). *Risk:* high if wrong, these are keyed by emission-order call
index (§3), so any earlier structural change shifts the indices and a stale entry drops a **live**
gate, leaving ancilla dirty. *Validate:* eval, but with the caveat that dead-gate proofs should come
from analysis (e.g. the CONSTPROP pass already finds some), not from "eval still passes." Safest as a
follow-on to a tool that *proves* a gate never affects an output.

**Where I would start:** Candidate A is the lowest-risk lever (exact, self-test-covered) and directly
targets the two ~13%/~12% phases, but it is a width/Toffoli trade, so its payoff depends on
per-phase width headroom below 1152. Candidate C is the most attractive *pure* win if the high
limbs are provably dead. Candidates B and D offer the most Toffoli but require correctness proofs
beyond a passing eval.
