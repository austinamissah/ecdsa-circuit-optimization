//! Throwaway: per-shot dump for validating a classical Leapfrog walk model. Copy to src/bin to build; do not commit it there.
//! Usage: lfcheck <nonce>...   -> TSV on stdout: nonce shot tx ox x3 wrong phase
use quantum_ecc::circuit::{analyze_ops, QubitId};
#[allow(dead_code)]
#[path = "../point_add/mod.rs"]
mod point_add;
#[allow(unused_imports)]
use quantum_ecc::{circuit, sim, weierstrass_elliptic_curve};
use quantum_ecc::sim::Simulator;
use quantum_ecc::weierstrass_elliptic_curve::WeierstrassEllipticCurve;
use ruint::aliases::U256;
use sha3::digest::{ExtendableOutput, Update, XofReader};
use sha3::Shake256;

fn main() {
    let hx = |h: &str| U256::from_str_radix(h, 16).unwrap();
    let curve = WeierstrassEllipticCurve {
        modulus: hx("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F"),
        a: U256::from(0u8),
        b: U256::from(7u8),
        gx: hx("79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798"),
        gy: hx("483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8"),
        order: hx("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141"),
    };
    let mut ops = point_add::build();
    let (nq, nb, _, regs) = analyze_ops(ops.iter());
    let start = ops.len() - 96;
    println!("nonce\tshot\ttx\tox\tx3\twrong\tphase");
    for a in std::env::args().skip(1) {
        let nonce: u64 = a.parse().unwrap();
        for b in 0..48 {
            let t = QubitId((nonce >> b) & 1);
            ops[start + 2 * b].q_target = t;
            ops[start + 2 * b + 1].q_target = t;
        }
        let mut h = Shake256::default();
        h.update(b"quantum_ecc-fiat-shamir-v2");
        h.update(&(ops.len() as u64).to_le_bytes());
        for op in &ops {
            h.update(&[op.kind as u8]);
            h.update(&op.q_control2.0.to_le_bytes());
            h.update(&op.q_control1.0.to_le_bytes());
            h.update(&op.q_target.0.to_le_bytes());
            h.update(&op.c_target.0.to_le_bytes());
            h.update(&op.c_condition.0.to_le_bytes());
            h.update(&op.r_target.0.to_le_bytes());
        }
        let mut xof = h.finalize_xof();
        let (mut tg, mut of, mut ex) = (Vec::new(), Vec::new(), Vec::new());
        for _ in 0..9024 {
            let mut rb = [[0u8; 32]; 2];
            XofReader::read(&mut xof, &mut rb[0]);
            XofReader::read(&mut xof, &mut rb[1]);
            let t = curve.mul(curve.gx, curve.gy, U256::from_le_bytes(rb[0]));
            let o = curve.mul(curve.gx, curve.gy, U256::from_le_bytes(rb[1]));
            if t.0 == o.0 || (t.0.is_zero() && t.1.is_zero()) || (o.0.is_zero() && o.1.is_zero()) {
                continue;
            }
            ex.push(curve.add(t.0, t.1, o.0, o.1));
            tg.push(t);
            of.push(o);
        }
        let n = tg.len();
        let mut sim = Simulator::new(nq as usize, nb as usize, &mut xof);
        for b in 0..n.div_ceil(64) {
            let bs = 64.min(n - b * 64);
            sim.clear_for_shot();
            for s in 0..bs {
                let i = b * 64 + s;
                sim.set_register(&regs[0], tg[i].0, s);
                sim.set_register(&regs[1], tg[i].1, s);
                sim.set_register(&regs[2], of[i].0, s);
                sim.set_register(&regs[3], of[i].1, s);
            }
            sim.apply_iter(ops.iter());
            for s in 0..bs {
                let i = b * 64 + s;
                let wrong = sim.get_register(&regs[0], s) != ex[i].0 || sim.get_register(&regs[1], s) != ex[i].1;
                let ph = (sim.phase >> s) & 1;
                println!("{nonce}\t{i}\t{:x}\t{:x}\t{:x}\t{}\t{}", tg[i].0, of[i].0, ex[i].0, wrong as u8, ph);
            }
        }
    }
}
