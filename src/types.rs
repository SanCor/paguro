//! Core chess types: colors, pieces, squares, and packed moves.

use std::fmt;

pub const MAX_MOVES: usize = 256;
pub const MAX_PLY: usize = 128;

pub const MATE: i32 = 32_000;
pub const MATE_IN_MAX: i32 = MATE - MAX_PLY as i32;
pub const INF: i32 = MATE + 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Color {
    White = 0,
    Black = 1,
}

impl Color {
    #[inline]
    pub const fn flip(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    #[inline]
    pub const fn idx(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PieceType {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5,
}

impl PieceType {
    #[inline]
    pub const fn idx(self) -> usize {
        self as usize
    }

    pub const ALL: [PieceType; 6] = [
        PieceType::Pawn,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Queen,
        PieceType::King,
    ];

    pub fn from_promo_char(c: u8) -> Option<PieceType> {
        match c {
            b'n' | b'N' => Some(PieceType::Knight),
            b'b' | b'B' => Some(PieceType::Bishop),
            b'r' | b'R' => Some(PieceType::Rook),
            b'q' | b'Q' => Some(PieceType::Queen),
            _ => None,
        }
    }

    pub fn promo_char(self) -> u8 {
        match self {
            PieceType::Knight => b'n',
            PieceType::Bishop => b'b',
            PieceType::Rook => b'r',
            PieceType::Queen => b'q',
            _ => b'?',
        }
    }
}

/// Packed piece: 0 empty, otherwise `type + 1 + color * 8` (Stockfish layout).
pub type Piece = u8;

pub const NO_PIECE: Piece = 0;

#[inline]
pub const fn make_piece(color: Color, pt: PieceType) -> Piece {
    (pt as u8 + 1) + (color as u8) * 8
}

#[inline]
pub const fn piece_color(p: Piece) -> Color {
    if p < 8 {
        Color::White
    } else {
        Color::Black
    }
}

#[inline]
pub const fn piece_type(p: Piece) -> PieceType {
    match p & 7 {
        1 => PieceType::Pawn,
        2 => PieceType::Knight,
        3 => PieceType::Bishop,
        4 => PieceType::Rook,
        5 => PieceType::Queen,
        _ => PieceType::King,
    }
}

pub const WHITE_OO: u8 = 1;
pub const WHITE_OOO: u8 = 2;
pub const BLACK_OO: u8 = 4;
pub const BLACK_OOO: u8 = 8;
pub const CASTLING_ANY: u8 = 15;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Square(pub u8);

pub const SQ_NONE: Square = Square(64);

pub const A1: Square = Square(0);
pub const B1: Square = Square(1);
pub const C1: Square = Square(2);
pub const D1: Square = Square(3);
pub const E1: Square = Square(4);
pub const F1: Square = Square(5);
pub const G1: Square = Square(6);
pub const H1: Square = Square(7);
pub const A8: Square = Square(56);
pub const B8: Square = Square(57);
pub const C8: Square = Square(58);
pub const D8: Square = Square(59);
pub const E8: Square = Square(60);
pub const F8: Square = Square(61);
pub const G8: Square = Square(62);
pub const H8: Square = Square(63);

impl Square {
    #[inline]
    pub const fn new(file: u8, rank: u8) -> Square {
        Square(file + rank * 8)
    }

    #[inline]
    pub const fn idx(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn file(self) -> u8 {
        self.0 & 7
    }

    #[inline]
    pub const fn rank(self) -> u8 {
        self.0 >> 3
    }

    #[inline]
    pub const fn is_ok(self) -> bool {
        self.0 < 64
    }

    #[inline]
    pub const fn bb(self) -> u64 {
        1u64 << self.0
    }

    #[inline]
    pub const fn flip(self) -> Square {
        Square(self.0 ^ 56)
    }

    pub fn from_uci(s: &str) -> Option<Square> {
        let b = s.as_bytes();
        if b.len() != 2 {
            return None;
        }
        let file = b[0].wrapping_sub(b'a');
        let rank = b[1].wrapping_sub(b'1');
        if file < 8 && rank < 8 {
            Some(Square::new(file, rank))
        } else {
            None
        }
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 >= 64 {
            return write!(f, "-");
        }
        let file = (b'a' + self.file()) as char;
        let rank = (b'1' + self.rank()) as char;
        write!(f, "{file}{rank}")
    }
}

pub const FLAG_NORMAL: u8 = 0;
pub const FLAG_PROMO_N: u8 = 1;
pub const FLAG_PROMO_B: u8 = 2;
pub const FLAG_PROMO_R: u8 = 3;
pub const FLAG_PROMO_Q: u8 = 4;
pub const FLAG_EN_PASSANT: u8 = 5;
pub const FLAG_CASTLE: u8 = 6;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Move(pub u16);

impl Move {
    pub const NONE: Move = Move(0);

    #[inline]
    pub const fn new(from: Square, to: Square, flag: u8) -> Move {
        Move((from.0 as u16) | ((to.0 as u16) << 6) | ((flag as u16) << 12))
    }

    #[inline]
    pub const fn from(self) -> Square {
        Square((self.0 & 63) as u8)
    }

    #[inline]
    pub const fn to(self) -> Square {
        Square(((self.0 >> 6) & 63) as u8)
    }

    #[inline]
    pub const fn flag(self) -> u8 {
        (self.0 >> 12) as u8
    }

    #[inline]
    pub const fn is_none(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn is_promotion(self) -> bool {
        let f = self.flag();
        f >= FLAG_PROMO_N && f <= FLAG_PROMO_Q
    }

    #[inline]
    pub const fn is_capture_flag(self) -> bool {
        self.flag() == FLAG_EN_PASSANT
    }

    #[inline]
    pub const fn is_castle(self) -> bool {
        self.flag() == FLAG_CASTLE
    }

    #[inline]
    pub const fn is_en_passant(self) -> bool {
        self.flag() == FLAG_EN_PASSANT
    }

    pub const fn promo_piece(self) -> Option<PieceType> {
        match self.flag() {
            FLAG_PROMO_N => Some(PieceType::Knight),
            FLAG_PROMO_B => Some(PieceType::Bishop),
            FLAG_PROMO_R => Some(PieceType::Rook),
            FLAG_PROMO_Q => Some(PieceType::Queen),
            _ => None,
        }
    }

    pub fn from_uci(s: &str) -> Option<Move> {
        let b = s.as_bytes();
        if b.len() < 4 {
            return None;
        }
        let from = Square::from_uci(&s[0..2])?;
        let to = Square::from_uci(&s[2..4])?;
        let flag = if b.len() >= 5 {
            match PieceType::from_promo_char(b[4])? {
                PieceType::Knight => FLAG_PROMO_N,
                PieceType::Bishop => FLAG_PROMO_B,
                PieceType::Rook => FLAG_PROMO_R,
                PieceType::Queen => FLAG_PROMO_Q,
                _ => return None,
            }
        } else {
            FLAG_NORMAL
        };
        Some(Move::new(from, to, flag))
    }

    pub fn to_uci(self) -> String {
        if self.is_none() {
            return "0000".to_string();
        }
        let mut s = format!("{}{}", self.from(), self.to());
        if let Some(pt) = self.promo_piece() {
            s.push(pt.promo_char() as char);
        }
        s
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_uci())
    }
}

pub fn material_value(pt: PieceType) -> i32 {
    match pt {
        PieceType::Pawn => 100,
        PieceType::Knight => 320,
        PieceType::Bishop => 330,
        PieceType::Rook => 500,
        PieceType::Queen => 900,
        PieceType::King => 0,
    }
}

/// Mask of castling rights cleared when a piece moves from or is captured on `sq`.
pub const CASTLING_MASK: [u8; 64] = {
    let mut m = [CASTLING_ANY; 64];
    m[A1.0 as usize] = CASTLING_ANY & !WHITE_OOO;
    m[H1.0 as usize] = CASTLING_ANY & !WHITE_OO;
    m[E1.0 as usize] = CASTLING_ANY & !(WHITE_OO | WHITE_OOO);
    m[A8.0 as usize] = CASTLING_ANY & !BLACK_OOO;
    m[H8.0 as usize] = CASTLING_ANY & !BLACK_OO;
    m[E8.0 as usize] = CASTLING_ANY & !(BLACK_OO | BLACK_OOO);
    m
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_uci_roundtrip() {
        assert_eq!(Square::from_uci("a1"), Some(A1));
        assert_eq!(Square::from_uci("h8"), Some(H8));
        assert_eq!(E1.to_string(), "e1");
    }

    #[test]
    fn move_pack() {
        let m = Move::new(E2, E4, FLAG_NORMAL);
        assert_eq!(m.from(), E2);
        assert_eq!(m.to(), E4);
        assert_eq!(m.to_uci(), "e2e4");
        let q = Move::new(E7, E8, FLAG_PROMO_Q);
        assert_eq!(q.to_uci(), "e7e8q");
    }
}

pub const E2: Square = Square(12);
pub const E4: Square = Square(28);
pub const E7: Square = Square(52);
