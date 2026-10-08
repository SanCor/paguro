//! NNUE evaluation (Stockfish 14 HalfKAv2).

pub mod accumulator;
pub mod features;
pub mod network;
mod simd;

use crate::board::Position;
use crate::types::Color;
use network::Network;

pub fn evaluate(pos: &mut Position, net: &Network) -> i32 {
    accumulator::ensure(pos, Color::White, net);
    accumulator::ensure(pos, Color::Black, net);

    let bucket = ((pos.piece_count() as usize).saturating_sub(1)) / 4;
    let bucket = bucket.min(7);

    let stm = pos.side;
    let nstm = stm.flip();

    let psqt = (pos.acc().psqt[stm.idx()][bucket] - pos.acc().psqt[nstm.idx()][bucket]) / 2;

    let mut input = [0u8; 1024];
    simd::clamp_acc(&pos.acc().accumulation[stm.idx()], &mut input[0..512]);
    simd::clamp_acc(&pos.acc().accumulation[nstm.idx()], &mut input[512..1024]);

    let positional = net.propagate(bucket, &input);
    (psqt + positional) / 16
}
