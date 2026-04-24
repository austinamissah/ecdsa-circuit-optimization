# Autoresearch Ideas Backlog

## Current State (2026-04-23)
- Best: **4,188,698 Toffoli @ 2717 qubits**, 24-seed phase-robust.
- SOTA target: **2.1M Toffoli @ 1175 qubits** (Babbush-Zalcman-Gidney et al., arXiv:2603.28846).
- Gap: ~2M Toffoli, ~1500 qubits.
- Already beats published HRSL 2020 (~12M) and Kim 2026 (~17M) by 3-4×.

## Peak qubit breakdown (at `kal_bulk_step4`)
Persistent ~2205: tx(256) + ty(256) + lam(256) + st.u(256) + st.v_w(256) + st.r(256) + st.s(256) + st.m_hist(408) + st.f_flag(1) + iter flags(4).
Transient ~513: step4 tmp(256) + Cuccaro carries(255) + misc(2).

## Priority-1 moonshot: Gidney 2025 venting adder
**The right route to SOTA. Multi-week port.**

Paper: Craig Gidney, "A Classical-Quantum Adder with Constant Workspace and Linear Gates", July 2025 (arXiv:2507.23079). Likely the core primitive underlying Google SOTA.

Key result: classical-quantum add in **3 clean ancillae + 4n Toffolis** (or 2 clean + n-2 dirty, 3n Toffolis). Controlled version has zero extra cost.

Technique: "venting" = measure Z-redundant carry qubits in X basis, leaving phase tasks fixed later via HRS17 carry-xor + classically-controlled Z gates.

**Implementation plan**:
1. Fetch Zenodo Python reference (doi:10.5281/zenodo.15866587).
2. Port streaming-MAJ + venting adder primitive (~400 LOC).
3. Port HRS17 carry-xor primitive for phase fixup.
4. Replace ~34 call sites of `add_nbit_const_fast`/`csub_nbit_const_fast`/`cadd_nbit_const_fast`.
5. Expected impact: peak 2717 → ~2460 (-256q), Toffoli likely net neutral.

**Risk**: phase-bug-prone. The critical circuit diagrams (Figures 2-6) are not in PDF-extracted text; must port from Python code. Without that reference, don't re-derive from paper text alone.

## Priority-2 moonshot: windowed Montgomery inversion (Gidney-Ekera style)
Targets 1100q. Core primitives:
1. Montgomery form throughout: `x̃ = x·2^n mod p`, `mul_mont(a,b) = a·b·2^{-n} mod p`.
2. Unified Kaliski/Montgomery with 4-bit window per step.
3. Window history ~n/4 = 64 qubits replace our 408-qubit m_hist.
4. Fold one Kaliski register onto input register.

**Estimated budget**: 512 (inputs doubling as Kaliski state) + 256 (aux) + 64 (window) = ~830q. Matches SOTA.

**Implementation complexity**: ~1000 LOC. Multi-week.

## Priority-3 moonshot: Kim 2026 unconditional Kaliski
Eliminates m_hist (-409q). Case computed from state each iter, not stored.
- Cost: +9-28% Toffoli per literature.
- Net 2718 → ~2310 qubits. Insufficient alone, but stacks with other moves.

## Known dead ends (don't re-attempt)
- **Montgomery batched inversion** (`c = dx·N` trick): cleanup requires 2nd Kaliski, net zero savings. Proven.
- **Bernstein-Yang divsteps (all w)**: per-iter cost × iter count ≥ Kaliski at every window width.
- **Jacobian coordinates**: same cleanup obstruction as Montgomery batched.
- **Naive Karatsuba in-Kaliski**: exceeds 2800 qubit cap (peak jumps to ~2996).
- **HRSL cumulative swap state**: +3.2M Toffoli, dead end.
- **Toom-3 / Fermat / Edwards-coord swap**: analyzed and rejected.

## Session-scale wins still possible (~50-200q, tens-of-k Toffoli)
- **In-place step4 (eliminate tmp via Gidney measurement-AND)**: -256q at +~800k Toffoli. Needs careful HMR matching.
- **Non-fast Cuccaro everywhere at peak**: -255q at +~300k Toffoli. Needs unified fwd/bwd variants.
- **Asymmetric pair iter tuning**: probably tapped out at 408/405.

## Latent bug notes
- **bulk_prefix_backward r[255]=1** bug was fixed in commit 351c0f7 (2026-04-23).
- **HMR ID-reorder sensitivity**: some phase corrections still depend on specific qubit-ID RNG alignment. Not currently manifesting, but fragile. Investigate if hit again.
