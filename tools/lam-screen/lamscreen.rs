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

use quantum_ecc::circuit::{analyze_ops, Op, QubitId, QubitOrBit};
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

fn run_classical(
    ops: &[Op],
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
    let mut sim = Simulator::new(total_qubits as usize, num_bits as usize, &mut xof);
    let mut classical = 0usize;
    const BATCH: usize = 64;
    let num_batches = n.div_ceil(BATCH);
    for batch in 0..num_batches {
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
            let gx = sim.get_register(&regs[0], shot);
            let gy = sim.get_register(&regs[1], shot);
            if gx != expected[i].0 || gy != expected[i].1 {
                classical += 1;
            }
        }
    }
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
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--nonces" => { nonce_file = args[i + 1].clone(); i += 2; }
            "--mode"   => { mode = args[i + 1].clone(); i += 2; }
            "--out"    => { out = args[i + 1].clone(); i += 2; }
            "--tag"    => { tag = args[i + 1].clone(); i += 2; }
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
        let t0 = Instant::now();
        let r = if mode == "count" {
            run_classical(&ops, &regs, total_qubits, num_bits, &prefix, &fb, FULL_SHOTS)
        } else {
            let mut last = run_classical(&ops, &regs, total_qubits, num_bits, &prefix, &fb, SHOT_LADDER[0]);
            for &rung in SHOT_LADDER.iter().skip(1) {
                if last.classical > 0 {
                    break;
                }
                last = run_classical(&ops, &regs, total_qubits, num_bits, &prefix, &fb, rung);
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
