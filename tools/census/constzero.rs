//! Stream-agnostic dead-gate certification by constant propagation.
//!
//! The deep strip and the census both certify gates dead *statistically* — "never
//! fired in N sampled inputs". [`../../docs/fire-vs-charge-cross-census.md`](../../docs/fire-vs-charge-cross-census.md)
//! showed the limit of that: 46,134 gates fire on none of the 9,024 official shots,
//! but "never fired on this draw" is a claim about one draw, and deleting a gate
//! re-rolls every shot (`eval_circuit.rs:204`). Such a certificate cannot survive
//! its own use.
//!
//! This instrument asks the structural question instead: **is a control provably
//! zero at that point in the stream, for every input?** A CCX whose control is
//! constant-zero never fires on any input whatsoever, so deleting it is an exact
//! identity — no sampling, no stream dependence, no λ cost.
//!
//! ## The lattice
//!
//! Each qubit and classical bit is tracked as `Zero`, `One`, or `Unknown`.
//! `eval_circuit` calls `clear_for_shot()` (all qubits and bits to 0) and then writes
//! only the four input registers, so the analysis starts from a strong and *sound*
//! initial state: **every non-register qubit is provably Zero**, and register qubits
//! are Unknown.
//!
//! Conditions are tracked as `AllOnes` / `AllZeros` / `Mixed` over the shot lanes,
//! combining the `PushCondition` stack with the op's own `c_condition`. A write under
//! a `Mixed` condition lands on some lanes and not others, so its target degrades to
//! `Unknown` — that is the conservative direction, and it is what keeps the
//! certificate sound rather than merely plausible.
//!
//! ## What is certified
//!
//! - `CCX(c2, c1, t)` fires on `cond & q(c1) & q(c2)`. If either control is `Zero`,
//!   it can never fire.
//! - `CCZ(c2, c1, t)` fires on `cond & q(t) & q(c1) & q(c2)` — the target is part of
//!   the effective condition — so `Zero` on any of the three certifies it.
//!
//! ## Gate on the instrument
//!
//! A wrong certificate deletes a live gate and destroys the circuit, so this is
//! checked against the measured fire counts rather than trusted: pass
//! `--check <hot.tsv>` (from `hotness.rs`) and **every certified gate must have
//! `fire == 0` in the 9,024-shot measurement.** A certified gate that fired is a
//! refutation of the analysis, and the tool exits non-zero. Necessary, not
//! sufficient — but it is the strongest available cross-check, and it is the same
//! discipline `hotness.rs` is held to.
//!
//! Usage:
//!     mkdir -p examples && cp tools/census/constzero.rs examples/
//!     cargo build --release --offline --example constzero
//!     ./target/release/examples/constzero --check /tmp/head.hot.tsv --out /tmp/certified.tsv

