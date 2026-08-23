//! Stream-agnostic dead-gate certification by **affine-relation analysis** over GF(2).
//!
//! [`../../docs/stream-agnostic-certification.md`](../../docs/stream-agnostic-certification.md)
//! showed that a *constant* lattice certifies nothing on the final stream: all
//! 1,343,361 CCX/CCZ have both controls Unknown, because `build()`'s own CONSTPROP
//! pass already harvested every constant-control gate. This is the next rung.
//!
//! ## The idea
//!
//! A `CCX(c2, c1, t)` fires on `cond & q(c1) & q(c2)`. It can never fire, on any
//! input, on any draw, whenever `q(c1) & q(c2) = 0` identically. Two cases are
//! decidable without knowing the values:
//!
//!   - either control is the **constant 0** (what `constzero.rs` tests), or
//!   - the controls are **complementary**: `q(c1) = ¬q(c2)`, so their AND is
//!     identically zero.
//!
//! Complementary flag pairs are exactly what binary-GCD sign/branch logic produces,
//! which is why this is the promising rung.
//!
//! ## Representation
//!
//! Every qubit and classical bit carries an **affine form over GF(2)**:
//!
//!     value = constant  XOR  (XOR of some set of atoms)
//!
//! An *atom* is a fresh symbolic unknown, minted whenever a value stops being an
//! affine function of what came before, the 512 input-register qubits at op 0, the
//! target of a genuinely nonlinear `CCX`, an `Hmr` measurement outcome, or any write
//! under a partial (`Mixed`) condition.
//!
//! Rather than materialise each form as a basis vector, each atom gets a random
//! `u128` and a form is stored as the **XOR of its atoms' tags** plus a constant bit.
//! XOR-hashing is exact for GF(2) vector spaces: two forms are equal iff their tags
//! and constants match, and `tag == 0` iff the form is a constant. Comparison is one
//! `u128` compare instead of a set operation, and a false equality needs a 128-bit
//! collision.
//!
//! ## Exact propagation
//!
//! Under a provably-`AllOnes` condition these are exact, not approximations:
//!
//!   - `X(t)`:        `const(t) ^= 1`
//!   - `CX(c, t)`:    `form(t) ^= form(c)`
//!   - `Swap(a, t)`:  exchange forms
//!   - `R`/`Hmr(t)`:  `form(t) = 0`
//!
//! `CCX` is nonlinear, but four of its cases stay affine or decide the gate outright:
//!
//!   - `form(c1)` or `form(c2)` is constant 0        -> **never fires** (certificate)
//!   - `form(c1) = ¬form(c2)`                        -> **never fires** (certificate)
//!   - `form(c1)` is constant 1                      -> `form(t) ^= form(c2)`, exact
//!   - `form(c1) = form(c2)`                         -> `form(t) ^= form(c1)`, exact
//!   - otherwise                                     -> `form(t)` gets a fresh atom
//!
//! `CCZ` fires on `cond & q(t) & q(c1) & q(c2)`, so a constant-0 or a complementary
//! pair anywhere among those three certifies it.
//!
//! Certification does **not** depend on the condition: if `q(c1) & q(c2) = 0` on
//! every lane then the gate never fires whatever the condition mask is. Conditions
//! only affect whether *propagation* stays exact.
//!
//! ## Validate before trusting
//!
//! A false certificate deletes a live gate and destroys the circuit. `--check
//! <hot.tsv>` requires that **every certified gate has `fire == 0`** in the measured
//! 9,024-shot dump from `hotness.rs`; any certified gate that fired is a soundness
//! bug and the tool exits non-zero. This is necessary, not sufficient, the whole
//! point of a structural certificate is that it claims more than the draw shows, but
//! a certificate that contradicts the draw is definitively wrong.
//!
//! Usage:
//!     mkdir -p examples && cp tools/census/affine.rs examples/
//!     cargo build --release --offline --example affine
//!     ./target/release/examples/affine --check /tmp/head.hot.tsv --out /tmp/affine-certified.tsv

use quantum_ecc::circuit::{analyze_ops, Op, OperationType, QubitOrBit, NO_BIT};
// `point_add` is no longer a module of the shared `quantum_ecc` library. It is
// compiled directly into each binary via `#[path]`, because that library is also
// linked into the trusted `eval_circuit`, where an `.init_array` constructor in
// contestant code would run before `main` and could forge `score.json`. Analysis
// tools take the same route `build_circuit` does, which keeps every existing
// `crate::point_add` path inside `src/point_add/**` resolving as before.
#[allow(dead_code)]
#[path = "../point_add/mod.rs"]
mod point_add;

