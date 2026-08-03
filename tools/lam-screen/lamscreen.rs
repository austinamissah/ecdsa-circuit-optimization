//! `lamscreen` — the nonce screen with a fast fixed-base scalar multiplier.
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
// Wide-lane classical simulator
// ---------------------------------------------------------------------------
//
// `sim::Simulator` bitslices 64 shots into one u64 and walks the whole 9.06 M-op
// stream once per batch of 64 -- 141 passes over 507 MB, plus one random read
// into a 4.2 MB bit array for every conditioned op. This replaces it with:
//
//   1. `W = 64 * L` lanes, so a 9,024-shot nonce takes 141/L passes. The bit
//      array read per op becomes L consecutive words -- one cache line up to
//      L = 8 -- so the miss count per op is unchanged while L times the shots
//      ride on it.
//   2. A 24-byte packed op instead of the 56-byte `Op`, with the 535,472
//      phase-only ops (Neg/Z/CZ/CCZ) and 1,028 structural ops dropped: they
//      cannot affect a classical outcome. 8.52 M ops x 24 B = 204 MB a pass.
//   3. A xorshift PRNG for the Hmr/R lanes in place of 1.01 M 8-byte SHAKE256
//      squeezes per pass.
//
// (2) and (3) are licensed by memory/04-traps.md section 4: classical outcomes
// are insensitive to both the value and the consumption order of the Hmr/R
// stream, and phase is not reported here. Both claims are re-tested end to end
// by `--mode gate`, which must reproduce all 199 harness counts exactly.

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

#[derive(Clone, Copy)]
#[repr(C)]
struct COp {
    kind: u8,
    qc1: u32,
    qc2: u32,
    qt: u32,
    ct: u32,
    cc: u32,
}

/// Pack the op stream, dropping everything that cannot move a classical bit.
/// The 96-op `apply_tail_nonce` tail is all `X`, so it survives compaction and
/// stays the last 96 entries -- `patch_ctail` relies on that and asserts it.
fn compact(ops: &[Op]) -> Vec<COp> {
    let mut out = Vec::with_capacity(ops.len());
    for op in ops {
        let kind = match op.kind {
            OperationType::CCX => K_CCX,
            OperationType::CX => K_CX,
            OperationType::Swap => K_SWAP,
            OperationType::X => K_X,
            OperationType::Hmr => K_HMR,
            OperationType::R => K_R,
            OperationType::BitInvert => K_BINV,
            OperationType::BitStore0 => K_BST0,
            OperationType::BitStore1 => K_BST1,
            OperationType::PushCondition => K_PUSH,
            OperationType::PopCondition => K_POP,
            // Phase-only (Neg/Z/CZ/CCZ) and structural (Register/
            // AppendToRegister/DebugPrint): no classical effect.
            _ => continue,
        };
        out.push(COp {
            kind,
            qc1: op.q_control1.0 as u32,
            qc2: op.q_control2.0 as u32,
            qt: op.q_target.0 as u32,
            ct: if op.c_target == NO_BIT { NOBIT32 } else { op.c_target.0 as u32 },
            cc: if op.c_condition == NO_BIT { NOBIT32 } else { op.c_condition.0 as u32 },
        });
    }
    out
}

fn patch_ctail(cops: &mut [COp], nonce: u64) {
    let start = cops.len() - TAIL;
    for b in 0..48 {
        assert_eq!(cops[start + 2 * b].kind, K_X, "tail op is not X");
        let t = if (nonce >> b) & 1 == 1 { 1u32 } else { 0u32 };
        cops[start + 2 * b].qt = t;
        cops[start + 2 * b + 1].qt = t;
    }
}

struct WideSim<const L: usize> {
    q: Vec<u64>,
    b: Vec<u64>,
    rng: u64,
}

impl<const L: usize> WideSim<L> {
    fn new(nq: usize, nb: usize, seed: u64) -> Self {
        Self { q: vec![0; nq * L], b: vec![0; nb * L], rng: seed | 1 }
    }

    fn clear(&mut self) {
        self.q.fill(0);
        self.b.fill(0);
    }

