//! Dump the full op stream and the CCX/CCZ gate stream, for stream archaeology.
//! Run with SUB4_APPLY_STRIP=0 to get the UNSTRIPPED stream (what the census sees).
//!
//! This is the instrument behind `docs/census-stream-provenance.md`. It is a cargo
//! *example*, not a bin, so it can be dropped into `examples/` in a detached worktree
//! at any historical commit and built there without touching Cargo.toml:
//!
//!     mkdir -p examples && cp tools/census/dump_gates.rs examples/
//!     cargo build --release --offline --example dump_gates
//!     SUB4_APPLY_STRIP=0 ./target/release/examples/dump_gates out
//!
//! Pass "-" as the prefix to print the summary counts only and skip the ~950 MB of
//! output files -- that is the form the 18-commit stream walk uses.
//!
//! Gate on it before trusting a dump: replaying the `apply_deep_strip_identity`
//! occupancy tripwire against the head dump must reproduce `build_circuit` exactly
//! (12,292 dead accepted / 251 stale, 3,923 downgrades / 0 stale).
//!
//! Outputs (path prefix from argv[1], default "dump"):
//!   <prefix>.gates.tsv  one line per CCX/CCZ, in stream order:
//!       opidx  kind  c2  c1  t  cond  ordinal  occupancy
//!   <prefix>.ops.tsv    one line per op, in stream order:
//!       kind  q_c2  q_c1  q_t  c_t  c_cond  r_t
//!   stdout: summary counts

use quantum_ecc::circuit::Op;
use quantum_ecc::point_add;
use std::collections::HashMap;
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let prefix = args.get(1).cloned().unwrap_or_else(|| "dump".to_string());

    let ops: Vec<Op> = point_add::build();
    let write_files = prefix != "-";

    type Tup = (u8, u64, u64, u64, u64);
    let mut occ: HashMap<Tup, u32> = HashMap::new();
    for op in &ops {
        let kb = op.kind as u8;
        if kb == 13 || kb == 14 {
            *occ.entry((kb, op.q_control2.0, op.q_control1.0, op.q_target.0, op.c_condition.0))
                .or_insert(0) += 1;
        }
    }

    let mut ngates = 0usize;
    if write_files {
        let mut ord: HashMap<Tup, u32> = HashMap::new();
        let f = std::fs::File::create(format!("{prefix}.gates.tsv")).unwrap();
        let mut w = std::io::BufWriter::new(f);
        for (i, op) in ops.iter().enumerate() {
            let kb = op.kind as u8;
            if kb != 13 && kb != 14 {
                continue;
            }
            let tup = (kb, op.q_control2.0, op.q_control1.0, op.q_target.0, op.c_condition.0);
            let o = ord.entry(tup).or_insert(0);
            writeln!(
                w,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                i, kb, tup.1, tup.2, tup.3, tup.4, *o, occ[&tup]
            )
            .unwrap();
            *o += 1;
            ngates += 1;
        }
    } else {
        ngates = ops
            .iter()
            .filter(|op| op.kind as u8 == 13 || op.kind as u8 == 14)
            .count();
    }

    if write_files {
        let f = std::fs::File::create(format!("{prefix}.ops.tsv")).unwrap();
        let mut w = std::io::BufWriter::new(f);
        for op in ops.iter() {
            writeln!(
                w,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                op.kind as u8,
                op.q_control2.0,
                op.q_control1.0,
                op.q_target.0,
                op.c_target.0,
                op.c_condition.0,
                op.r_target.0
            )
            .unwrap();
        }
    }

    let mut kinds: HashMap<u8, usize> = HashMap::new();
    for op in &ops {
        *kinds.entry(op.kind as u8).or_insert(0) += 1;
    }
    let mut ks: Vec<_> = kinds.into_iter().collect();
    ks.sort();
    println!("DUMP ops={} gates={} distinct_tuples={}", ops.len(), ngates, occ.len());
    for (k, n) in ks {
        println!("KIND\t{}\t{}", k, n);
    }
}
