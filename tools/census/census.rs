//! `census` — re-mine the identity-keyed deep strip against the CURRENT stream.
//!
//! Identical in semantics to `tools/nonce-screen/screen.rs`: same seed, same
//! pairs, same simulator, same classical count. The only change is HOW `k*G` is
//! computed. `WeierstrassEllipticCurve::mul` is an affine double-and-add that
//! pays a modular inversion on every one of its ~384 group operations, which
//! measured at 690 us per point — 12.5 s of the screen's 22.3 s per nonce.
//!
//! `FastBase` replaces it with a 8-bit fixed-base window table plus Jacobian
//! accumulation, so the whole scalar multiplication costs 32 mixed additions and
//! exactly one inversion. `--selftest` asserts bit-equality against the library
//! routine over random scalars; the 199-nonce gate re-asserts it end to end.

use quantum_ecc::circuit::{analyze_ops, Op, OperationType, QubitId, QubitOrBit, NO_BIT};
use quantum_ecc::point_add;
use quantum_ecc::sim::Simulator;
use quantum_ecc::weierstrass_elliptic_curve::WeierstrassEllipticCurve;
use ruint::aliases::U256;
use sha3::digest::{ExtendableOutput, Update, XofReader};
use sha3::Shake256;
use std::io::Write;
use std::time::Instant;

const FULL_SHOTS: usize = 9024;
const SHOT_LADDER: [usize; 4] = [512, 2_048, 8_192, 9_024];
const TAIL: usize = 96;

fn secp256k1() -> WeierstrassEllipticCurve {
    WeierstrassEllipticCurve {
        modulus: U256::from_str_radix(
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F",
            16,
        )
        .unwrap(),
        a: U256::from(0),
        b: U256::from(7),
        gx: U256::from_str_radix(
            "79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798",
            16,
        )
        .unwrap(),
        gy: U256::from_str_radix(
            "483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8",
            16,
        )
        .unwrap(),
        order: U256::from_str_radix(
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141",
            16,
        )
        .unwrap(),
    }
}

// ---------------------------------------------------------------------------
// Fast fixed-base scalar multiplication
// ---------------------------------------------------------------------------

/// 8-bit fixed-base comb: `tbl[w][d-1] = d * 256^w * G`, affine, `d` in 1..=255.
struct FastBase {
    m: U256,
    tbl: Vec<[(U256, U256); 255]>,
}

#[inline]
fn sub_m(a: U256, b: U256, m: U256) -> U256 {
    if a >= b { a - b } else { m - (b - a) }
}

impl FastBase {
    fn new(curve: &WeierstrassEllipticCurve) -> Self {
        let m = curve.modulus;
        let mut tbl = Vec::with_capacity(32);
        let mut base = (curve.gx, curve.gy);
        for _ in 0..32 {
            let mut row = [(U256::ZERO, U256::ZERO); 255];
            let mut acc = base;
            row[0] = acc;
            for d in 1..255 {
                acc = curve.add(acc.0, acc.1, base.0, base.1);
                row[d] = acc;
            }
            tbl.push(row);
            // next window base = 256 * base
            for _ in 0..8 {
                base = curve.add(base.0, base.1, base.0, base.1);
            }
        }
        Self { m, tbl }
    }

    /// Jacobian mixed addition: (X1,Y1,Z1) += affine (x2,y2). Z1 != 0 assumed.
    /// Returns None on the degenerate H == 0 case (doubling or cancellation),
    /// which the caller resolves by falling back to the library routine.
    #[inline]
    fn madd(&self, p: (U256, U256, U256), q: (U256, U256)) -> Option<(U256, U256, U256)> {
        let m = self.m;
        let (x1, y1, z1) = p;
        let z1z1 = z1.mul_mod(z1, m);
        let u2 = q.0.mul_mod(z1z1, m);
        let s2 = q.1.mul_mod(z1z1.mul_mod(z1, m), m);
        let h = sub_m(u2, x1, m);
        let r = sub_m(s2, y1, m);
        if h.is_zero() {
            return None;
        }
        let hh = h.mul_mod(h, m);
        let hhh = hh.mul_mod(h, m);
        let x1hh = x1.mul_mod(hh, m);
        let x3 = sub_m(
            sub_m(r.mul_mod(r, m), hhh, m),
            x1hh.mul_mod(U256::from(2), m),
            m,
        );
        let y3 = sub_m(
            r.mul_mod(sub_m(x1hh, x3, m), m),
            y1.mul_mod(hhh, m),
            m,
        );
        Some((x3, y3, z1.mul_mod(h, m)))
    }

