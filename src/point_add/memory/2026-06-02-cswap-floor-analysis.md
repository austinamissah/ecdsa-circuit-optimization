# 2026-06-02 cswap reduction — FLOOR analysis (mostly negative)

Target phases (clean baseline, 1,704,086 / 1698):
- `dialog_gcd_compressed_block_tobitvector_cswap` fwd+rev = 227,720 (over u/v, active_width-truncated, 2 pairs)
- `dialog_gcd_compressed_block_apply_cswap` fwd + `..._reverse_cswap` = 204,288 (full-width N=256 over x/y; 399*256 each)

Each cswap = cx;ccx;cx = 1 CCX/bit. eval counts a CCX as `c_condition` popcount,
NOT quantum-control popcount -> every cswap CCX counts fully every shot (no
firing-rate discount).

## All three suggested approaches are BLOCKED

### (1) Merge adjacent apply cswaps via cswap(c1)·cswap(c2)=cswap(c1⊕c2): NO
- apply step = `double_y; cadd(b0); cswap(c)`, c=b0∧b1. Consecutive cswaps are
  separated by `double_y; cadd` — BOTH asymmetric (single out the y register).
- The XOR-merge identity needs the two swaps ADJACENT. Commuting a cswap through
  double_y/cadd conjugates them into c-CONTROLLED ops (controlled double of the
  OTHER register; controlled add direction). A controlled mod-double ≈ 2× the
  uncontrolled 69 CCX and you need it on both registers; routing the cadd doubles
  the 408k materialized_special phase. Net LOSS, large.
- Tobitvector has the same blocker (`cswap(c); sub(b0)` separated by asymmetric sub).
- NOTE: dialog-GCD already uses only 1 cswap/step — the old-Kaliski 2-cswap/iter
  merge was already captured by this algorithm choice. Nothing left to merge.

### (2) Tighten / fuse the tobitvector cswap: TAPPED / NO
- cswap width is LOCKED to active_width: the immediately-following controlled-sub
  reads the SAME bits, so narrowing the cswap below the sub corrupts any shot
  whose value reaches the margin zone. No cswap-specific width exists.
- active_width = N - 0.7075·step + margin is calibrated to the TRUE GCD envelope:
  * slope floor: 0.74 -> 303 mism, 0.78 -> 6208, 0.82 -> 9005. Real shrink ≈ 0.7075.
  * margin floor: 27 works (reroll 5, 0/0/0); 26 -> 1+ mism; 24 -> 9-18 mism.
- Compare-exchange-subtract fuse: removing the physical swap forces either a
  c-controlled subtract direction or a sign-magnitude subtractor that must
  preserve the discarded operand — both cost ≥ the swap. No CCX sweep saved.

### (3) Relabel / measure the cswap away: NO
- Swaps act on LIVE data (u/v, x/y), not ancilla -> measured-uncompute (free
  ancilla clear) does not apply.
- Only measurement primitive is Hmr = X-basis measure-and-reset with RANDOM
  outcome (faithful). Can't read a control into a classical c_condition. So the
  ~3/4 of apply swaps that don't fire (c=0) still cost a full 256-CCX sweep, and
  that waste is information-theoretically locked.
- Static relabel impossible: swap decisions depend on the superposed input.

## EMPIRICAL PROOF apply cswap is full-width (the big 204k target)
Flag-gated probe `DIALOG_GCD_APPLY_CSWAP_TRUNC_W=W` (truncate apply cswap to W
bits). W=255 (drop only the top bit) -> **9024/9024 classical mismatches + 141
phase-garbage**. x=target/y=accumulator are uniform 256-bit field elements; the
swap needs all 256 bits. No envelope. (Probe reverted; baseline byte-identical.)

## Confirmed floors
- ACTIVE_ITERATIONS=399 = convergence floor (397 -> 1 input fails to converge).
- margin 27, slope 0.7075 = envelope floor.

## Only clean win (width lever, NOT cswap-structural, mostly sub/add)
`DIALOG_GCD_WIDTH_MARGIN=27 DIALOG_REROLL=5` -> avg Toffoli 1,704,086 ->
**1,699,450 (-4,636)**, peak 1698, 0/0/0 over 9024. cswap share only ~1,432
(tobit 113,860 -> 113,144); rest is co-located sub/add. No code change.
(May not stack cleanly with the stray uncommitted KAL_DOUBLE_CARRY_TRUNC_W=20
reroll=1 lever — both need their own reroll island; coordinator to merge.)

## Only viable path to a BIG cswap cut = algorithmic
Batched / jump GCD (safegcd-style): fold T per-step swaps into a 2×2 transition
matrix computed on the LOW ~2T bits (cheap swaps), then apply the matrix to the
full-width values once (full-width mul-add, NO per-step swaps). This is a major
rewrite of the dialog-GCD core (see kaliski_jump.rs / round185_halfgcd_*). Not a
safe single-session flag. Recommend the swarm spend cswap effort here or move to
the reducible phases (materialized add/sub LOAD 227k, materialized_special 408k).
