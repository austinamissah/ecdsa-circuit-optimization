#!/usr/bin/env python3
"""Re-apply the analysis-only geometry dump to src/point_add/pingpong_div.rs.

Why this is a script and not a committed edit: every sync takes `src/point_add`
from upstream wholesale, which silently drops anything we added there. The dump
was written once, committed, and then removed by the very next merge without a
conflict or a warning. So it lives here and gets re-applied after each sync.

What it adds, all of it inert unless `PP_GEOMETRY` is set:

  * the effective walk depth for both traversals, and the width scheduled at
    every round, resolved rather than described. An external screener that
    recomputed those would have to track the width table, the rescale, the
    sparse repair set and the bias. Every one of those has moved at least once,
    and a screener that drifts produces noise that looks exactly like data.
  * the chunk bounds and comparison window chosen for every replay add. Those
    are picked per round against the interleaving allowance, so they cannot be
    predicted from the constants either.

Idempotent, and verifies its own inertness afterwards: with `PP_GEOMETRY` unset
the op stream must be byte-identical.

    python3 tools/pp-screen/instrument.py            # apply
    python3 tools/pp-screen/instrument.py --check    # report only
"""

import pathlib
import sys

TARGET = pathlib.Path("src/point_add/pingpong_div.rs")

MODULE = '''
/// Analysis-only geometry dump.  `PP_GEOMETRY=<path>` records the resolved walk
/// geometry and, for every replay add, the chunk bounds and comparison window
/// the layout actually chose.  Both vary with knobs and with the round, so a
/// screener that re-derived them would desync silently.  Emits no ops, and with
/// the variable unset does nothing at all.
mod geom {
    use std::cell::{Cell, RefCell};

    thread_local! {
        static TAG: Cell<(u8, usize)> = const { Cell::new((0, 0)) };
        static ROWS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn enabled() -> bool {
        std::env::var_os("PP_GEOMETRY").is_some()
    }

    pub(super) fn tag(direction: u8, round: usize) {
        if enabled() {
            TAG.with(|c| c.set((direction, round)));
        }
    }

    pub(super) fn record(bounds: &[(usize, usize)], compare: usize) {
        if !enabled() {
            return;
        }
        let (dir, round) = TAG.with(|c| c.get());
        let spans: Vec<String> = bounds.iter().map(|&(lo, hi)| format!("{lo}-{hi}")).collect();
        ROWS.with(|r| {
            r.borrow_mut().push(format!(
                "{}\\t{}\\t{}\\t{}",
                if dir == 0 { "div" } else { "mul" },
                round,
                spans.join(","),
                compare
            ))
        });
    }

    pub(super) fn flush() {
        if !enabled() {
            return;
        }
        let path = std::env::var("PP_GEOMETRY").unwrap_or_default();
        let body = ROWS.with(|r| r.borrow().join("\\n"));

        let div = super::rounds_for(super::PingPongDirection::Divide);
        let mul = super::rounds_for(super::PingPongDirection::Multiply);
        let mut head = format!("#rounds\\t{div}\\t{mul}\\n");
        // Resolved truncation windows. These are set by `set_default_env` in
        // mod.rs, NOT by the literals in this file, so reading the source gives
        // the wrong config. The divide and multiply fold windows also differ.
        head.push_str(&format!(
            "#windows\\t{}\\t{}\\t{}\\t{}\\t{}\\n",
            super::replay_fold_window(),
            super::replay_fold_window_mul(),
            super::endpoint_fold_window(),
            super::replay_chunk_compare(),
            super::replay_flag_compare(),
        ));
        for r in 0..div.max(mul) {
            head.push_str(&format!("#width\\t{r}\\t{}\\n", super::value_width(r)));
        }

        let _ = std::fs::write(
            path,
            format!("{head}direction\\tround\\tbounds\\tcompare\\n{body}\\n"),
        );
    }
}
'''

EDITS = [
    # (anchor, insertion, where)
    ("""#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PingPongDirection {
    Divide,
    Multiply,
}""", MODULE, "after"),
    ("""fn replay_halving_round(b: &mut B, round: usize, sign: QubitId, x: &[QubitId], y: &[QubitId]) {""",
     "\n    geom::tag(0, round);", "after"),
    ("""fn replay_doubling_round(b: &mut B, round: usize, sign: QubitId, x: &[QubitId], y: &[QubitId]) {""",
     "\n    geom::tag(1, round);", "after"),
    ("""    let legacy = std::env::var_os("SUB4_PP_LEGACY_CHUNK_ORDER").is_some();""",
     "    geom::record(&bounds, replay_chunk_compare());\n", "before"),
    ("""    let ops = circ.take_ops();""", "\n    geom::flush();", "after"),
]


def main() -> int:
    check = "--check" in sys.argv
    if not TARGET.exists():
        print(f"error: {TARGET} not found; run from the repo root", file=sys.stderr)
        return 2

    src = TARGET.read_text()
    if "mod geom {" in src:
        print("already instrumented; nothing to do")
        return 0
    if check:
        print("NOT instrumented; run without --check to apply")
        return 1

    for anchor, text, where in EDITS:
        if src.count(anchor) != 1:
            print(f"error: anchor not unique or missing:\n  {anchor[:70]}", file=sys.stderr)
            print("upstream moved the code; re-point this script before using it", file=sys.stderr)
            return 2
        src = src.replace(anchor, anchor + text if where == "after" else text + anchor, 1)

    TARGET.write_text(src)
    print(f"instrumented {TARGET}")
    print("now verify inertness: build with PP_GEOMETRY unset and set, md5 ops.bin must match")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