    /// `k * G`, bit-identical to `WeierstrassEllipticCurve::mul(gx, gy, k)`.
    fn mul_g(&self, curve: &WeierstrassEllipticCurve, k: U256) -> (U256, U256) {
        let m = self.m;
        let bytes: [u8; 32] = k.to_le_bytes();
        let mut acc: Option<(U256, U256, U256)> = None;
        for (w, &d) in bytes.iter().enumerate() {
            if d == 0 {
                continue;
            }
            let q = self.tbl[w][d as usize - 1];
            acc = match acc {
                None => Some((q.0, q.1, U256::from(1))),
                Some(p) => match self.madd(p, q) {
                    Some(n) => Some(n),
                    // H == 0: the accumulator and the addend share an
                    // x-coordinate. Astronomically rare for XOF-drawn scalars;
                    // resolve exactly rather than approximately.
                    None => return curve.mul(curve.gx, curve.gy, k),
                },
            };
        }
        match acc {
            None => (U256::ZERO, U256::ZERO), // k == 0
            Some((x, y, z)) => {
                let zi = z.inv_mod(m).expect("Z nonzero");
                let zi2 = zi.mul_mod(zi, m);
                (x.mul_mod(zi2, m), y.mul_mod(zi2.mul_mod(zi, m), m))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Census
// ---------------------------------------------------------------------------
//
// For every CCX/CCZ in the UNSTRIPPED stream, accumulate three facts over many
// random on-curve input pairs:
//
//   fired  : the gate's effect mask was ever nonzero
//   viol1  : "q_control1 is redundant" was ever violated
//   viol2  : "q_control2 is redundant" was ever violated
//
// CCX effect is `cond & c1 & c2`; CCZ's is `cond & t & c1 & c2`, so CCZ carries
// its target into the effective condition. A gate that never fires is DEAD. A
// gate where `cond & c2 & ~c1` never fires has c1 implied, so CCX(c2,c1,t)
// collapses to CX(c2,t) exactly -- that is memory/03-proven-floors.md's
// "cond & q1 & ~q2 == 0" predicate, strictly weaker than "always 1" or
// "controls always equal".
//
// Shards are independent seeds; a gate is only certified if it is clean in
// EVERY shard, so merging is a bitwise OR of the violation flags.

const F_FIRED: u8 = 1;
const F_VIOL1: u8 = 2;
const F_VIOL2: u8 = 4;

/// Packed census op. `gate` indexes the accumulator for CCX/CCZ, else u32::MAX.
#[derive(Clone, Copy)]
#[repr(C)]
struct COp {
    kind: u8,
    qc1: u32,
    qc2: u32,
    qt: u32,
    ct: u32,
    cc: u32,
    gate: u32,
}

const NOBIT32: u32 = u32::MAX;
const K_CCX: u8 = 0;
const K_CX: u8 = 1;
const K_SWAP: u8 = 2;
const K_X: u8 = 3;
const K_HMR: u8 = 4;
const K_R: u8 = 5;
const K_BINV: u8 = 6;
const K_BST0: u8 = 7;
const K_BST1: u8 = 8;
const K_PUSH: u8 = 9;
const K_POP: u8 = 10;
const K_CCZ: u8 = 11;

/// Compact the stream, keeping CCZ (a strip target) even though it moves no
/// qubit, and dropping only what can neither affect state nor be stripped.
fn compact(ops: &[Op]) -> (Vec<COp>, usize) {
    let mut out = Vec::with_capacity(ops.len());
    let mut ngates = 0u32;
    for op in ops {
        let (kind, is_gate) = match op.kind {
            OperationType::CCX => (K_CCX, true),
            OperationType::CCZ => (K_CCZ, true),
            OperationType::CX => (K_CX, false),
            OperationType::Swap => (K_SWAP, false),
            OperationType::X => (K_X, false),
            OperationType::Hmr => (K_HMR, false),
            OperationType::R => (K_R, false),
            OperationType::BitInvert => (K_BINV, false),
            OperationType::BitStore0 => (K_BST0, false),
            OperationType::BitStore1 => (K_BST1, false),
            OperationType::PushCondition => (K_PUSH, false),
            OperationType::PopCondition => (K_POP, false),
            _ => continue, // Neg/Z/CZ: phase-only and not strip targets
        };
        let gate = if is_gate { let g = ngates; ngates += 1; g } else { u32::MAX };
        out.push(COp {
            kind,
            qc1: op.q_control1.0 as u32,
            qc2: op.q_control2.0 as u32,
            qt: op.q_target.0 as u32,
            ct: if op.c_target == NO_BIT { NOBIT32 } else { op.c_target.0 as u32 },
            cc: if op.c_condition == NO_BIT { NOBIT32 } else { op.c_condition.0 as u32 },
            gate,
        });
    }
    (out, ngates as usize)
}

struct Sim<const L: usize> {
    q: Vec<u64>,
    b: Vec<u64>,
    rng: u64,
    /// Harness mode: pre-drawn XOF words, consumed in stream order by BOTH Hmr
    /// and R -- real sim.rs reads 8 bytes for each, so R must consume too or
    /// the whole downstream stream desynchronises.
    rngbuf: Vec<u64>,
    rngpos: usize,
}

impl<const L: usize> Sim<L> {
    fn new(nq: usize, nb: usize, seed: u64) -> Self {
        Self { q: vec![0; nq * L], b: vec![0; nb * L], rng: seed | 1, rngbuf: Vec::new(), rngpos: 0 }
    }
    fn clear(&mut self) { self.q.fill(0); self.b.fill(0); }
    #[inline(always)]
    fn next_rng(&mut self) -> u64 {
        if !self.rngbuf.is_empty() {
            let v = self.rngbuf[self.rngpos];
            self.rngpos += 1;
            return v;
        }
        let mut x = self.rng;
        x ^= x >> 12; x ^= x << 25; x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn run(&mut self, ops: &[COp], acc: &mut [u8]) {
        let mut base = [u64::MAX; L];
        let mut stack: Vec<[u64; L]> = Vec::with_capacity(64);
        let mut cw = [0u64; L];
        for op in ops {
            if op.cc == NOBIT32 { cw = base; } else {
                let o = op.cc as usize * L;
                for w in 0..L { cw[w] = base[w] & self.b[o + w]; }
            }
            match op.kind {
                K_CCX | K_CCZ => {
                    let (a, c, t) = (op.qc1 as usize * L, op.qc2 as usize * L, op.qt as usize * L);
                    let mut fired = 0u64; let mut v1 = 0u64; let mut v2 = 0u64;
                    for w in 0..L {
                        // CCZ folds its target into the effective condition.
                        let e = if op.kind == K_CCZ { cw[w] & self.q[t + w] } else { cw[w] };
                        let (c1, c2) = (self.q[a + w], self.q[c + w]);
                        fired |= e & c1 & c2;
                        v1 |= e & c2 & !c1; // c1 not implied -> cannot drop c1
                        v2 |= e & c1 & !c2; // c2 not implied -> cannot drop c2
                        if op.kind == K_CCX { self.q[t + w] ^= e & c1 & c2; }
                    }
                    let f = &mut acc[op.gate as usize];
                    if fired != 0 { *f |= F_FIRED; }
                    if v1 != 0 { *f |= F_VIOL1; }
                    if v2 != 0 { *f |= F_VIOL2; }
                }
                K_CX => { let (a, t) = (op.qc1 as usize * L, op.qt as usize * L);
                    for w in 0..L { self.q[t + w] ^= cw[w] & self.q[a + w]; } }
                K_X => { let t = op.qt as usize * L;
                    for w in 0..L { self.q[t + w] ^= cw[w]; } }
                K_SWAP => { let (a, t) = (op.qc1 as usize * L, op.qt as usize * L);
                    for w in 0..L { let mut c1 = self.q[a + w]; let mut qt = self.q[t + w];
                        c1 ^= qt; qt ^= cw[w] & c1; c1 ^= qt;
                        self.q[a + w] = c1; self.q[t + w] = qt; } }
                K_HMR => { let (t, ct) = (op.qt as usize * L, op.ct as usize * L);
                    for w in 0..L { let r = self.next_rng();
                        self.b[ct + w] = (self.b[ct + w] & !cw[w]) ^ (r & cw[w]);
                        self.q[t + w] &= !cw[w]; } }
                K_R => { let t = op.qt as usize * L;
                    for w in 0..L { let _ = self.next_rng(); self.q[t + w] &= !cw[w]; } }
                K_BINV => { let t = op.ct as usize * L;
                    for w in 0..L { self.b[t + w] ^= cw[w]; } }
                K_BST0 => { let t = op.ct as usize * L;
                    for w in 0..L { self.b[t + w] &= !cw[w]; } }
                K_BST1 => { let t = op.ct as usize * L;
                    for w in 0..L { self.b[t + w] |= cw[w]; } }
                K_PUSH => { stack.push(base); let o = op.cc as usize * L;
                    for w in 0..L { base[w] &= self.b[o + w]; } }
                _ => { if let Some(v) = stack.pop() { base = v; } }
            }
        }
    }

    fn set_reg(&mut self, reg: &[QubitOrBit], val: U256, shot: usize) {
        let (w, bit) = (shot / 64, shot % 64);
        let m = 1u64 << bit;
        for (i, item) in reg.iter().enumerate() {
            let on = val.bit(i);
            let slot = match item {
                QubitOrBit::Qubit(id) => &mut self.q[id.0 as usize * L + w],
                QubitOrBit::Bit(id) => &mut self.b[id.0 as usize * L + w],
            };
            if on { *slot |= m; } else { *slot &= !m; }
        }
    }
}

fn census_shard<const L: usize>(
    cops: &[COp], regs: &[Vec<QubitOrBit>], nq: usize, nb: usize,
    ngates: usize, samples: usize, seed: u64, fb: &FastBase, curve: &WeierstrassEllipticCurve,
) -> Vec<u8> {
    let w = 64 * L;
    let mut acc = vec![0u8; ngates];
    let mut sim: Sim<L> = Sim::new(nq, nb, seed);
    // Independent input stream per shard: SHAKE256 over the shard seed.
    let mut h = Shake256::default();
    h.update(b"census-shard");
    h.update(&seed.to_le_bytes());
    let mut xof = h.finalize_xof();
    let mut done = 0usize;
    let t0 = Instant::now();
    while done < samples {
        let bs = w.min(samples - done);
        sim.clear();
        let mut filled = 0usize;
        while filled < bs {
            let mut rb = [[0u8; 32]; 2];
            XofReader::read(&mut xof, &mut rb[0]);
            XofReader::read(&mut xof, &mut rb[1]);
            let t = fb.mul_g(curve, U256::from_le_bytes(rb[0]));
            let o = fb.mul_g(curve, U256::from_le_bytes(rb[1]));
            if t.0 == o.0 || (t.0.is_zero() && t.1.is_zero()) || (o.0.is_zero() && o.1.is_zero()) {
                continue;
            }
            sim.set_reg(&regs[0], t.0, filled);
            sim.set_reg(&regs[1], t.1, filled);
            sim.set_reg(&regs[2], o.0, filled);
            sim.set_reg(&regs[3], o.1, filled);
            filled += 1;
        }
        sim.run(cops, &mut acc);
        done += bs;
        if done % (w * 64) == 0 {
            eprintln!("  seed={seed} {done}/{samples} ({:.0} shots/s)",
                      done as f64 / t0.elapsed().as_secs_f64());
        }
    }
    acc
}


fn absorb_op(h: &mut Shake256, op: &Op) {
    h.update(&[op.kind as u8]);
    h.update(&op.q_control2.0.to_le_bytes());
    h.update(&op.q_control1.0.to_le_bytes());
    h.update(&op.q_target.0.to_le_bytes());
    h.update(&op.c_target.0.to_le_bytes());
    h.update(&op.c_condition.0.to_le_bytes());
    h.update(&op.r_target.0.to_le_bytes());
}

/// One census shard in HARNESS ORDER: W=64 (one u64), inputs and the Hmr/R
/// stream both drawn from the real Fiat-Shamir XOF, pairs drawn up front and
/// the simulator continuing from the same reader -- exactly eval_circuit.
fn census_harness(
    ops: &[Op], cops: &[COp], regs: &[Vec<QubitOrBit>], nq: usize, nb: usize,
    ngates: usize, samples: usize, fb: &FastBase, curve: &WeierstrassEllipticCurve,
) -> Vec<u8> {
    let mut h = Shake256::default();
    h.update(b"quantum_ecc-fiat-shamir-v2");
    h.update(&(ops.len() as u64).to_le_bytes());
    for op in ops { absorb_op(&mut h, op); }
    let mut xof = h.finalize_xof();

    let mut targets = Vec::with_capacity(samples);
    let mut offsets = Vec::with_capacity(samples);
    for _ in 0..samples {
        let mut rb = [[0u8; 32]; 2];
        XofReader::read(&mut xof, &mut rb[0]);
        XofReader::read(&mut xof, &mut rb[1]);
        let t = fb.mul_g(curve, U256::from_le_bytes(rb[0]));
        let o = fb.mul_g(curve, U256::from_le_bytes(rb[1]));
        if t.0 == o.0 || (t.0.is_zero() && t.1.is_zero()) || (o.0.is_zero() && o.1.is_zero()) { continue; }
        targets.push(t); offsets.push(o);
    }
    let n = targets.len();
    let mut acc = vec![0u8; ngates];
    let nrng = cops.iter().filter(|o| o.kind == K_HMR || o.kind == K_R).count();
    eprintln!("harness mode: {} XOF words per 64-shot pass", nrng);
    let mut sim: Sim<1> = Sim::new(nq, nb, 1);
    let t0 = Instant::now();
    for b in 0..n.div_ceil(64) {
        let bs = 64.min(n - b * 64);
        sim.clear();
        for shot in 0..bs {
            let i = b * 64 + shot;
            sim.set_reg(&regs[0], targets[i].0, shot);
            sim.set_reg(&regs[1], targets[i].1, shot);
            sim.set_reg(&regs[2], offsets[i].0, shot);
            sim.set_reg(&regs[3], offsets[i].1, shot);
        }
        sim.rngbuf.clear();
        sim.rngbuf.resize(nrng, 0);
        for v in sim.rngbuf.iter_mut() {
            let mut bb = [0u8; 8];
            XofReader::read(&mut xof, &mut bb);
            *v = u64::from_le_bytes(bb);
        }
        sim.rngpos = 0;
        sim.run(cops, &mut acc);
        if b % 20000 == 0 && b > 0 {
            eprintln!("  harness {}/{} ({:.0} shots/s)", b*64, n, (b*64) as f64 / t0.elapsed().as_secs_f64());
        }
    }
    acc
}

fn out_path_dump(s: &str) -> String { s.to_string() }

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut mode = String::from("shard");
    let mut samples = 1_000_000usize;
    let mut seed = 1u64;
    let mut lanes = 16usize;
    let mut out_file = String::from("shard.bin");
    let mut shards = String::new();
    let mut harness = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--mode" => { mode = args[i + 1].clone(); i += 2; }
            "--samples" => { samples = args[i + 1].replace('_', "").parse().unwrap(); i += 2; }
            "--seed" => { seed = args[i + 1].parse().unwrap(); i += 2; }
            "--lanes" => { lanes = args[i + 1].parse().unwrap(); i += 2; }
            "--out" => { out_file = args[i + 1].clone(); i += 2; }
            "--shards" => { shards = args[i + 1].clone(); i += 2; }
            "--harness" => { harness = true; i += 1; }
            o => { eprintln!("unknown arg {o}"); std::process::exit(2); }
        }
        }

    let curve = secp256k1();
    let fb = FastBase::new(&curve);
    let ops = point_add::build();
    let (total_qubits, num_bits, _n, regs) = analyze_ops(ops.iter());
    let (cops, ngates) = compact(&ops);
    eprintln!("census: ops={} compacted={} gates(CCX+CCZ)={} qubits={} bits={} mode={mode}",
              ops.len(), cops.len(), ngates, total_qubits, num_bits);

    if mode == "shard" && harness {
        let t0 = Instant::now();
        let acc = census_harness(&ops, &cops, &regs, total_qubits as usize, num_bits as usize, ngates, samples, &fb, &curve);
        std::fs::write(&out_file, &acc).expect("write shard");
        let dead = acc.iter().filter(|f| **f & F_FIRED == 0).count();
        let d1 = acc.iter().filter(|f| **f & F_FIRED != 0 && **f & F_VIOL1 == 0).count();
        let d2 = acc.iter().filter(|f| **f & F_FIRED != 0 && **f & F_VIOL2 == 0).count();
        eprintln!("HARNESS-ORDER shard samples={samples} in {:.1}s -> never-fired {dead}, c1-implied {d1}, c2-implied {d2}", t0.elapsed().as_secs_f64());
        return;
    }
    if mode == "shard" {
        let t0 = Instant::now();
        let acc = match lanes {
            8 => census_shard::<8>(&cops, &regs, total_qubits as usize, num_bits as usize, ngates, samples, seed, &fb, &curve),
            16 => census_shard::<16>(&cops, &regs, total_qubits as usize, num_bits as usize, ngates, samples, seed, &fb, &curve),
            32 => census_shard::<32>(&cops, &regs, total_qubits as usize, num_bits as usize, ngates, samples, seed, &fb, &curve),
            64 => census_shard::<64>(&cops, &regs, total_qubits as usize, num_bits as usize, ngates, samples, seed, &fb, &curve),
            128 => census_shard::<128>(&cops, &regs, total_qubits as usize, num_bits as usize, ngates, samples, seed, &fb, &curve),
            _ => panic!("--lanes must be 8, 16, 32, 64 or 128"),
        };
        std::fs::write(&out_file, &acc).expect("write shard");
        let dead = acc.iter().filter(|f| **f & F_FIRED == 0).count();
        let d1 = acc.iter().filter(|f| **f & F_FIRED != 0 && **f & F_VIOL1 == 0).count();
        let d2 = acc.iter().filter(|f| **f & F_FIRED != 0 && **f & F_VIOL2 == 0).count();
        eprintln!("shard seed={seed} samples={samples} in {:.1}s -> never-fired {dead}, c1-implied {d1}, c2-implied {d2}",
                  t0.elapsed().as_secs_f64());
        return;
    }

    if mode == "dump" {
        // Every gate: tuple, ordinal, occupancy, merged flags. Lets the
        // known-answer test be done in full, including gates that are LIVE
        // (and therefore absent from any emitted key table).
        let mut merged = vec![0u8; ngates];
        for f in shards.split(',').filter(|s| !s.is_empty()) {
            let d = std::fs::read(f.split(':').next().unwrap()).expect("read shard");
            assert_eq!(d.len(), ngates);
            for (m, x) in merged.iter_mut().zip(d.iter()) { *m |= *x; }
        }
        use std::collections::HashMap;
        let mut occ: HashMap<(u8,u64,u64,u64,u64), u32> = HashMap::new();
        for op in &ops {
            let kb = op.kind as u8;
            if kb == 13 || kb == 14 {
                *occ.entry((kb, op.q_control2.0, op.q_control1.0, op.q_target.0, op.c_condition.0)).or_insert(0) += 1;
            }
        }
        let mut ord: HashMap<(u8,u64,u64,u64,u64), u32> = HashMap::new();
        let mut out = String::new();
        let mut g = 0usize;
        for op in &ops {
            let kb = op.kind as u8;
            if kb != 13 && kb != 14 { continue; }
            let tup = (kb, op.q_control2.0, op.q_control1.0, op.q_target.0, op.c_condition.0);
            let o = ord.entry(tup).or_insert(0);
            out.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                kb, tup.1, tup.2, tup.3, tup.4, *o, occ[&tup], merged[g]));
            *o += 1; g += 1;
        }
        std::fs::write(&out_path_dump(&out_file), out).expect("write dump");
        eprintln!("dumped {} gates", g);
        return;
    }

