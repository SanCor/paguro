//! Zobrist hashing.

use crate::types::{Color, Piece, Square, CASTLING_ANY};
use std::sync::OnceLock;

pub struct Zobrist {
    pub psq: [[[u64; 64]; 6]; 2],
    pub castling: [u64; 16],
    pub ep: [u64; 8],
    pub side: u64,
}

static ZOBRIST: OnceLock<Zobrist> = OnceLock::new();

pub fn init() {
    let _ = keys();
}

pub fn keys() -> &'static Zobrist {
    ZOBRIST.get_or_init(Zobrist::new)
}

impl Zobrist {
    fn new() -> Zobrist {
        let mut state = 0xA1B2_C3D4_E5F6_7788u64;
        let mut psq = [[[0u64; 64]; 6]; 2];
        for c in 0..2 {
            for pt in 0..6 {
                for sq in 0..64 {
                    psq[c][pt][sq] = splitmix(&mut state);
                }
            }
        }
        let mut castling = [0u64; 16];
        for i in 0..16 {
            castling[i] = splitmix(&mut state);
        }
        let mut ep = [0u64; 8];
        for i in 0..8 {
            ep[i] = splitmix(&mut state);
        }
        Zobrist {
            psq,
            castling,
            ep,
            side: splitmix(&mut state),
        }
    }
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

#[inline]
pub fn psq(color: Color, pt: usize, sq: Square) -> u64 {
    keys().psq[color.idx()][pt][sq.idx()]
}

#[inline]
pub fn piece_key(p: Piece, sq: Square) -> u64 {
    use crate::types::{piece_color, piece_type};
    psq(piece_color(p), piece_type(p).idx(), sq)
}

#[inline]
pub fn castle_key(cr: u8) -> u64 {
    keys().castling[(cr & CASTLING_ANY) as usize]
}

#[inline]
pub fn ep_key(file: u8) -> u64 {
    keys().ep[file as usize]
}

#[inline]
pub fn side_key() -> u64 {
    keys().side
}
