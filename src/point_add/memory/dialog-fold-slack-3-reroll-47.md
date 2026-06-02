# Dialog fold slack 4→3 with R_SMALL=326 and reroll=47

State slice: `src/point_add/kaliski_state.rs` DIALOG_FOLD_SLACK and
R_SMALL_THRESHOLD, plus `src/point_add/mod.rs` KAL_REROLL default.

The current C* stack (`KAL_DIALOG_FOLD=1`, `AFFINE_SQUARE_RECOMPUTE=1`,
`KAL_GZ_EARLY_RECOVER=1`, `KAL_WTRUNC_K0=20`, margin 0, carry-tail W=19)
validates with `R_SMALL_THRESHOLD=326`, `KAL_DIALOG_FOLD_SLACK=3` when the
free reroll is `KAL_REROLL=47`.

Validation:

```bash
KAL_R_SMALL_THRESHOLD=326 KAL_DIALOG_FOLD_SLACK=3 KAL_REROLL=47 ./benchmark.sh
```

Result: 0 classical mismatches, 0 phase-garbage batches, 0 ancilla-garbage
batches over 9024 shots. Metrics: 2,559,463 average executed Toffoli, 2024
qubits, score 5,180,353,112.

Previous state: slack=4 with R_SMALL=326 gave 2,559,463 Toffoli × 2025 =
5,182,912,575. Narrowing the recovery band by 1 bit (slack 4→3) frees one
additional v_w high bit for dialog folding, dropping peak by 1 qubit.
Score delta: −2,559,463 (−0.049%).

Slack=2 was also clean at rr=510 but did not yield a further qubit reduction
(binding constraint elsewhere). Slack=1 and R_SMALL=327 both had no clean
reroll islands in 0-511.

### Prior state (first submission):
R_SMALL=325→326 with slack=4 and reroll=254 gave 5,182,912,575.