use quantum_ecc::circuit::{analyze_ops, Op, OperationType, QubitOrBit, NO_BIT};
use quantum_ecc::point_add;
use std::collections::{HashMap, HashSet};
use std::io::Write;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum L {
    Zero,
    One,
    Unknown,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum C {
    AllOnes,
    AllZeros,
    Mixed,
}

fn flip(v: L) -> L {
    match v {
        L::Zero => L::One,
        L::One => L::Zero,
        L::Unknown => L::Unknown,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut check: Option<String> = None;
    let mut out: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--check" => {
                check = Some(args[i + 1].clone());
                i += 2;
            }
            "--out" => {
                out = Some(args[i + 1].clone());
                i += 2;
            }
            _ => i += 1,
        }
    }

    let ops: Vec<Op> = point_add::build();
    let (total_qubits, num_bits, _n, regs) = analyze_ops(ops.iter());
    eprintln!(
        "ops={} qubits={} bits={}",
        ops.len(),
        total_qubits,
        num_bits
    );

    // Initial state: clear_for_shot zeroes everything; only the four input
    // registers are then written, so every other qubit is provably Zero.
    let mut q = vec![L::Zero; total_qubits as usize];
    let mut b = vec![L::Zero; num_bits as usize];
    let mut input_q = 0usize;
    for reg in regs.iter().take(4) {
        for qb in reg {
            match *qb {
                QubitOrBit::Qubit(id) => {
                    q[id.0 as usize] = L::Unknown;
                    input_q += 1;
                }
                QubitOrBit::Bit(id) => b[id.0 as usize] = L::Unknown,
            }
        }
    }
    eprintln!("input register qubits marked Unknown: {input_q}");

    let mut stack: Vec<C> = Vec::new();
    let mut base = C::AllOnes;

    let mut certified: Vec<(usize, u8, u64, u64, u64, &'static str)> = Vec::new();
    let mut dead_block_gates = 0usize;
    // non-vacuity diagnostics: a "0 certified" result is only informative if the
    // lattice actually carried information past the start of the stream.
    let mut ctrl_both_unknown = 0usize;
    let mut ctrl_some_one = 0usize;
    let mut exact_ops = 0usize;
    let mut mixed_ops = 0usize;
    let mut checkpoints: Vec<(usize, usize, usize, usize)> = Vec::new();
    let cps: Vec<usize> = vec![
        ops.len() / 100,
        ops.len() / 10,
        ops.len() / 2,
        ops.len() - 1,
    ];

    for (idx, op) in ops.iter().enumerate() {
        if cps.contains(&idx) {
            let z = q.iter().filter(|v| **v == L::Zero).count();
            let o = q.iter().filter(|v| **v == L::One).count();
            let u = q.iter().filter(|v| **v == L::Unknown).count();
            checkpoints.push((idx, z, o, u));
        }
        // effective condition for this op
        let eff = if op.c_condition == NO_BIT {
            base
        } else {
            match b[op.c_condition.0 as usize] {
                L::One => base,
                L::Zero => C::AllZeros,
                L::Unknown => {
                    if base == C::AllZeros {
                        C::AllZeros
                    } else {
                        C::Mixed
                    }
                }
            }
        };
        let exact = eff == C::AllOnes; // effect applies on every lane
        let nothing = eff == C::AllZeros; // effect applies on no lane
        if exact { exact_ops += 1 } else if !nothing { mixed_ops += 1 }
        if matches!(op.kind, OperationType::CCX | OperationType::CCZ) {
            let c1 = q[op.q_control1.0 as usize];
            let c2 = q[op.q_control2.0 as usize];
            if c1 == L::Unknown && c2 == L::Unknown { ctrl_both_unknown += 1 }
            if c1 == L::One || c2 == L::One { ctrl_some_one += 1 }
        }

        match op.kind {
            OperationType::CCX => {
                let c1 = q[op.q_control1.0 as usize];
                let c2 = q[op.q_control2.0 as usize];
                if nothing {
                    dead_block_gates += 1;
                } else if c1 == L::Zero || c2 == L::Zero {
                    certified.push((
                        idx,
                        13,
                        op.q_control2.0,
                        op.q_control1.0,
                        op.q_target.0,
                        if c1 == L::Zero { "c1=0" } else { "c2=0" },
                    ));
                    // target unchanged
                } else if exact && c1 == L::One && c2 == L::One {
                    let t = op.q_target.0 as usize;
                    q[t] = flip(q[t]);
                } else {
                    q[op.q_target.0 as usize] = L::Unknown;
                }
            }
            OperationType::CCZ => {
                let c1 = q[op.q_control1.0 as usize];
                let c2 = q[op.q_control2.0 as usize];
                let t = q[op.q_target.0 as usize];
                if nothing {
                    dead_block_gates += 1;
                } else if c1 == L::Zero || c2 == L::Zero || t == L::Zero {
                    certified.push((
                        idx,
                        14,
                        op.q_control2.0,
                        op.q_control1.0,
                        op.q_target.0,
                        if t == L::Zero {
                            "t=0"
                        } else if c1 == L::Zero {
                            "c1=0"
                        } else {
                            "c2=0"
                        },
                    ));
                }
                // CCZ writes no qubit
            }
            OperationType::CX => {
                if nothing {
                } else {
                    let c = q[op.q_control1.0 as usize];
                    if c == L::Zero {
                        // no effect
                    } else if exact && c == L::One {
                        let t = op.q_target.0 as usize;
                        q[t] = flip(q[t]);
                    } else {
                        q[op.q_target.0 as usize] = L::Unknown;
                    }
                }
            }
            OperationType::X => {
                if nothing {
                } else if exact {
                    let t = op.q_target.0 as usize;
                    q[t] = flip(q[t]);
                } else {
                    q[op.q_target.0 as usize] = L::Unknown;
                }
            }
            OperationType::Swap => {
                if nothing {
                } else if exact {
                    let a = op.q_control1.0 as usize;
                    let t = op.q_target.0 as usize;
                    q.swap(a, t);
                } else {
                    q[op.q_control1.0 as usize] = L::Unknown;
                    q[op.q_target.0 as usize] = L::Unknown;
                }
            }
            OperationType::Hmr => {
                // c_target gets random bits on conditioned lanes; q_target is zeroed there.
                if nothing {
                } else {
                    b[op.c_target.0 as usize] = L::Unknown;
                    q[op.q_target.0 as usize] = if exact { L::Zero } else { L::Unknown };
                }
            }
            OperationType::R => {
                if nothing {
                } else {
                    q[op.q_target.0 as usize] = if exact { L::Zero } else { L::Unknown };
                }
            }
            OperationType::BitInvert => {
                if nothing {
                } else if exact {
                    let t = op.c_target.0 as usize;
                    b[t] = flip(b[t]);
                } else {
                    b[op.c_target.0 as usize] = L::Unknown;
                }
            }
            OperationType::BitStore0 => {
                if nothing {
                } else {
                    b[op.c_target.0 as usize] = if exact { L::Zero } else { L::Unknown };
                }
            }
            OperationType::BitStore1 => {
                if nothing {
                } else {
                    b[op.c_target.0 as usize] = if exact { L::One } else { L::Unknown };
                }
            }
            OperationType::PushCondition => {
                stack.push(base);
                base = match b[op.c_condition.0 as usize] {
                    L::One => base,
                    L::Zero => C::AllZeros,
                    L::Unknown => {
                        if base == C::AllZeros {
                            C::AllZeros
                        } else {
                            C::Mixed
                        }
                    }
                };
            }
            OperationType::PopCondition => {
                if let Some(v) = stack.pop() {
                    base = v;
                }
            }
            OperationType::CZ
            | OperationType::Z
            | OperationType::Neg
            | OperationType::AppendToRegister
            | OperationType::Register
            | OperationType::DebugPrint => {}
        }
    }

    let ngates = ops
        .iter()
        .filter(|o| matches!(o.kind, OperationType::CCX | OperationType::CCZ))
        .count();
    println!("--- non-vacuity diagnostics ---");
    println!("ops with provably-AllOnes condition : {exact_ops}");
    println!("ops with Mixed condition            : {mixed_ops}");
    println!("qubit lattice at checkpoints (opidx, Zero, One, Unknown):");
    for (i, z, o, u) in &checkpoints {
        println!("  op {i:>9}: Zero={z:<6} One={o:<4} Unknown={u}");
    }
    println!("CCX/CCZ with both controls Unknown  : {ctrl_both_unknown}");
    println!("CCX/CCZ with a provably-One control : {ctrl_some_one}");
    println!("-------------------------------");
    println!("gates={ngates}");
    println!("CERTIFIED constant-zero-control gates: {}", certified.len());
    println!("gates inside provably-dead condition blocks: {dead_block_gates}");
    let mut why: HashMap<&str, usize> = HashMap::new();
    for c in &certified {
        *why.entry(c.5).or_insert(0) += 1;
    }
    for (k, v) in why {
        println!("  reason {k}: {v}");
    }

    // ---- the gate: every certified gate must have fired zero times ----
    if let Some(path) = check {
        let text = std::fs::read_to_string(&path).expect("hot tsv");
        let mut fire: HashMap<usize, u64> = HashMap::new();
        for line in text.lines().skip(1) {
            let p: Vec<&str> = line.split('\t').collect();
            if p.len() >= 8 {
                fire.insert(p[0].parse().unwrap(), p[7].parse().unwrap());
            }
        }
        let mut missing = 0usize;
        let mut violated: Vec<(usize, u64)> = Vec::new();
        for c in &certified {
            match fire.get(&c.0) {
                Some(0) => {}
                Some(f) => violated.push((c.0, *f)),
                None => missing += 1,
            }
        }
        println!(
            "\nCHECK against {path}: {} certified, {} not present in dump, {} VIOLATIONS",
            certified.len(),
            missing,
            violated.len()
        );
        for (i, f) in violated.iter().take(10) {
            println!("  !! opidx {i} certified dead but fired {f} times");
        }
        if !violated.is_empty() {
            eprintln!("ANALYSIS REFUTED — do not use");
            std::process::exit(1);
        }
        println!("CHECK ok: no certified gate fired");

        // how much of the never-fire population does the structural argument explain?
        let never: HashSet<usize> = fire
            .iter()
            .filter(|(_, v)| **v == 0)
            .map(|(k, _)| *k)
            .collect();
        let cert: HashSet<usize> = certified.iter().map(|c| c.0).collect();
        println!(
            "never-fire gates in dump: {}   certified structurally: {}   unexplained: {}",
            never.len(),
            cert.len(),
            never.difference(&cert).count()
        );
    }

    if let Some(o) = out {
        let f = std::fs::File::create(&o).unwrap();
        let mut w = std::io::BufWriter::new(f);
        writeln!(w, "opidx\tkind\tc2\tc1\tt\treason").unwrap();
        for c in &certified {
            writeln!(w, "{}\t{}\t{}\t{}\t{}\t{}", c.0, c.1, c.2, c.3, c.4, c.5).unwrap();
        }
        println!("wrote {o}");
    }
}
