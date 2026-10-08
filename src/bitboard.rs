//! Bitboard helpers.

use crate::types::Square;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const ALL: Bitboard = Bitboard(!0);

    #[inline]
    pub const fn new(v: u64) -> Bitboard {
        Bitboard(v)
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn any(self) -> bool {
        self.0 != 0
    }

    #[inline]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    #[inline]
    pub const fn contains(self, sq: Square) -> bool {
        (self.0 & (1u64 << sq.0)) != 0
    }

    #[inline]
    pub const fn set(self, sq: Square) -> Bitboard {
        Bitboard(self.0 | (1u64 << sq.0))
    }

    #[inline]
    pub const fn clear(self, sq: Square) -> Bitboard {
        Bitboard(self.0 & !(1u64 << sq.0))
    }

    #[inline]
    pub const fn lsb(self) -> Square {
        Square(self.0.trailing_zeros() as u8)
    }

    #[inline]
    pub fn pop_lsb(&mut self) -> Square {
        let sq = self.lsb();
        self.0 &= self.0.wrapping_sub(1);
        sq
    }

    #[inline]
    pub const fn union(self, other: Bitboard) -> Bitboard {
        Bitboard(self.0 | other.0)
    }

    #[inline]
    pub const fn intersect(self, other: Bitboard) -> Bitboard {
        Bitboard(self.0 & other.0)
    }

    #[inline]
    pub const fn shift(self, dir: i32) -> Bitboard {
        if dir >= 0 {
            Bitboard(self.0 << dir)
        } else {
            Bitboard(self.0 >> (-dir))
        }
    }
}

impl std::ops::BitOr for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitand(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 & rhs.0)
    }
}

impl std::ops::BitXor for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitxor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 ^ rhs.0)
    }
}

impl std::ops::Not for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn not(self) -> Bitboard {
        Bitboard(!self.0)
    }
}

impl std::ops::BitOrAssign for Bitboard {
    #[inline]
    fn bitor_assign(&mut self, rhs: Bitboard) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAndAssign for Bitboard {
    #[inline]
    fn bitand_assign(&mut self, rhs: Bitboard) {
        self.0 &= rhs.0;
    }
}

impl std::ops::BitXorAssign for Bitboard {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Bitboard) {
        self.0 ^= rhs.0;
    }
}

impl IntoIterator for Bitboard {
    type Item = Square;
    type IntoIter = BitIter;
    fn into_iter(self) -> BitIter {
        BitIter(self.0)
    }
}

pub struct BitIter(u64);

impl Iterator for BitIter {
    type Item = Square;
    #[inline]
    fn next(&mut self) -> Option<Square> {
        if self.0 == 0 {
            None
        } else {
            let sq = Square(self.0.trailing_zeros() as u8);
            self.0 &= self.0.wrapping_sub(1);
            Some(sq)
        }
    }
}

pub const FILE_A: u64 = 0x0101_0101_0101_0101;
pub const FILE_H: u64 = 0x8080_8080_8080_8080;
pub const RANK_1: u64 = 0x0000_0000_0000_00FF;
pub const RANK_2: u64 = 0x0000_0000_0000_FF00;
pub const RANK_3: u64 = 0x0000_0000_00FF_0000;
pub const RANK_6: u64 = 0x0000_FF00_0000_0000;
pub const RANK_7: u64 = 0x00FF_0000_0000_0000;
pub const RANK_8: u64 = 0xFF00_0000_0000_0000;

#[inline]
pub const fn north(bb: u64) -> u64 {
    bb << 8
}

#[inline]
pub const fn south(bb: u64) -> u64 {
    bb >> 8
}

#[inline]
pub const fn east(bb: u64) -> u64 {
    (bb & !FILE_H) << 1
}

#[inline]
pub const fn west(bb: u64) -> u64 {
    (bb & !FILE_A) >> 1
}

#[inline]
pub const fn north_east(bb: u64) -> u64 {
    (bb & !FILE_H) << 9
}

#[inline]
pub const fn north_west(bb: u64) -> u64 {
    (bb & !FILE_A) << 7
}

#[inline]
pub const fn south_east(bb: u64) -> u64 {
    (bb & !FILE_H) >> 7
}

#[inline]
pub const fn south_west(bb: u64) -> u64 {
    (bb & !FILE_A) >> 9
}

pub fn try_offset(sq: Square, df: i8, dr: i8) -> Option<Square> {
    let f = sq.file() as i8 + df;
    let r = sq.rank() as i8 + dr;
    if (0..8).contains(&f) && (0..8).contains(&r) {
        Some(Square::new(f as u8, r as u8))
    } else {
        None
    }
}
