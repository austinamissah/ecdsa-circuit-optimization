//! ============================================================================
//!  VALIDATED 2026-08-02 on head 801dd20 — CLASSICAL CHANNEL ONLY.
//! ============================================================================
//!
//! Built clean and passed its correctness gate: it reproduces the full
//! harness's per-nonce classical mismatch **count** — not merely its
//! clean/dirty verdict — on **199 / 199** nonces, exactly. The paired
//! difference (screen − harness) is a single spike at zero: min 0, max 0,
//! mean 0, zero nonzero entries, and the totals agree at 3,230 mismatches on
//! both sides. All 199 produced distinct stream fingerprints, so the tail
//! patch demonstrably reached the stream on every trial.
//!
//! Evidence, committed:
//!   - `docs/data/screen-gate-801dd20-paired.tsv`  (harness vs screen, per nonce)
//!   - `docs/data/screen-gate-801dd20.tsv`         (raw screen output)
//!   - `docs/data/lambda-sweep-801dd20.tsv`        (the harness reference)
//!
//! The gate must be re-run against any new circuit head before this is trusted
//! there: it validates a transcription of `eval_circuit`'s test loop, and that
//! transcription is only known correct for the stream it was checked on.
//!
//! ## What it does — CLASSICAL CHANNEL ONLY
//!
//! Reproduce `eval_circuit`'s classical-mismatch count without re-emitting the
//! op stream per nonce. Two structural savings over `./benchmark.sh`:
//!
//!   1. `point_add::build()` runs ONCE. `apply_tail_nonce` only rewrites
//!      `q_target` on the last 96 ops, so per nonce we patch those in place.
//!      This is the big one: the build is ~59 s and the harness pays it on
//!      EVERY trial.
//!   2. `fiat_shamir_seed` is a streaming SHAKE256 absorb. The state over
//!      ops[0 .. n-96] is absorbed once and cloned per nonce; each nonce then
//!      absorbs only 96*56 = 5,376 bytes instead of the full ~507 MB.
//!
//! Everything downstream of the seed — pair drawing, register layout, the batch
//! loop, the comparison — is transcribed from `src/bin/eval_circuit.rs`, and
//! the gate above confirms the classical count comes out identical.
//!
//! Measured uncontended on the validation machine (see
//! `docs/lambda-measurement.md` for hardware):
//!
//!     ./benchmark.sh                  110 s/nonce   (build 59 + eval 57)
//!     screen --mode count              ~55 s/nonce   (eval only, build amortised)
//!     screen --mode ladder            ~12 s/nonce   (mean over 20 nonces)
//!
//! so the ladder path is ~9x the harness. The one-time build is ~53-59 s per
//! PROCESS, so batch as many nonces per invocation as possible.
//!
//! ## Known limitation, by construction
//!
//! This covers the classical channel only. The measured decomposition on
//! `801dd20` is lambda_classical_only 9.12, lambda_both 7.11, lambda_phase_only
//! 3.80, so a nonce that passes this screen still has
//!
//!     P(phase-clean) = e^-3.80 = 2.2e-2
//!
//! A screen hit is therefore a **CANDIDATE requiring full-harness
//! confirmation**, never a clean seed. Expect roughly 45 candidates per true
//! seed. Reporting a hit as a clean seed would be the same class of error as
//! the lazy-XOF bug in `src/point_add/memory/04-traps.md` section 4.
//!
//! NOT reproduced at all: phase-garbage, ancilla-garbage, avgT. avgT depends on
//! the Hmr/R stream and is W=64-harness-order only (04-traps.md section 4);
//! this binary must never report it.
//!
//! ## Building it
//!
//! It is deliberately NOT under `src/bin/`, so cargo does not auto-discover it
//! and it cannot affect `cargo build` or `./benchmark.sh` in the submission
//! tree. Copy it into `src/bin/` of a THROWAWAY copy of the repo — never the
//! submission tree — and `cargo build --release --bin screen`. It links the
//! `quantum_ecc` lib and adds no dependencies.
//!
//!     ./screen --nonces LIST --mode count  --out OUT.tsv   # all 9024 shots
//!     ./screen --nonces LIST --mode ladder --out OUT.tsv   # 512/2048/8192/9024
//!
//! `--mode count` is what the correctness gate needs; `--mode ladder` is the
//! fast path and stops at the first rung showing a mismatch.

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
}

