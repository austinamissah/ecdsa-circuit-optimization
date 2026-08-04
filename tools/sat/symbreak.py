#!/usr/bin/env python3
"""Sound symmetry break for the exact-eight joint-codec synthesis CNF.

`06-research-status.md` lists "a machine-checkable symmetry reduction" as the first
of four conditions for reopening unrestricted exact-eight joint synthesis. This is
that reduction.

## What symmetry is left to break

`y5_normalizer_synth.constrain_gate_shape` already breaks the **left/right
commutativity** of each shear's control product, by forcing the two nonconstant
coefficient vectors into numeric order.

**Wire permutation is not a symmetry of this problem.** The 25 input states are
pinned to literal bit patterns and the target table is fixed, so conjugating the
program by an invertible wire map changes the inputs it is required to reproduce.
The free affine output map can absorb such a map on the output side only, which is
not a symmetry of the constraint set. No wire-permutation break is therefore sound
here, and none is emitted.

**Gate order is a symmetry, and it is not broken.** Two shears whose effects
commute may appear in either order, so every solution using k pairwise-commuting
gates is one of k! equivalent solutions the solver may explore separately.

## The break

Shear g acts as `x -> x ^ d * (L(x) & R(x))` with `L(x) = l0 ^ sum_i l_i x_i`.
Applying g1 changes `L2` by `(l2 . d1) * p1`, so g1 and g2 commute **unconditionally**
when all four GF(2) dot products vanish:

    l2.d1 = 0,  r2.d1 = 0,  l1.d2 = 0,  r1.d2 = 0

For each adjacent pair (i, i+1) this emits

    commute(i, i+1)  ->  params(i)  <=_lex  params(i+1)

with `params = [left[0..6], right[0..6], direction[0..5]]` (20 bits, MSB first).
Conditioning the ordering on `commute` is what makes it sound: non-commuting
neighbours keep both orders available, so no solution is excluded.

## Gate on the reduction

Symmetry breaking that removes a solution is worse than useless on an instance whose
whole question is satisfiability. The `--selftest` mode therefore builds the
**exact-nine** instance, which is known SAT (the nine-CCX reference is a witness),
solves it with and without the break, and requires SAT both times. Run it before
trusting any UNSAT that comes back from a broken instance.

Usage:
    python3 tools/sat/symbreak.py --selftest             # required before use
    python3 tools/sat/symbreak.py --gates 8 --out out.cnf
"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

REPRO = Path(__file__).resolve().parents[2] / "src/point_add/memory/repro"
sys.path.insert(0, str(REPRO))

import y5_joint_codec_synth as joint  # noqa: E402
import y5_normalizer_synth as synth  # noqa: E402

WIDTH = synth.WIDTH


class Emitter:
    """Appends clauses over fresh variables numbered above the base CNF."""

    def __init__(self, next_var: int) -> None:
        self.next_var = next_var
        self.clauses: list[list[int]] = []

    def var(self) -> int:
        self.next_var += 1
        return self.next_var

    def clause(self, *lits: int) -> None:
        self.clauses.append(list(lits))

    def const_true(self) -> int:
        v = self.var()
        self.clause(v)
        return v

    def and_(self, a: int, b: int) -> int:
        y = self.var()
        self.clause(-y, a)
        self.clause(-y, b)
        self.clause(y, -a, -b)
        return y

    def xor(self, a: int, b: int) -> int:
        y = self.var()
        self.clause(-y, a, b)
        self.clause(-y, -a, -b)
        self.clause(y, -a, b)
        self.clause(y, a, -b)
        return y

    def xor_chain(self, terms: list[int]) -> int:
        if not terms:
            return -self.const_true()
        acc = terms[0]
        for t in terms[1:]:
            acc = self.xor(acc, t)
        return acc

    def eq(self, a: int, b: int) -> int:
        """y <-> (a == b)"""
        return -self.xor(a, b) if False else self._eqv(a, b)

    def _eqv(self, a: int, b: int) -> int:
        y = self.var()
        self.clause(-y, -a, b)
        self.clause(-y, a, -b)
        self.clause(y, -a, -b)
        self.clause(y, a, b)
        return y


def dot(em: Emitter, coeffs: list[int], direction: list[int]) -> int:
    """GF(2) dot product of a coefficient vector with a direction vector."""
    return em.xor_chain([em.and_(c, d) for c, d in zip(coeffs, direction)])


def params(shear: synth.ShearVariables) -> list[int]:
    return [*shear.left, *shear.right, *shear.direction]


def lex_le_conditional(em: Emitter, cond: int, a: list[int], b: list[int]) -> None:
    """cond -> a <=_lex b, MSB first."""
    prefix_eq = em.const_true()
    for ak, bk in zip(a, b):
        # (cond & prefix_eq & ak) -> bk
        em.clause(-cond, -prefix_eq, -ak, bk)
        prefix_eq = em.and_(prefix_eq, em._eqv(ak, bk))


_CONFIGURED = False


def _configure_once() -> None:
    """`joint.configure_problem` mutates `synth.REFERENCE_CCX_COUNT` to 9, which makes
    `synth.load_reference_ops` fail its own count check on a second call. Configure once."""
    global _CONFIGURED
    if not _CONFIGURED:
        joint.configure_problem()
        _CONFIGURED = True


def build(gates: int, break_symmetry: bool) -> tuple[str, dict]:
    _configure_once()
    cnf, variables, _table = synth.build_problem(
        gates, inputs=list(joint.PAIR_INPUTS), exact=True
    )
    base_vars, base_clauses = cnf.nvars, list(cnf.clauses)
    stats = {"base_variables": base_vars, "base_clauses": len(base_clauses)}

    extra: list[list[int]] = []
    if break_symmetry:
        em = Emitter(base_vars)
        for i in range(len(variables.shears) - 1):
            g1, g2 = variables.shears[i], variables.shears[i + 1]
            l1, r1, d1 = g1.left[1:], g1.right[1:], g1.direction
            l2, r2, d2 = g2.left[1:], g2.right[1:], g2.direction
            zero = [
                -dot(em, l2, d1),
                -dot(em, r2, d1),
                -dot(em, l1, d2),
                -dot(em, r1, d2),
            ]
            commute = em.var()
            # commute <-> all four dot products are zero
            for z in zero:
                em.clause(-commute, z)
            em.clause(commute, *[-z for z in zero])
            lex_le_conditional(em, commute, params(g1), params(g2))
        extra = em.clauses
        stats["symmetry_variables"] = em.next_var - base_vars
        stats["symmetry_clauses"] = len(extra)

    total_vars = base_vars + stats.get("symmetry_variables", 0)
    all_clauses = base_clauses + extra
    stats["variables"] = total_vars
    stats["clauses"] = len(all_clauses)

    lines = [f"c exact-{gates} joint codec, symmetry_break={int(break_symmetry)}"]
    lines.append(f"p cnf {total_vars} {len(all_clauses)}")
    for cl in all_clauses:
        lines.append(" ".join(str(x) for x in cl) + " 0")
    return "\n".join(lines) + "\n", stats


def solve(path: Path, binary: str, timeout: int) -> str:
    try:
        p = subprocess.run([binary, str(path)], capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return "timeout"
    return {10: "SAT", 20: "UNSAT"}.get(p.returncode, f"rc={p.returncode}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--gates", type=int, default=8)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--no-break", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    ap.add_argument("--kissat", default="kissat")
    ap.add_argument("--timeout", type=int, default=900)
    a = ap.parse_args()

    if a.selftest:
        ok = True
        # 1. exact-8 without the break must reproduce the recorded CNF size.
        _, s8 = build(8, False)
        kat = (s8["variables"], s8["clauses"]) == (11416, 54051)
        print(f"KAT exact-8 unbroken: {s8['variables']} vars / {s8['clauses']} clauses "
              f"-> {'PASS' if kat else 'FAIL (expected 11416/54051)'}")
        ok &= kat
        # 2. exact-9 is SAT; it must stay SAT with the break applied.
        import tempfile
        with tempfile.TemporaryDirectory() as td:
            td = Path(td)
            for brk in (False, True):
                text, st = build(9, brk)
                p = td / f"exact9-{int(brk)}.cnf"
                p.write_text(text)
                verdict = solve(p, a.kissat, a.timeout)
                tag = "with break" if brk else "unbroken"
                print(f"exact-9 {tag:11s}: {st['variables']:6d} vars / {st['clauses']:6d} clauses"
                      f" -> {verdict}")
                ok &= verdict == "SAT"
        print("SELFTEST", "PASS" if ok else "FAIL")
        return 0 if ok else 1

    text, stats = build(a.gates, not a.no_break)
    if a.out:
        a.out.write_text(text)
    print(stats)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
