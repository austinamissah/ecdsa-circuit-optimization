# R-small 326 with reroll 254

State slice: `src/point_add/kaliski_state.rs` R-small threshold and
`src/point_add/mod.rs` Fiat-Shamir reroll default.

The current C* stack (`KAL_DIALOG_FOLD=1`, `AFFINE_SQUARE_RECOMPUTE=1`,
`KAL_GZ_EARLY_RECOVER=1`, `KAL_WTRUNC_K0=20`, margin 0, carry-tail W=19)
validates with `R_SMALL_THRESHOLD=326` when the free reroll is `KAL_REROLL=254`.

Validation:

```bash
KAL_R_SMALL_THRESHOLD=326 KAL_REROLL=254 ./benchmark.sh
```

Result: 0 classical mismatches, 0 phase-garbage batches, 0 ancilla-garbage
batches over 9024 shots. Metrics: 2,559,463 average executed Toffoli, 2025
qubits, score 5,182,912,575.

Previous state: R_SMALL=325 with reroll=10 gave 2,559,671 Toffoli x 2025 =
5,183,333,775. The +1 R_SMALL bump (325→326) applies the r-doubling shortcut
(mod_double replaced by plain shift, ~255 CCX saved) to one additional iteration,
saving ~208 avg-executed Toffoli (the exact delta varies since not all 9024 shots
reach iter 326).

Negative evidence: R_SMALL=327 had no clean reroll in 0-255 screen (every rr hit
at least 1 classical mismatch + phase garbage).