/// Classical-only replica of eval_circuit::run_tests.
///
/// `target_shots` selects the ladder rung. Per 04-traps.md section 4 ALL pairs
/// for the rung are drawn BEFORE the Simulator is constructed, so the simulator
/// never consumes XOF bytes the input draw still needs.
fn run_classical(
    ops: &[Op],
    regs: &[Vec<QubitOrBit>],
    total_qubits: u64,
    num_bits: u64,
    prefix: &Shake256,
    target_shots: usize,
) -> Trial {
    // Per-nonce absorb: only the 96-op tail.
    let mut h = prefix.clone();
    for op in &ops[ops.len() - TAIL..] {
        absorb_op(&mut h, op);
    }

    // Stream fingerprint: SHAKE256 over the ENTIRE op stream. Two distinct
    // nonces yielding the same fp means the tail edit did not reach the stream.
    let mut fp_reader = h.clone().finalize_xof();
    let mut fp_bytes = [0u8; 16];
    XofReader::read(&mut fp_reader, &mut fp_bytes);
    let fp = fp_bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();

    let mut xof = h.finalize_xof();
    let curve = secp256k1();

    // --- draw every pair up front ---
    let mut targets = Vec::with_capacity(target_shots);
    let mut offsets = Vec::with_capacity(target_shots);
    let mut expected = Vec::with_capacity(target_shots);
    for _ in 0..target_shots {
        let mut rb = [[0u8; 32]; 2];
        XofReader::read(&mut xof, &mut rb[0]);
        XofReader::read(&mut xof, &mut rb[1]);
        let k1 = U256::from_le_bytes(rb[0]);
        let k2 = U256::from_le_bytes(rb[1]);
        let t = curve.mul(curve.gx, curve.gy, k1);
        let o = curve.mul(curve.gx, curve.gy, k2);
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

    // --- simulate ---
    let mut sim = Simulator::new(total_qubits as usize, num_bits as usize, &mut xof);
    let mut classical = 0usize;
    const BATCH: usize = 64;
    let num_batches = (n + BATCH - 1) / BATCH;
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
    Trial { classical, n_shots: n, fp }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut nonce_file = String::new();
    let mut mode = "count".to_string();
    let mut out = "-".to_string();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--nonces" => { nonce_file = args[i + 1].clone(); i += 2; }
            "--mode"   => { mode = args[i + 1].clone(); i += 2; }
            "--out"    => { out = args[i + 1].clone(); i += 2; }
            other => { eprintln!("unknown arg {other}"); std::process::exit(2); }
        }
    }

    let nonces: Vec<u64> = std::fs::read_to_string(&nonce_file)
        .expect("read nonce file")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().parse().expect("parse nonce"))
        .collect();

    let t_build = Instant::now();
    let mut ops = point_add::build();
    let build_ms = t_build.elapsed().as_millis();
    let (total_qubits, num_bits, _nregs, regs) = analyze_ops(ops.iter());
    assert_eq!(regs.len(), 4, "expected 4 registers");

    // Absorb everything except the 96-op tail, once.
    let t_prefix = Instant::now();
    let mut prefix = Shake256::default();
    prefix.update(b"quantum_ecc-fiat-shamir-v2");
    prefix.update(&(ops.len() as u64).to_le_bytes());
    for op in &ops[..ops.len() - TAIL] {
        absorb_op(&mut prefix, op);
    }
    let prefix_ms = t_prefix.elapsed().as_millis();

    eprintln!(
        "screen: ops={} qubits={} bits={} build={}ms prefix_absorb={}ms mode={}",
        ops.len(), total_qubits, num_bits, build_ms, prefix_ms, mode
    );

    let mut sink: Box<dyn Write> = if out == "-" {
        Box::new(std::io::stdout())
    } else {
        Box::new(std::fs::File::create(&out).expect("create out"))
    };
    writeln!(sink, "nonce\tclassical\tn_shots\trung\tms\tstream_fp").unwrap();

    for nonce in nonces {
        patch_tail(&mut ops, nonce);
        let t0 = Instant::now();
        let (classical, n_shots, rung, fp) = if mode == "count" {
            let r = run_classical(&ops, &regs, total_qubits, num_bits, &prefix, FULL_SHOTS);
            (r.classical, r.n_shots, FULL_SHOTS, r.fp)
        } else {
            // Ladder: stop at the first rung that shows a mismatch.
            let mut last = Trial { classical: 0, n_shots: 0, fp: String::new() };
            let mut rung_hit = 0usize;
            for &rung in SHOT_LADDER.iter() {
                last = run_classical(&ops, &regs, total_qubits, num_bits, &prefix, rung);
                rung_hit = rung;
                if last.classical > 0 {
                    break;
                }
            }
            (last.classical, last.n_shots, rung_hit, last.fp)
        };
        let ms = t0.elapsed().as_millis();
        writeln!(sink, "{nonce}\t{classical}\t{n_shots}\t{rung}\t{ms}\t{fp}").unwrap();
        sink.flush().unwrap();
    }
}
