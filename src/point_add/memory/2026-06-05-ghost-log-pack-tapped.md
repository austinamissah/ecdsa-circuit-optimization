# 2026-06-05 Ghost-log / log-pack already deployed; peak binder is the apply ripple

Investigated the trailmix "ghost/spooky-pebble register dropping" and "host the
transcript log on freed u/v high bits" ideas as ways to break the ~1319q floor.
Both are **already deployed** in the current frontier route; the binder is not
the bare log.

## Peak anatomy (ISLAND_DUMP_PEAK on the baked route)

- `peak_qubits = 1319`, `peak_phase = dialog_gcd_apply_chunk_sub_final_ripple`.
- At the apply final ripple the live set is ~ tx(256) + ty(256) + a live slice of
  the compressed log + the ripple carry transient. The bare ~691q log is NOT all
  co-live with tx+ty.

## Why the log is already pebbled/hosted

`configure_ecdsafail_submission_route()` already enables:
- `DIALOG_GCD_COMPRESSED_LOG_U_HIGH_RUNWAY=1` (+ `_BLOCKS=999`): forward/reverse
  GCD parks compressed-log blocks on the provably-zero high bits of `u` (idle
  under the width envelope), so the log does not allocate fresh peak qubits while
  `u` is live. This is the "host log on freed u/v high bits" (log-pack) idea.
- `DIALOG_GCD_COMPRESSED_BLOCK_LIFECYCLE=1` + `HOST_REVERSE_RAW_BLOCK=1`:
  blocks are materialized/consumed incrementally (block lifecycle), so the whole
  transcript is not simultaneously live. This is the spooky-pebble (ghost) idea.
- `DIALOG_GCD_APPLY_REPLAY_SWAP_HOST=1`, `DIALOG_GCD_BODY_HOST_CIN=1`,
  `DIALOG_GCD_LATE_BORROW_UV_HIGH=1`, `DIALOG_GCD_BRANCH_BITS_HOST_COMPARATOR=1`:
  every transient carry/gated/comparator lane is already hosted on temporarily
  clean future-log / u-high slices.

## Floor

The residual peak is the apply-phase teardown (`APPLY_FINAL_LOWQ`,
`APPLY_BOUNDARY_SPLIT=100`, `APPLY_CHUNKED_F_CUT*`), which the leaderboard is
actively grinding. Per the 1320q-teardown note the next floors below the apply
ripple are the compressed-block tobitvector (~1320) and ROUND84 (~1309); pushing
there needs reducing coexistence around the apply ripple AND the tobitvector body
simultaneously, high-regression-risk deep work.

## Conclusion

Like deeper jump (see the jump-depth note), the structural qubit levers drawn
from trailmix are already realized here. Remaining EV is the truncation-island
notch loop (now that the classical pre-filter is validated bit-exact), plus, for a
bigger qubit cut, a dedicated apply-ripple + tobitvector co-binder teardown.
