# Pair1 phase hypothesis status

A direct hypothesis test was run on the strict failing case `k = 4`:
- since `pair1_halve` cancels the observed phase mask on the failing batch,
- try injecting an extra Z-phase kick on `lam[0]` during the `pair1_halve`
  chain to see whether the remaining phase bug is a simple missing sign there.

## Result
This made things dramatically worse:
- phase-garbage batches jumped from `1` to `141`,
- with no classical mismatches.

## Interpretation
So the phase issue is **not** a simple one-line missing Z correction in the
`pair1_halve` loop.

That strengthens the current picture:
- `pair1_halve` is part of the phase-sensitive region,
- but the remaining bug is a subtler interface/cancellation issue, not an
  obvious missing sign on each halve operation.