    // ---- emit: merge shards and write deep_strip_keys.rs ----
    let mut merged = vec![0u8; ngates];
    let mut total_samples = 0usize;
    let mut nsh = 0usize;
    for f in shards.split(',').filter(|s| !s.is_empty()) {
        let parts: Vec<&str> = f.split(':').collect();
        let data = std::fs::read(parts[0]).expect("read shard");
        assert_eq!(data.len(), ngates, "shard {} has {} gates, stream has {}", parts[0], data.len(), ngates);
        for (m, d) in merged.iter_mut().zip(data.iter()) { *m |= *d; }
        if parts.len() > 1 { total_samples += parts[1].replace('_', "").parse::<usize>().unwrap(); }
        nsh += 1;
    }
    eprintln!("merged {nsh} shards, {total_samples} total samples");

    // ordinal / occupancy keyed exactly as apply_deep_strip_identity expects
    use std::collections::HashMap;
    type Tup = (u8, u64, u64, u64, u64);
    let mut occ: HashMap<Tup, u32> = HashMap::new();
    let mut keyed: Vec<(Tup, u32)> = Vec::with_capacity(ngates);
    for op in &ops {
        let kb = op.kind as u8;
        if kb == 13 || kb == 14 {
            let tup = (kb, op.q_control2.0, op.q_control1.0, op.q_target.0, op.c_condition.0);
            let e = occ.entry(tup).or_insert(0);
            keyed.push((tup, *e));
            *e += 1;
        }
    }
    assert_eq!(keyed.len(), ngates, "gate indexing disagrees with stream order");

