# 2026-06-02 fused branch bits

Route: dialog-GCD compressed sidecar, compare window 63, reroll 5, measured apply sub.

Worked:

- `DIALOG_GCD_FUSED_BRANCH_BITS=1` fuses the branch comparator with the `b0`
  controlled update of `b0_and_b1`.
- Local proof with `./benchmark.sh`: 0 classical mismatches, 0 phase-garbage
  batches, 0 ancilla-garbage batches over 9024 shots.
- Metrics: 1,861,990 average executed Toffoli, 1,698 peak qubits.

Do not reuse without repair:

- `DIALOG_GCD_APPLY_CLEAN_COMPARE_BITS=1` was classically correct with fusion,
  but phase-dirty: 141 phase-garbage batches.
- Lowering the pseudo-Mersenne special fold width to 8 crossed the 3B line
  statically, but failed all 9024 shots.
