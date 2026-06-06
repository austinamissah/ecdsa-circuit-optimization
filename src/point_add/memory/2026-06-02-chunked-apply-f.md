# 2026-06-02 Chunked Apply F

Submission route added a peak-safe replacement for the materialized apply
add/sub helpers. Instead of allocating all 256 bits of `f = ctrl & a` across
the raw apply ripple, `DIALOG_GCD_APPLY_CHUNKED_F_BLOCKS=2` loads one slice at
a time, clears it by HMR, and clears the carry/borrow boundary with a controlled
truncated comparator.

The winning route uses `DIALOG_GCD_APPLY_CHUNKED_F_CUT=70`, which keeps the raw
apply phases below the existing ROUND84 peak while shrinking the boundary
comparators. It is paired with fused truncated underflow cleanup:
`ctrl & !(acc < !a)` is emitted as `CX(ctrl)` plus one controlled comparator,
removing the temporary underflow predicate and second comparator pass.

Validated island:

- `DIALOG_GCD_APPLY_CHUNKED_F_BLOCKS=2`
- `DIALOG_GCD_APPLY_CHUNKED_F_CUT=70`
- `DIALOG_REROLL=4`
- `DIALOG_POST_SUB_REROLL=15`
- Trace/eval: `1567` qubits, `1,689,505` average executed Toffoli,
  score `2,647,954,335`, all `9024` shots OK.

Cut scan notes from this base:

- `cut=68`: `1567q`, `1,687,909T`, but first seed failed and needs reroll.
- `cut=70`: `1567q`, `1,689,505T`, clean at reroll `4/15`.
- `cut=72`: `1567q`, `1,691,101T`.
