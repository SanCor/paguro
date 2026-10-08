//! HalfKAv2 feature indices (Stockfish 14).

use crate::board::Position;
use crate::types::*;

pub const PS_NB: usize = 11 * 64;
pub const DIMENSIONS: usize = 64 * PS_NB; // 45056

const PSI: [[u16; 16]; 2] = [
    [
        0,
        0,
        2 * 64,
        4 * 64,
        6 * 64,
        8 * 64,
        10 * 64,
        0,
        0,
        1 * 64,
        3 * 64,
        5 * 64,
        7 * 64,
        9 * 64,
        10 * 64,
        0,
    ],
    [
        0,
        1 * 64,
        3 * 64,
        5 * 64,
        7 * 64,
        9 * 64,
        10 * 64,
        0,
        0,
        0,
        2 * 64,
        4 * 64,
        6 * 64,
        8 * 64,
        10 * 64,
        0,
    ],
];

#[inline]
pub fn orient(perspective: Color, s: Square) -> Square {
    if perspective == Color::Black {
        s.flip()
    } else {
        s
    }
}

#[inline]
pub fn make_index(perspective: Color, s: Square, pc: Piece, ksq_oriented: Square) -> usize {
    orient(perspective, s).idx() + PSI[perspective.idx()][pc as usize] as usize + PS_NB * ksq_oriented.idx()
}

pub fn king_oriented(pos: &Position, perspective: Color) -> Square {
    orient(perspective, pos.king_sq(perspective))
}

pub fn append_active(pos: &Position, perspective: Color, out: &mut Vec<usize>) {
    let ksq = king_oriented(pos, perspective);
    for sq in 0..64u8 {
        let pc = pos.piece_on(Square(sq));
        if pc != NO_PIECE {
            out.push(make_index(perspective, Square(sq), pc, ksq));
        }
    }
}

pub fn king_moved(pos: &Position, perspective: Color) -> bool {
    for i in 0..pos.dirty_num as usize {
        let d = pos.dirty[i];
        if d.piece != NO_PIECE
            && piece_color(d.piece) == perspective
            && piece_type(d.piece) == PieceType::King
        {
            return true;
        }
    }
    false
}
