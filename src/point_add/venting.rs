//! Gidney 2025 venting adder primitives (arxiv 2507.23079).
//!
//! These primitives implement classical-quantum addition with O(1) clean
//! ancilla qubits, by "venting" carry qubits (measuring them in X basis
//! and deferring the corresponding phase-flip tasks to the end via
//! Häner-Roetteler-Soeken's carry-xor construction).
//!
//! Python reference: https://zenodo.org/doi/10.5281/zenodo.15866587
//!
//! The key primitives:
//! - [`xor_right_shifted_carries_into`]: Häner carry-xor.
//!   Performs `Q_dst ^= carry(Q_src, offset, carry_in) >> 1` in ~2n CCX
//!   using 0 clean ancilla.
//! - [`add_vented_2clean`]: streaming vented add. 2 clean ancilla, ~n CCX,
//!   leaves n-2 phase-flip tasks behind.
//! - [`iadd_3clean`]: full const-quantum add. 3 clean ancilla, 4n CCX.
//!
//! Status: initial port, API subject to change. Tests in the unit-test
//! module at the bottom.

use super::{B, BitId, QubitId};

/// Performs `Q_dst ^= carry(Q_src, offset, carry_in) >> 1` in-place.
///
/// Here `carry(x, d, c0)` returns an n-bit value where bit k is the carry
/// into bit k of the addition `x + d + c0` (with c0 being the bit-0
/// carry-in). The `>> 1` means we skip the LSB of the carry (which equals
/// the carry-in and is trivially accessible).
///
/// `offset` may be classical or quantum. When classical, `offset[k]` is
/// a `BitId` whose value is the k-th bit of the constant offset. When
/// quantum, `offset[k]` is a `QubitId`.
///
/// Cost: ~2n CCX, 0 clean ancilla.
///
/// # Arguments
/// - `q_src`: n+1 qubits (or n) representing the "target" of the
///   reference addition.
/// - `offset`: n classical bits (the constant to add).
/// - `q_dst`: n qubits to XOR the right-shifted carries into.
/// - `carry_in`: classical bit (0 or 1) for the LSB carry-in.
#[allow(dead_code)]
pub fn xor_right_shifted_carries_into_classical(
    b: &mut B,
    q_src: &[QubitId],
    offset_bits: u64,
    q_dst: &[QubitId],
    carry_in: bool,
) {
    let n = q_dst.len();
    assert!(n <= q_src.len() && q_src.len() <= n + 1, "len mismatch");
    if n == 0 {
        return;
    }

    // Helper: bit k of the classical offset.
    let bit = |k: usize| -> bool { (offset_bits >> k) & 1 != 0 };

    // Helper: apply CCX(ctrl_a, ctrl_b, target) with each control
    // possibly classically-inverted. The original `a ^ offset[k]` means:
    // if offset[k] = 0, use `a` directly; if offset[k] = 1, use `NOT a`.
    // We implement this via `X(a)` before and after the CCX.
    let ccx_inv =
        |b: &mut B, ctrl_a: QubitId, inv_a: bool, ctrl_b: QubitId, inv_b: bool, target: QubitId| {
            if inv_a {
                b.x(ctrl_a);
            }
            if inv_b {
                b.x(ctrl_b);
            }
            b.ccx(ctrl_a, ctrl_b, target);
            if inv_b {
                b.x(ctrl_b);
            }
            if inv_a {
                b.x(ctrl_a);
            }
        };

    // First loop (reversed over k=1..n):
    //   ccx(Q_src[k] ^ offset[k], Q_dst[k-1], Q_dst[k])
    for k in (1..n).rev() {
        ccx_inv(b, q_src[k], bit(k), q_dst[k - 1], false, q_dst[k]);
    }

    // broadcast_cx(offset, Q_dst): for each k, if offset[k]: X(Q_dst[k]).
    // (This is equivalent to XORing the classical offset into Q_dst.)
    for k in 0..n {
        if bit(k) {
            b.x(q_dst[k]);
        }
    }

    // ccx(Q_src[0] ^ offset[0], carry_in ^ offset[0], Q_dst[0])
    // carry_in is CLASSICAL here. If (carry_in XOR offset[0]) = 0, the
    // CCX has a classical-0 control and does nothing. If it's 1, the CCX
    // reduces to CX(q_src[0] with inv, q_dst[0]).
    let carry_in_xor_offset0 = carry_in ^ bit(0);
    if carry_in_xor_offset0 {
        // CX(q_src[0] ^ offset[0], q_dst[0]).
        if bit(0) {
            b.x(q_src[0]);
        }
        b.cx(q_src[0], q_dst[0]);
        if bit(0) {
            b.x(q_src[0]);
        }
    }

    // Second loop (k=1..n):
    //   ccx(Q_src[k] ^ offset[k], Q_dst[k-1] ^ offset[k], Q_dst[k])
    for k in 1..n {
        ccx_inv(b, q_src[k], bit(k), q_dst[k - 1], bit(k), q_dst[k]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Simulator;
    use sha3::{
        digest::{ExtendableOutput, Update},
        Shake256,
    };

    /// Classical reference: compute bit-k of carry(x, d, cin).
    /// The carry bit into position k (c_k) is defined by:
    ///   c_0 = cin
    ///   c_{k+1} = MAJ(c_k, x_k, d_k)
    fn classical_carry(x: u64, d: u64, cin: bool, n: usize) -> u64 {
        // Compute bit-by-bit.
        let mut c: u64 = 0;
        let mut prev = cin;
        for k in 0..n {
            let xk = (x >> k) & 1 != 0;
            let dk = (d >> k) & 1 != 0;
            // new carry = MAJ(prev, xk, dk)
            let new_carry = (prev && xk) || (prev && dk) || (xk && dk);
            if new_carry {
                c |= 1 << (k + 1);
            }
            prev = new_carry;
        }
        // Also set bit 0 to cin (the "carry into bit 0")
        if cin {
            c |= 1;
        }
        c
    }

    fn run_xor_rsh_carries(n: usize, trials: usize) -> bool {
        let mut hasher = Shake256::default();
        hasher.update(&[n as u8, trials as u8, 42]);
        use sha3::digest::XofReader;
        let mut xof =
            <sha3::Shake256 as sha3::digest::ExtendableOutput>::finalize_xof(hasher);
        for _trial in 0..trials {
            let mut buf = [0u8; 32];
            xof.read(&mut buf);
            let src_raw = u64::from_le_bytes(buf[0..8].try_into().unwrap());
            let dst_raw = u64::from_le_bytes(buf[8..16].try_into().unwrap());
            let offset_raw = u64::from_le_bytes(buf[16..24].try_into().unwrap());
            let cin_raw = buf[24];
            let src = if n < 64 {
                src_raw & ((1u64 << n) - 1)
            } else {
                src_raw
            };
            let dst = if n < 64 {
                dst_raw & ((1u64 << n) - 1)
            } else {
                dst_raw
            };
            let offset = if n < 64 {
                offset_raw & ((1u64 << n) - 1)
            } else {
                offset_raw
            };
            let cin = (cin_raw & 1) != 0;

            // Build circuit with src, dst qubits.
            let mut bb = B::new();
            let q_src: Vec<QubitId> = bb.alloc_qubits(n);
            let q_dst: Vec<QubitId> = bb.alloc_qubits(n);

            xor_right_shifted_carries_into_classical(
                &mut bb,
                &q_src,
                offset,
                &q_dst,
                cin,
            );

            let ops = bb.ops.clone();
            let num_qubits = bb.next_qubit as usize;
            let num_bits = 0usize;
            let mut inner_hasher = Shake256::default();
            inner_hasher.update(&[77u8]);
            let mut inner_xof = <sha3::Shake256 as sha3::digest::ExtendableOutput>::finalize_xof(inner_hasher);
            let mut sim = Simulator::new(num_qubits, num_bits, &mut inner_xof);
            sim.clear_for_shot();
            // Set src[k] = (src >> k) & 1 for shot 0.
            for k in 0..n {
                if (src >> k) & 1 != 0 {
                    *sim.qubit_mut(q_src[k]) = 1; // set bit for shot 0
                }
                if (dst >> k) & 1 != 0 {
                    *sim.qubit_mut(q_dst[k]) = 1;
                }
            }
            sim.apply(&ops);

            let expected_carries = classical_carry(src, offset, cin, n + 1);
            let expected_rsh = expected_carries >> 1; // carries shifted right by 1
            let expected_dst = (dst ^ expected_rsh) & ((1u64 << n) - 1);

            let mut got_dst: u64 = 0;
            for k in 0..n {
                if sim.qubit(q_dst[k]) & 1 != 0 {
                    got_dst |= 1 << k;
                }
            }
            if got_dst != expected_dst {
                eprintln!(
                    "n={} src={:#x} dst={:#x} offset={:#x} cin={} got={:#x} exp={:#x}",
                    n, src, dst, offset, cin, got_dst, expected_dst
                );
                return false;
            }
        }
        true
    }

    #[test]
    fn test_xor_rsh_carries_small() {
        for n in 1..=8 {
            assert!(run_xor_rsh_carries(n, 20), "failed at n={n}");
        }
    }
}
