# λ levers — what each one actually buys, measured on `801dd20`

> **Measured 2026-08-03** on the hardware in [`lambda-measurement.md`](lambda-measurement.md).
> Circuit head `801dd20`; the two source changes described below are in this branch and are
> byte-identical to it at the shipped knob set.
>
> **Classical channel only.** Every λ figure here is `λ_classical`, from
> [`../tools/lam-screen/`](../tools/lam-screen/). It is not λ_total, and a screen-clean nonce is a
> candidate, not a seed — see [`upstream-search-economics.md`](upstream-search-economics.md).

`lambda-measurement.md` establishes that λ reduction is the gating project: at λ_total = 20.04 a
clean seed costs ~5e8 harness trials, and every score-lowering change re-rolls the seed. This
document measures what the individual λ sources are actually worth on the current head, and what
each costs on the score axis. `memory/02-lambda.md` priced four of them by classical emulation at
`ITERS = 258`; this is a direct measurement at the shipped `ITERS = 261`.

PLACEHOLDER-RESULTS

## Method

PLACEHOLDER-METHOD
