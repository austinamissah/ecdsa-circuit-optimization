# 096 acf2bdb / 8ee9ad8

## Identity

- Commit: `acf2bdbd08d8836b5f8e5b4c6a251b7ecd907118`
- Submission: `8ee9ad8f-ac39-40e0-9d2c-bc86cc727ead`
- Solver/co-author: saucegodbased
- Date: 2026-06-02T08:17:02Z
- Public metric: score 3121008900 = 1838050 Toffoli x 1698 qubits
- Public diff: -40650120 (-0.38%)

## Inferred Approach

retunes truncated comparison width, trading correctness-island search against Toffoli; changes the core approximate dialog-GCD inversion/addition route; retunes Fiat-Shamir reroll knobs to find a clean validation island after exact route changes.

Tags: `apply-clean`, `compare-width`, `dialog-gcd`, `qubit-cut`, `reroll-island`

## Changed Files

- `src/point_add/mod.rs` (+9/-6)

## Notable Diff Signals

- `-        cmp_lt_into(b, &acc[compare_start..], &f[compare_start..], acc_ovf);`
- `+        cmp_lt_into_fast(b, &acc[compare_start..], &f[compare_start..], acc_ovf);`
- `-    set_default_env("DIALOG_REROLL", "5");`
- `+    // Apply-phase clean compares also use the measured comparator`
- `+    // (cmp_lt_into_fast); op stream changes, reroll=4 lands a clean island.`
- `+    set_default_env("DIALOG_REROLL", "4");`

## Public Note Excerpt

> No public note fetched for this commit.

## Follow-up Value

- Reuse when exploring: retunes truncated comparison width, trading correctness-island search against Toffoli; changes the core approximate dialog-GCD inversion/addition route; retunes Fiat-Shamir reroll knobs to find a clean validation island after exact route changes.
- Watch for validation-island coupling: if this commit touched reroll or compare knobs, retest after stacking with other route changes.
