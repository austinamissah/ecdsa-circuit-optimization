# 022 3bbccd3 / 0c38fea

## Identity

- Commit: `3bbccd3cb0813249eb0ad4ba8dab1a42fd718802`
- Submission: `0c38fea7-7e87-483e-a5dc-aae6a5c583bb`
- Solver/co-author: mmurrs
- Date: 2026-05-31T17:02:39Z
- Public metric: score 9591294596 = 3541837 Toffoli x 2708 qubits
- Public diff: -16248 (-0.00%)

## Inferred Approach

ports or applies Gidney-style vented adders for low-qubit arithmetic; switches or retunes multiplication/square implementation details; primarily targets peak qubit width rather than raw Toffoli.

Tags: `karatsuba`, `qubit-cut`, `venting`

## Changed Files

- `src/point_add/mod.rs` (+288/-2)

## Notable Diff Signals

- `+/// Carry qubits are measured out with the same Gidney-style phase correction`
- `+/// qubits.`
- `+    let carries = b.alloc_qubits(n - 1);`
- `+    let carries = b.alloc_qubits(n);`
- `-    let use_venting = std::env::var("KAL_VENT_HALVE").ok().as_deref() == Some("1")`
- `+    let use_venting = env_flag_enabled("KAL_VENT_HALVE", false)`
- `+        let acc = b.alloc_qubits(n);`

## Public Note Excerpt

> No public note fetched for this commit.

## Follow-up Value

- Reuse when exploring: ports or applies Gidney-style vented adders for low-qubit arithmetic; switches or retunes multiplication/square implementation details; primarily targets peak qubit width rather than raw Toffoli.
- Watch for validation-island coupling: if this commit touched reroll or compare knobs, retest after stacking with other route changes.
