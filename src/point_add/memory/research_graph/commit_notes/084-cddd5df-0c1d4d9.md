# 084 cddd5df / 0c1d4d9

## Identity

- Commit: `cddd5dfeca39a1f92e957b164a331931b7689e67`
- Submission: `0c1d4d95-f3d7-4120-863b-ce6d9934a921`
- Solver/co-author: pldallairedemers
- Date: 2026-06-02T06:04:44Z
- Public metric: score 4156442508 = 2447846 Toffoli x 1698 qubits
- Public diff: -964247614 (-8.96%)

## Inferred Approach

lifetime/hosting optimization that borrows future-clean lanes instead of allocating full scratch; hosts comparator carry/scratch on already clean slices to move the peak binder; moves the x-tail square/Solinas binder that controls the low-qubit floor.

Tags: `active-iters`, `apply-clean`, `branch-hosting`, `compare-width`, `cswap`, `dialog-gcd`, `karatsuba`, `low-bit-fastpath`, `partial-hosting`, `qubit-cut`, `reroll-island`, `round218-b5`, `round763-compressor`, `round84-square`, `venting`, `width-margin`

## Changed Files

- `src/point_add/bench_by.rs` (+0/-1059)
- `src/point_add/bench_probe.rs` (+0/-1181)
- `src/point_add/bench_scaled.rs` (+0/-46)
- `src/point_add/blockers_and_paths.md` (+287/-0)
- `src/point_add/builder.rs` (+0/-636)
- `src/point_add/by.rs` (+16365/-0)
- `src/point_add/by_sota_architecture.md` (+295/-0)
- `src/point_add/compare.rs` (+0/-356)
- `src/point_add/coset_proto.md` (+137/-0)
- `src/point_add/coset_proto.rs` (+354/-0)
- `src/point_add/cost_model.md` (+687/-0)
- `src/point_add/creative_attempts_log.md` (+138/-0)
- `src/point_add/cuccaro.rs` (+0/-912)
- `src/point_add/fermat_inv.rs` (+526/-2)
- `src/point_add/google_harness_match.md` (+88/-0)
- `src/point_add/halfgcd_coeff_decoder.rs` (+978/-0)
- `src/point_add/halfgcd_live_pa.rs` (+568/-0)
- `src/point_add/kaliski_1200q_feasibility.md` (+753/-0)
- ... 52 more files

## Notable Diff Signals

- `-    let f = b.alloc_qubits(acc.len());`
- `-    let f = b.alloc_qubits(acc.len());`
- `-        cswap(b, a, r[i], s[i]);`
- `-    let f = b.alloc_qubits(acc.len());`
- `-    let f = b.alloc_qubits(acc.len());`
- `-        cswap(b, a, r[i], s[i]);`
- `-        cswap(b, a, r[i], s[i]);`
- `-        cswap(b, a, r[i], s[i]);`
- `-pub(crate) fn by_2adic_branch_step_for_bench(`
- `-        cswap(b, a_out, f[i], g[i]);`
- `-pub(crate) fn by_2adic_branch_step_reverse_for_bench(`
- `-        cswap(b, a_hist, f[i], g[i]);`
- `-pub(crate) fn by_signed_branch_step_for_bench(`
- `-        cswap(b, a_out, f[i], g[i]);`
- `-pub(crate) fn by_signed_branch_step_reverse_for_bench(`
- `-        cswap(b, a_hist, f[i], g[i]);`
- `-pub(crate) fn by_signed_branch_apply_step_for_bench(`
- `-        cswap(b, a, f[i], g[i]);`
- `-pub(crate) fn by_signed_branch_apply_step_reverse_for_bench(`
- `-        cswap(b, a, f[i], g[i]);`
- `-    // 16 BY branch decisions depend only on the low 16 bits of the current`
- `-    let f = b.alloc_qubits(QBITS);`
- `-    let g = b.alloc_qubits(QBITS);`
- `-    let delta = b.alloc_qubits(delta_full.len());`

## Public Note Excerpt

> TensorFurnace dialog-GCD compressed-sidecar point-addition port for the ecdsa.fail secp256k1 PA benchmark.
> Local official benchmark result:
> - Benchmark seed/domain: ecdsa.fail `quantum_ecc-fiat-shamir-v2` over `ops.bin`
> - Correctness: all 9024 shots OK
> - Classical mismatches: 0
> - Phase-garbage batches: 0
> - Ancilla-garbage batches: 0
> - Qubits: 1698
> - Average executed Toffoli: 2447846
> - Claimed score: 4156442508

## Follow-up Value

- Reuse when exploring: lifetime/hosting optimization that borrows future-clean lanes instead of allocating full scratch; hosts comparator carry/scratch on already clean slices to move the peak binder; moves the x-tail square/Solinas binder that controls the low-qubit floor.
- Watch for validation-island coupling: if this commit touched reroll or compare knobs, retest after stacking with other route changes.
