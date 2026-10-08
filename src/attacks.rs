//! Attack tables for leapers and sliders.

use crate::bitboard::{north_east, north_west, south_east, south_west, try_offset};
use crate::magics;
use crate::types::{Color, Square};
use std::sync::OnceLock;

pub struct AttackTables {
    pub pawn: [[u64; 64]; 2],
    pub knight: [u64; 64],
    pub king: [u64; 64],
    pub between: [[u64; 64]; 64],
}

static TABLES: OnceLock<AttackTables> = OnceLock::new();

pub fn init() {
    magics::init();
    let _ = tables();
}

fn tables() -> &'static AttackTables {
    TABLES.get_or_init(AttackTables::new)
}

impl AttackTables {
    fn new() -> AttackTables {
        magics::init();
        let mut pawn = [[0u64; 64]; 2];
        let mut knight = [0u64; 64];
        let mut king = [0u64; 64];
        let mut between = [[0u64; 64]; 64];

        const KNIGHT_D: [(i8, i8); 8] = [
            (1, 2),
            (1, -2),
            (-1, 2),
            (-1, -2),
            (2, 1),
            (2, -1),
            (-2, 1),
            (-2, -1),
        ];
        const KING_D: [(i8, i8); 8] = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];

        for sq in 0..64u8 {
            let s = Square(sq);
            let b = s.bb();
            pawn[Color::White.idx()][sq as usize] = north_east(b) | north_west(b);
            pawn[Color::Black.idx()][sq as usize] = south_east(b) | south_west(b);
            for &(df, dr) in &KNIGHT_D {
                if let Some(n) = try_offset(s, df, dr) {
                    knight[sq as usize] |= n.bb();
                }
            }
            for &(df, dr) in &KING_D {
                if let Some(n) = try_offset(s, df, dr) {
                    king[sq as usize] |= n.bb();
                }
            }
        }

        for a in 0..64u8 {
            for bsq in 0..64u8 {
                if a == bsq {
                    continue;
                }
                let sa = Square(a);
                let sb = Square(bsq);
                let occ = sa.bb() | sb.bb();
                let attacks = if sa.file() == sb.file() || sa.rank() == sb.rank() {
                    magics::rook_attacks(sa, occ)
                } else if (sa.file() as i8 - sb.file() as i8).abs()
                    == (sa.rank() as i8 - sb.rank() as i8).abs()
                {
                    magics::bishop_attacks(sa, occ)
                } else {
                    0
                };
                if attacks & sb.bb() != 0 {
                    between[a as usize][bsq as usize] = magics::queen_attacks(sa, occ)
                        & magics::queen_attacks(sb, occ);
                }
            }
        }

        AttackTables {
            pawn,
            knight,
            king,
            between,
        }
    }
}

#[inline]
pub fn pawn_attacks(color: Color, sq: Square) -> u64 {
    tables().pawn[color.idx()][sq.idx()]
}

#[inline]
pub fn knight_attacks(sq: Square) -> u64 {
    tables().knight[sq.idx()]
}

#[inline]
pub fn king_attacks(sq: Square) -> u64 {
    tables().king[sq.idx()]
}

#[inline]
pub fn between(a: Square, b: Square) -> u64 {
    tables().between[a.idx()][b.idx()]
}

#[inline]
pub fn bishop_attacks(sq: Square, occ: u64) -> u64 {
    magics::bishop_attacks(sq, occ)
}

#[inline]
pub fn rook_attacks(sq: Square, occ: u64) -> u64 {
    magics::rook_attacks(sq, occ)
}

#[inline]
pub fn queen_attacks(sq: Square, occ: u64) -> u64 {
    magics::queen_attacks(sq, occ)
}