// Root bindings backing the `crate::{circuit,sim,weierstrass_elliptic_curve}`
// paths used inside `src/point_add/**`.
#[allow(unused_imports)]
use quantum_ecc::{circuit, sim, weierstrass_elliptic_curve};
use std::collections::{HashMap, HashSet};
use std::io::Write;

/// An affine form over GF(2): `constant XOR (XOR of atoms)`, atoms XOR-hashed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct F {
    tag: u128,
    c: bool,
}

impl F {
    const ZERO: F = F { tag: 0, c: false };
    const ONE: F = F { tag: 0, c: true };
    fn xor(self, o: F) -> F {
        F {
            tag: self.tag ^ o.tag,
            c: self.c ^ o.c,
        }
    }
    fn is_const(self) -> bool {
        self.tag == 0
    }
    fn is_zero(self) -> bool {
        self.tag == 0 && !self.c
    }
    /// self == !other  <=>  same atoms, opposite constant
    fn is_complement_of(self, o: F) -> bool {
        self.tag == o.tag && self.c != o.c
    }
}

struct Atoms {
    state: u64,
    minted: u64,
}

impl Atoms {
    fn new() -> Self {
        Atoms {
            state: 0x9E3779B97F4A7C15,
            minted: 0,
        }
    }
    fn next_u64(&mut self) -> u64 {
        // splitmix64, deterministic, so runs are reproducible
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    fn fresh(&mut self) -> F {
        self.minted += 1;
        let hi = self.next_u64() as u128;
        let lo = self.next_u64() as u128;
        let tag = (hi << 64) | lo;
        F {
            tag: if tag == 0 { 1 } else { tag },
            c: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum C {
    AllOnes,
    AllZeros,
    Mixed,
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

    let positive_control = args.iter().any(|a| a == "--positive-control");

    // (ops, qubits, bits, qubits that start Unknown, bits that start Unknown)
    let (ops, total_qubits, num_bits, unknown_q, unknown_b): (Vec<Op>, u64, u64, Vec<u64>, Vec<u64>) =
        if positive_control {
            // Positive control: a hand-built stream containing a KNOWN complementary
            // control pair, so that a zero result on the real circuit can be
            // distinguished from an analysis that cannot see such a pair at all.
            //
            //   q0            : free input
            //   CX(q0 -> q1)  : q1 = q0        (q1 starts provably 0)
            //   X(q1)         : q1 = !q0
            //   CCX(q1,q0,q2) : controls q0 and !q0  -> MUST be certified
            //   CCX(q3,q0,q4) : q3 an unrelated input -> must NOT be certified
            //   CCZ(q1,q0,q5) : complementary pair again -> MUST be certified
            let mk = |kind, c2: u64, c1: u64, t: u64| {
                let mut o = Op::empty();
                o.kind = kind;
                o.q_control2 = quantum_ecc::circuit::QubitId(c2);
                o.q_control1 = quantum_ecc::circuit::QubitId(c1);
                o.q_target = quantum_ecc::circuit::QubitId(t);
                o
            };
            let mut v: Vec<Op> = Vec::new();
            v.push(mk(OperationType::CX, 0, 0, 1));
            v.push(mk(OperationType::X, 0, 0, 1));
            v.push(mk(OperationType::CCX, 1, 0, 2));
            v.push(mk(OperationType::CCX, 3, 0, 4));
            v.push(mk(OperationType::CCZ, 1, 0, 5));
            (v, 6, 1, vec![0, 3], vec![])
        } else {
            let o: Vec<Op> = point_add::build();
            let (tq, nb, _n, regs) = analyze_ops(o.iter());
            let mut uq = Vec::new();
            let mut ub = Vec::new();
            for reg in regs.iter().take(4) {
                for qb in reg {
                    match *qb {
                        QubitOrBit::Qubit(id) => uq.push(id.0),
                        QubitOrBit::Bit(id) => ub.push(id.0),
                    }
                }
            }
            (o, tq, nb, uq, ub)
        };
    eprintln!("ops={} qubits={} bits={}", ops.len(), total_qubits, num_bits);

    let mut at = Atoms::new();
    // hash-consed AND terms: two CCX with identical control forms share one atom,
    // so the representation is an XOR-of-AND graph rather than XOR-of-opaque-atoms.
    let mut and_memo: HashMap<(u128, bool, u128, bool), F> = HashMap::new();
    let mut and_hits = 0u64;
    // clear_for_shot zeroes everything; only the four input registers are written.
    let mut q = vec![F::ZERO; total_qubits as usize];
    let mut b = vec![F::ZERO; num_bits as usize];
    for id in &unknown_q {
        q[*id as usize] = at.fresh();
    }
    for id in &unknown_b {
        b[*id as usize] = at.fresh();
    }

    let mut stack: Vec<C> = Vec::new();
    let mut base = C::AllOnes;

    let mut certified: Vec<(usize, u8, u64, u64, u64, &'static str)> = Vec::new();
    // diagnostics
    let (mut n_affine_eq, mut n_affine_c1one, mut n_fresh, mut n_ccx) = (0u64, 0u64, 0u64, 0u64);
    let mut ctrl_related = 0u64;

    for (idx, op) in ops.iter().enumerate() {
        let eff = if op.c_condition == NO_BIT {
            base
        } else {
            let f = b[op.c_condition.0 as usize];
            if f.is_const() {
                if f.c {
                    base
                } else {
                    C::AllZeros
                }
            } else if base == C::AllZeros {
                C::AllZeros
            } else {
                C::Mixed
            }
        };
        let exact = eff == C::AllOnes;
        let nothing = eff == C::AllZeros;

        match op.kind {
            OperationType::CCX | OperationType::CCZ => {
                let c1 = q[op.q_control1.0 as usize];
                let c2 = q[op.q_control2.0 as usize];
                let t = q[op.q_target.0 as usize];
                let ccz = op.kind == OperationType::CCZ;
                if op.kind == OperationType::CCX {
                    n_ccx += 1;
                }
                if c1.tag == c2.tag {
                    ctrl_related += 1;
                }

                // --- certificates: the effective product is identically zero ---
                let mut reason: Option<&'static str> = None;
                if c1.is_zero() {
                    reason = Some("c1=0");
                } else if c2.is_zero() {
                    reason = Some("c2=0");
                } else if c1.is_complement_of(c2) {
                    reason = Some("c1=!c2");
                } else if ccz {
                    if t.is_zero() {
                        reason = Some("t=0");
                    } else if t.is_complement_of(c1) {
                        reason = Some("t=!c1");
                    } else if t.is_complement_of(c2) {
                        reason = Some("t=!c2");
                    }
                }

                if let Some(r) = reason {
                    if !nothing {
                        certified.push((
                            idx,
                            if ccz { 14 } else { 13 },
                            op.q_control2.0,
                            op.q_control1.0,
                            op.q_target.0,
                            r,
                        ));
                    }
                    continue; // provably no effect on the target either way
                }

                if ccz {
                    continue; // phase only, writes no qubit
                }
                // --- propagation for CCX ---
                if nothing {
                    // no effect on any lane
                } else if exact && c1.is_const() && c1.c {
                    q[op.q_target.0 as usize] = t.xor(c2); // c1 == 1
                    n_affine_c1one += 1;
                } else if exact && c2.is_const() && c2.c {
                    q[op.q_target.0 as usize] = t.xor(c1);
                    n_affine_c1one += 1;
                } else if exact && c1 == c2 {
                    q[op.q_target.0 as usize] = t.xor(c1); // x & x = x
                    n_affine_eq += 1;
                } else if exact {
                    // t ^= (c1 & c2), with the AND term hash-consed on its operands
                    let key = if (c1.tag, c1.c) <= (c2.tag, c2.c) {
                        (c1.tag, c1.c, c2.tag, c2.c)
                    } else {
                        (c2.tag, c2.c, c1.tag, c1.c)
                    };
                    let a = match and_memo.get(&key) {
                        Some(f) => {
                            and_hits += 1;
                            *f
                        }
                        None => {
                            let f = at.fresh();
                            and_memo.insert(key, f);
                            f
                        }
                    };
                    q[op.q_target.0 as usize] = t.xor(a);
                    n_fresh += 1;
                } else {
                    q[op.q_target.0 as usize] = at.fresh();
                    n_fresh += 1;
                }
            }
            OperationType::CX => {
                if !nothing {
                    let c = q[op.q_control1.0 as usize];
                    if c.is_zero() {
                        // no effect
                    } else if exact {
                        let t = op.q_target.0 as usize;
                        q[t] = q[t].xor(c);
                    } else {
                        q[op.q_target.0 as usize] = at.fresh();
                    }
                }
            }
            OperationType::X => {
                if !nothing {
                    let t = op.q_target.0 as usize;
                    q[t] = if exact { q[t].xor(F::ONE) } else { at.fresh() };
                }
            }
            OperationType::Swap => {
                if !nothing {
                    if exact {
                        q.swap(op.q_control1.0 as usize, op.q_target.0 as usize);
                    } else {
                        q[op.q_control1.0 as usize] = at.fresh();
                        q[op.q_target.0 as usize] = at.fresh();
                    }
                }
            }
            OperationType::Hmr => {
                if !nothing {
                    b[op.c_target.0 as usize] = at.fresh();
                    q[op.q_target.0 as usize] = if exact { F::ZERO } else { at.fresh() };
                }
            }
            OperationType::R => {
                if !nothing {
                    q[op.q_target.0 as usize] = if exact { F::ZERO } else { at.fresh() };
                }
            }
            OperationType::BitInvert => {
                if !nothing {
                    let t = op.c_target.0 as usize;
                    b[t] = if exact { b[t].xor(F::ONE) } else { at.fresh() };
                }
            }
            OperationType::BitStore0 => {
                if !nothing {
                    b[op.c_target.0 as usize] = if exact { F::ZERO } else { at.fresh() };
                }
            }
            OperationType::BitStore1 => {
                if !nothing {
                    b[op.c_target.0 as usize] = if exact { F::ONE } else { at.fresh() };
                }
            }
            OperationType::PushCondition => {
                stack.push(base);
                let f = b[op.c_condition.0 as usize];
                base = if f.is_const() {
                    if f.c {
                        base
                    } else {
                        C::AllZeros
                    }
                } else if base == C::AllZeros {
                    C::AllZeros
                } else {
                    C::Mixed
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

    println!("--- affine propagation diagnostics ---");
    println!("atoms minted                        : {}", at.minted);
    println!("CCX total                           : {n_ccx}");
    println!("CCX kept affine (a control == 1)    : {n_affine_c1one}");
    println!("CCX kept affine (controls equal)    : {n_affine_eq}");
    println!("CCX handled as a hash-consed AND    : {n_fresh}");
    println!("  of which reused an existing AND   : {and_hits}");
    println!("distinct AND terms                  : {}", and_memo.len());
    println!("CCX/CCZ whose controls share a tag  : {ctrl_related}");
    println!("--------------------------------------");
    println!("CERTIFIED never-firing gates        : {}", certified.len());
    let mut why: HashMap<&str, usize> = HashMap::new();
    for c in &certified {
        *why.entry(c.5).or_insert(0) += 1;
    }
    let mut ws: Vec<_> = why.into_iter().collect();
    ws.sort();
    for (k, v) in ws {
        println!("  reason {k}: {v}");
    }

    if positive_control {
        let ok = certified.len() == 2
            && certified.iter().any(|c| c.0 == 2 && c.5 == "c1=!c2")
            && certified.iter().any(|c| c.0 == 4);
        println!("\nPOSITIVE CONTROL: expected 2 certificates (op 2 CCX, op 4 CCZ), got {}", certified.len());
        for c in &certified {
            println!("  op {} kind {} reason {}", c.0, c.1, c.5);
        }
        println!("POSITIVE CONTROL {}", if ok { "PASS" } else { "FAIL" });
        std::process::exit(if ok { 0 } else { 1 });
    }

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
            "\nCHECK against {path}: {} certified, {} absent from dump, {} VIOLATIONS",
            certified.len(),
            missing,
            violated.len()
        );
        for (i, f) in violated.iter().take(20) {
            println!("  !! opidx {i} certified never-firing but FIRED {f} times");
        }
        if !violated.is_empty() {
            eprintln!("SOUNDNESS BUG, analysis refuted, do not remove anything");
            std::process::exit(1);
        }
        println!("CHECK ok: no certified gate fired on any of the 9,024 shots");

        let never: HashSet<usize> = fire
            .iter()
            .filter(|(_, v)| **v == 0)
            .map(|(k, _)| *k)
            .collect();
        let cert: HashSet<usize> = certified.iter().map(|c| c.0).collect();
        println!(
            "never-fire in dump: {}   certified structurally: {}   still unexplained: {}",
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