    let mut dead_rows = Vec::new();
    let mut down_rows = Vec::new();
    for (g, &(tup, ord)) in keyed.iter().enumerate() {
        let f = merged[g];
        let tot = occ[&tup];
        if f & F_FIRED == 0 {
            dead_rows.push((tup, ord, tot));
        } else if f & F_VIOL2 == 0 {
            down_rows.push((tup, ord, tot, 2u8)); // c2 implied by c1: keep c1
        } else if f & F_VIOL1 == 0 {
            down_rows.push((tup, ord, tot, 1u8)); // c1 implied by c2: keep c2
        }
    }

    let mut s = String::new();
    s.push_str("// Auto-generated identity-keyed deep strip (do not edit by hand).\n");
    s.push_str("// Key = (kind, q_control2, q_control1, q_target, c_condition, ordinal, tuple_occupancy).\n");
    s.push_str("// ordinal = k-th occurrence of that exact CCX/CCZ operand tuple in stream order;\n");
    s.push_str("// tuple_occupancy = how many times that tuple occurred in the censused stream.\n");
    s.push_str("//\n");
    s.push_str(&format!(
        "// Census: {} random on-curve secp256k1 input pairs over {} independent shards, against\n\
         // the wide-lane simulator in tools/census/ (fixed-base comb, bit-identical to curve.mul).\n\
         // Mined at TLM_SCHED_J2_DELTA=2, SUB4_APPLY_STRIP=0:\n\
         //   {} ops / {} CCX+CCZ.\n",
        total_samples, nsh, ops.len(), ngates));
    s.push_str("pub static DEAD_KEYS: &[(u8, u64, u64, u64, u64, u32, u32)] = &[\n");
    for ((k, c2, c1, t, cc), o, tot) in &dead_rows {
        s.push_str(&format!("    ({k}, {c2}, {c1}, {t}, {cc}, {o}, {tot}),\n"));
    }
    s.push_str("];\n\npub static DOWNGRADE_KEYS: &[(u8, u64, u64, u64, u64, u32, u32, u8)] = &[\n");
    for ((k, c2, c1, t, cc), o, tot, act) in &down_rows {
        s.push_str(&format!("    ({k}, {c2}, {c1}, {t}, {cc}, {o}, {tot}, {act}),\n"));
    }
    s.push_str("];\n");
    std::fs::write(&out_file, s).expect("write keys");
    eprintln!("wrote {out_file}: {} dead, {} downgrade", dead_rows.len(), down_rows.len());
}
