//! Incremental HalfKAv2 feature transformer.

use crate::board::{DirtyPiece, Position};
use crate::nnue::features;
use crate::nnue::network::{Network, FT_OUT, PSQT_BUCKETS};
use crate::types::*;

pub fn ensure(pos: &mut Position, perspective: Color, net: &Network) {
    if pos.acc().computed[perspective.idx()] {
        return;
    }
    if features::king_moved(pos, perspective) || pos.acc_ply == 0 {
        refresh(pos, perspective, net);
        return;
    }
    let parent = pos.acc_ply - 1;
    let pidx = perspective.idx();
    if !pos.acc_stack[parent].computed[pidx] {
        refresh(pos, perspective, net);
        return;
    }

    pos.acc_stack[pos.acc_ply].accumulation[pidx] = pos.acc_stack[parent].accumulation[pidx];
    pos.acc_stack[pos.acc_ply].psqt[pidx] = pos.acc_stack[parent].psqt[pidx];

    let ksq = features::king_oriented(pos, perspective);
    let dirty_num = pos.dirty_num as usize;
    let dirty = pos.dirty;
    for i in 0..dirty_num {
        apply_dirty(&mut pos.acc_stack[pos.acc_ply], perspective, dirty[i], ksq, net);
    }
    pos.acc_stack[pos.acc_ply].computed[pidx] = true;
}

fn apply_dirty(
    acc: &mut crate::board::Accumulator,
    perspective: Color,
    d: DirtyPiece,
    ksq: Square,
    net: &Network,
) {
    if d.from.is_ok() {
        let idx = features::make_index(perspective, d.from, d.piece, ksq);
        add_feature(acc, perspective, idx, net, -1);
    }
    if d.to.is_ok() {
        let idx = features::make_index(perspective, d.to, d.piece, ksq);
        add_feature(acc, perspective, idx, net, 1);
    }
}

fn add_feature(
    acc: &mut crate::board::Accumulator,
    perspective: Color,
    index: usize,
    net: &Network,
    sign: i16,
) {
    let pidx = perspective.idx();
    let off = index * FT_OUT;
    let w = &net.ft_weights[off..off + FT_OUT];
    crate::nnue::simd::add_i16x512(&mut acc.accumulation[pidx], w, sign);
    let po = index * PSQT_BUCKETS;
    for b in 0..PSQT_BUCKETS {
        acc.psqt[pidx][b] += net.psqt[po + b] * sign as i32;
    }
}

pub fn refresh(pos: &mut Position, perspective: Color, net: &Network) {
    let pidx = perspective.idx();
    pos.acc_mut().accumulation[pidx].copy_from_slice(&net.ft_biases);
    pos.acc_mut().psqt[pidx] = [0; 8];

    let mut active = Vec::with_capacity(32);
    features::append_active(pos, perspective, &mut active);
    for idx in active {
        let off = idx * FT_OUT;
        let w = &net.ft_weights[off..off + FT_OUT];
        let acc = pos.acc_mut();
        crate::nnue::simd::add_i16x512(&mut acc.accumulation[pidx], w, 1);
        let po = idx * PSQT_BUCKETS;
        for b in 0..PSQT_BUCKETS {
            acc.psqt[pidx][b] += net.psqt[po + b];
        }
    }
    pos.acc_mut().computed[pidx] = true;
}