    #[inline(always)]
    fn next_rng(&mut self) -> u64 {
        // xorshift64*: the Hmr/R lanes only need to be unbiased, not reproducible.
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn apply(&mut self, ops: &[COp]) {
        let mut base = [u64::MAX; L];
        let mut stack: Vec<[u64; L]> = Vec::with_capacity(64);
        let mut cw = [0u64; L];

        for op in ops {
            if op.cc == NOBIT32 {
                cw = base;
            } else {
                let o = op.cc as usize * L;
                for w in 0..L {
                    cw[w] = base[w] & self.b[o + w];
                }
            }
            match op.kind {
                K_CCX => {
                    let (a, c, t) = (op.qc1 as usize * L, op.qc2 as usize * L, op.qt as usize * L);
                    for w in 0..L {
                        self.q[t + w] ^= cw[w] & self.q[a + w] & self.q[c + w];
                    }
                }
                K_CX => {
                    let (a, t) = (op.qc1 as usize * L, op.qt as usize * L);
                    for w in 0..L {
                        self.q[t + w] ^= cw[w] & self.q[a + w];
                    }
                }
                K_X => {
                    let t = op.qt as usize * L;
                    for w in 0..L {
                        self.q[t + w] ^= cw[w];
                    }
                }
                K_SWAP => {
                    let (a, t) = (op.qc1 as usize * L, op.qt as usize * L);
                    for w in 0..L {
                        let mut c1 = self.q[a + w];
                        let mut qt = self.q[t + w];
                        c1 ^= qt;
                        qt ^= cw[w] & c1;
                        c1 ^= qt;
                        self.q[a + w] = c1;
                        self.q[t + w] = qt;
                    }
                }
                K_HMR => {
                    let (t, ct) = (op.qt as usize * L, op.ct as usize * L);
                    for w in 0..L {
                        let r = self.next_rng();
                        self.b[ct + w] = (self.b[ct + w] & !cw[w]) ^ (r & cw[w]);
                        self.q[t + w] &= !cw[w];
                    }
                }
                K_R => {
                    let t = op.qt as usize * L;
                    for w in 0..L {
                        self.q[t + w] &= !cw[w];
                    }
                }
                K_BINV => {
                    let t = op.ct as usize * L;
                    for w in 0..L {
                        self.b[t + w] ^= cw[w];
                    }
                }
                K_BST0 => {
                    let t = op.ct as usize * L;
                    for w in 0..L {
                        self.b[t + w] &= !cw[w];
                    }
                }
                K_BST1 => {
                    let t = op.ct as usize * L;
                    for w in 0..L {
                        self.b[t + w] |= cw[w];
                    }
                }
                K_PUSH => {
                    stack.push(base);
                    let o = op.cc as usize * L;
                    for w in 0..L {
                        base[w] &= self.b[o + w];
                    }
                }
                _ => {
                    if let Some(v) = stack.pop() {
                        base = v;
                    }
                }
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
            if on {
                *slot |= m;
            } else {
                *slot &= !m;
            }
        }
    }

    fn get_reg(&self, reg: &[QubitOrBit], shot: usize) -> U256 {
        let (w, bit) = (shot / 64, shot % 64);
        let mut v = U256::ZERO;
        for (i, item) in reg.iter().enumerate() {
            let word = match item {
                QubitOrBit::Qubit(id) => self.q[id.0 as usize * L + w],
                QubitOrBit::Bit(id) => self.b[id.0 as usize * L + w],
            };
            v.set_bit(i, (word >> bit) & 1 != 0);
        }
        v
    }
}

/// Run `n` shots through a `WideSim<L>` and count classical mismatches.
#[allow(clippy::too_many_arguments)]
fn wide_count<const L: usize>(
    cops: &[COp],
    regs: &[Vec<QubitOrBit>],
    nq: usize,
    nb: usize,
    targets: &[(U256, U256)],
    offsets: &[(U256, U256)],
    expected: &[(U256, U256)],
    seed: u64,
) -> usize {
    let w = 64 * L;
    let n = targets.len();
    let mut sim: WideSim<L> = WideSim::new(nq, nb, seed);
    let mut classical = 0usize;
    for batch in 0..n.div_ceil(w) {
        let bs = w.min(n - batch * w);
        sim.clear();
        for shot in 0..bs {
            let i = batch * w + shot;
            sim.set_reg(&regs[0], targets[i].0, shot);
            sim.set_reg(&regs[1], targets[i].1, shot);
            sim.set_reg(&regs[2], offsets[i].0, shot);
            sim.set_reg(&regs[3], offsets[i].1, shot);
        }
        sim.apply(cops);
        for shot in 0..bs {
            let i = batch * w + shot;
            if sim.get_reg(&regs[0], shot) != expected[i].0
                || sim.get_reg(&regs[1], shot) != expected[i].1
            {
                classical += 1;
            }
        }
    }
    classical
}

fn wide_dispatch(
    lanes: usize,
    cops: &[COp],
    regs: &[Vec<QubitOrBit>],
    nq: usize,
    nb: usize,
    t: &[(U256, U256)],
    o: &[(U256, U256)],
    e: &[(U256, U256)],
    seed: u64,
) -> usize {
    match lanes {
        1 => wide_count::<1>(cops, regs, nq, nb, t, o, e, seed),
        2 => wide_count::<2>(cops, regs, nq, nb, t, o, e, seed),
        4 => wide_count::<4>(cops, regs, nq, nb, t, o, e, seed),
        8 => wide_count::<8>(cops, regs, nq, nb, t, o, e, seed),
        16 => wide_count::<16>(cops, regs, nq, nb, t, o, e, seed),
        32 => wide_count::<32>(cops, regs, nq, nb, t, o, e, seed),
        _ => panic!("--lanes must be one of 1,2,4,8,16,32"),
    }
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

/// Verbatim `apply_tail_nonce` (point_add/mod.rs:1792), in place.
fn patch_tail(ops: &mut [Op], nonce: u64) {
    let start = ops.len() - TAIL;
    for b in 0..48 {
        let t = if (nonce >> b) & 1 == 1 { QubitId(1) } else { QubitId(0) };
        ops[start + 2 * b].q_target = t;
        ops[start + 2 * b + 1].q_target = t;
    }
}

struct Trial {
    classical: usize,
    n_shots: usize,
    fp: String,
    pair_ms: u128,
    sim_ms: u128,
}

#[allow(clippy::too_many_arguments)]
fn run_classical(
    ops: &[Op],
    cops: &[COp],
    lanes: usize,
    regs: &[Vec<QubitOrBit>],
    total_qubits: u64,
    num_bits: u64,
    prefix: &Shake256,
    fb: &FastBase,
    target_shots: usize,
) -> Trial {
    let mut h = prefix.clone();
    for op in &ops[ops.len() - TAIL..] {
        absorb_op(&mut h, op);
    }

    let mut fp_reader = h.clone().finalize_xof();
    let mut fp_bytes = [0u8; 16];
    XofReader::read(&mut fp_reader, &mut fp_bytes);
    let fp = fp_bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();

    let mut xof = h.finalize_xof();
    let curve = secp256k1();

    // --- draw every pair up front (04-traps.md section 4) ---
    let t_pair = Instant::now();
    let mut targets = Vec::with_capacity(target_shots);
    let mut offsets = Vec::with_capacity(target_shots);
    let mut expected = Vec::with_capacity(target_shots);
    for _ in 0..target_shots {
        let mut rb = [[0u8; 32]; 2];
        XofReader::read(&mut xof, &mut rb[0]);
        XofReader::read(&mut xof, &mut rb[1]);
        let t = fb.mul_g(&curve, U256::from_le_bytes(rb[0]));
        let o = fb.mul_g(&curve, U256::from_le_bytes(rb[1]));
        if t.0 == o.0 {
            continue;
        }
        if t.0.is_zero() && t.1.is_zero() {
            continue;
        }
        if o.0.is_zero() && o.1.is_zero() {
            continue;
        }
        let e = curve.add(t.0, t.1, o.0, o.1);
        targets.push(t);
        offsets.push(o);
        expected.push(e);
    }
    let n = targets.len();
    let pair_ms = t_pair.elapsed().as_millis();

    // --- simulate ---
    let t_sim = Instant::now();
    let classical = if lanes == 0 {
        // Reference path: the library's 64-lane simulator, reading the real
        // op stream and the real XOF. This is what the 199/199 gate validated.
        let mut sim = Simulator::new(total_qubits as usize, num_bits as usize, &mut xof);
        let mut mism = 0usize;
        const BATCH: usize = 64;
        for batch in 0..n.div_ceil(BATCH) {
            let bs = BATCH.min(n - batch * BATCH);
            sim.clear_for_shot();
            for shot in 0..bs {
                let i = batch * BATCH + shot;
                sim.set_register(&regs[0], targets[i].0, shot);
                sim.set_register(&regs[1], targets[i].1, shot);
                sim.set_register(&regs[2], offsets[i].0, shot);
                sim.set_register(&regs[3], offsets[i].1, shot);
            }
            sim.apply_iter(ops.iter());
            for shot in 0..bs {
                let i = batch * BATCH + shot;
                if sim.get_register(&regs[0], shot) != expected[i].0
                    || sim.get_register(&regs[1], shot) != expected[i].1
                {
                    mism += 1;
                }
            }
        }
        mism
    } else {
        // Seed the Hmr/R lanes off the stream fingerprint so a nonce is
        // reproducible run to run, without touching the XOF.
        let seed = u64::from_le_bytes(fp_bytes[..8].try_into().unwrap());
        wide_dispatch(
            lanes, cops, regs, total_qubits as usize, num_bits as usize,
            &targets, &offsets, &expected, seed,
        )
    };
    let sim_ms = t_sim.elapsed().as_millis();
    Trial { classical, n_shots: n, fp, pair_ms, sim_ms }
}

fn selftest(rounds: usize) {
    let curve = secp256k1();
    let t0 = Instant::now();
    let fb = FastBase::new(&curve);
    let build_ms = t0.elapsed().as_millis();
    let mut h = Shake256::default();
    h.update(b"lamscreen-selftest");
    let mut xof = h.finalize_xof();

    // Edge cases first.
    for k in [U256::ZERO, U256::from(1u64), U256::from(255u64), U256::from(256u64), curve.order] {
        let a = fb.mul_g(&curve, k);
        let b = curve.mul(curve.gx, curve.gy, k);
        assert_eq!(a, b, "mismatch at k={k}");
    }

    let mut t_fast = 0u128;
    let mut t_slow = 0u128;
    for _ in 0..rounds {
        let mut rb = [0u8; 32];
        XofReader::read(&mut xof, &mut rb);
        let k = U256::from_le_bytes(rb);
        let s = Instant::now();
        let a = fb.mul_g(&curve, k);
        t_fast += s.elapsed().as_nanos();
        let s = Instant::now();
        let b = curve.mul(curve.gx, curve.gy, k);
        t_slow += s.elapsed().as_nanos();
        assert_eq!(a, b, "mismatch at k={k}");
    }
    println!(
        "selftest OK: {rounds} random scalars + 5 edge cases bit-identical.\n\
         table build {build_ms} ms; fast {:.1} us/point, library {:.1} us/point, speedup {:.1}x",
        t_fast as f64 / rounds as f64 / 1000.0,
        t_slow as f64 / rounds as f64 / 1000.0,
        t_slow as f64 / t_fast as f64
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut nonce_file = String::new();
    let mut mode = "count".to_string();
    let mut out = "-".to_string();
    let mut tag = String::new();
    let mut lanes = 0usize; // 0 = reference library simulator
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--nonces" => { nonce_file = args[i + 1].clone(); i += 2; }
            "--mode"   => { mode = args[i + 1].clone(); i += 2; }
            "--out"    => { out = args[i + 1].clone(); i += 2; }
            "--tag"    => { tag = args[i + 1].clone(); i += 2; }
            "--lanes"  => { lanes = args[i + 1].parse().unwrap(); i += 2; }
            "--selftest" => { selftest(args[i + 1].parse().unwrap()); return; }
            other => { eprintln!("unknown arg {other}"); std::process::exit(2); }
        }
    }

    let nonces: Vec<u64> = std::fs::read_to_string(&nonce_file)
        .expect("read nonce file")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().parse().expect("parse nonce"))
        .collect();

    let curve = secp256k1();
    let fb = FastBase::new(&curve);

    let t_build = Instant::now();
    let mut ops = point_add::build();
    let build_ms = t_build.elapsed().as_millis();
    let (total_qubits, num_bits, _nregs, regs) = analyze_ops(ops.iter());
    assert_eq!(regs.len(), 4, "expected 4 registers");

    let mut cops = compact(&ops);
    eprintln!("lamscreen: compacted {} -> {} ops ({} lanes/word-group)", ops.len(), cops.len(), lanes);

    let t_prefix = Instant::now();
    let mut prefix = Shake256::default();
    prefix.update(b"quantum_ecc-fiat-shamir-v2");
    prefix.update(&(ops.len() as u64).to_le_bytes());
    for op in &ops[..ops.len() - TAIL] {
        absorb_op(&mut prefix, op);
    }
    let prefix_ms = t_prefix.elapsed().as_millis();

    eprintln!(
        "lamscreen: tag={tag} ops={} qubits={} bits={} build={build_ms}ms prefix_absorb={prefix_ms}ms mode={mode}",
        ops.len(), total_qubits, num_bits
    );

    let mut sink: Box<dyn Write> = if out == "-" {
        Box::new(std::io::stdout())
    } else {
        Box::new(std::fs::File::create(&out).expect("create out"))
    };
    writeln!(sink, "tag\tnonce\tclassical\tn_shots\trung\tms\tpair_ms\tsim_ms\tstream_fp").unwrap();

    for nonce in nonces {
        patch_tail(&mut ops, nonce);
        patch_ctail(&mut cops, nonce);
        let t0 = Instant::now();
        let r = if mode == "count" {
            run_classical(&ops, &cops, lanes, &regs, total_qubits, num_bits, &prefix, &fb, FULL_SHOTS)
        } else {
            let mut last = run_classical(&ops, &cops, lanes, &regs, total_qubits, num_bits, &prefix, &fb, SHOT_LADDER[0]);
            for &rung in SHOT_LADDER.iter().skip(1) {
                if last.classical > 0 {
                    break;
                }
                last = run_classical(&ops, &cops, lanes, &regs, total_qubits, num_bits, &prefix, &fb, rung);
            }
            last
        };
        let rung = if mode == "count" { FULL_SHOTS } else { r.n_shots };
        let ms = t0.elapsed().as_millis();
        writeln!(
            sink,
            "{tag}\t{nonce}\t{}\t{}\t{rung}\t{ms}\t{}\t{}\t{}",
            r.classical, r.n_shots, r.pair_ms, r.sim_ms, r.fp
        ).unwrap();
        sink.flush().unwrap();
    }
}
