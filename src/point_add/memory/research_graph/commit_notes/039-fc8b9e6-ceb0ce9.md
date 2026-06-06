# 039 fc8b9e6 / ceb0ce9

## Identity

- Commit: `fc8b9e6325f3f2a28f0f6ce1b16db99bfcbee1d8`
- Submission: `ceb0ce91-cc0b-4f3c-a4a0-6facf83072d8`
- Solver/co-author: Gajesh2007
- Date: 2026-06-01T15:28:55Z
- Public metric: score 6616811249 = 2865661 Toffoli x 2309 qubits
- Public diff: -10113420 (-0.09%)

## Inferred Approach

tightens active-width envelope/margin for the dialog-GCD body; primarily targets peak qubit width rather than raw Toffoli.

Tags: `qubit-cut`, `width-margin`

## Changed Files

- `src/point_add/kaliski_state.rs` (+9/-6)

## Notable Diff Signals

- `-    // stragglers). Validated clean; score 6,626,924,669. (Without carrytail the`
- `+    // -4,380 avg-exec Toffoli vs margin=4, peak-neutral 2309. Validated clean;`
- `+    // score 6,616,811,249. (Carry-tail base had margin=4; pre-carry-tail it was`

## Public Note Excerpt

> No public note fetched for this commit.

## Follow-up Value

- Reuse when exploring: tightens active-width envelope/margin for the dialog-GCD body; primarily targets peak qubit width rather than raw Toffoli.
- Watch for validation-island coupling: if this commit touched reroll or compare knobs, retest after stacking with other route changes.
