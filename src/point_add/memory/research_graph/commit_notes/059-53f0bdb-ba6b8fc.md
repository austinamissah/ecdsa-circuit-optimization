# 059 53f0bdb / ba6b8fc

## Identity

- Commit: `53f0bdb306ccfbd87652493312a1aedb0311e701`
- Submission: `ba6b8fcb-0f6e-46df-8205-e239e7ceb2b5`
- Solver/co-author: anupsv
- Date: 2026-06-01T21:03:58Z
- Public metric: score 5632468386 = 2439354 Toffoli x 2309 qubits
- Public diff: -20672477 (-0.19%)

## Inferred Approach

works on block-5 transport/source-live quotient/product lowering; ports or applies Gidney-style vented adders for low-qubit arithmetic; tightens active-width envelope/margin for the dialog-GCD body.

Tags: `cswap`, `qubit-cut`, `round218-b5`, `venting`, `width-margin`

## Changed Files

- `src/point_add/kaliski_state.rs` (+15/-12)
- `src/point_add/modular.rs` (+39/-0)

## Notable Diff Signals

- `-// unique. Score: 2,448,307 T x 2309 = 5,651,540,863. UV_CSWAP_MARGIN island-invariant.`
- `+// cswap-base a25248f margin=0 island (with K0=26/R=326 only W=26 is clean at`
- `+    // the cswap-base margin=0 island only with the R=325 re-roll (K0=24 rejects).`
- `-    // Score: 2,448,307 x 2309 = 5,651,540,863 (9024-clean, 0/0/0).`
- `+        // the high result bits exact; validated 9024-clean (-6,346 avg-exec Toffoli`
- `+    // -6,346 avg-exec Toffoli vs the loaded-const full-width path, flat peak 2309.`

## Public Note Excerpt

> No public note fetched for this commit.

## Follow-up Value

- Reuse when exploring: works on block-5 transport/source-live quotient/product lowering; ports or applies Gidney-style vented adders for low-qubit arithmetic; tightens active-width envelope/margin for the dialog-GCD body.
- Watch for validation-island coupling: if this commit touched reroll or compare knobs, retest after stacking with other route changes.
