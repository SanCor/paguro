//! Legal move generation (pseudo-legal + king-safety filter).

use crate::attacks;
use crate::bitboard::{RANK_1, RANK_8};
use crate::board::Position;
use crate::types::*;

pub struct MoveList {
    pub moves: [Move; MAX_MOVES],
    pub scores: [i32; MAX_MOVES],
    pub len: usize,
}

impl MoveList {
    pub fn new() -> MoveList {
        MoveList {
            moves: [Move::NONE; MAX_MOVES],
            scores: [0; MAX_MOVES],
            len: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, m: Move) {
        self.moves[self.len] = m;
        self.len += 1;
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = Move> + '_ {
        self.moves[..self.len].iter().copied()
    }

    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len]
    }

    #[inline]
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Gen {
    All,
    Tactical,
    Quiet,
}

impl Position {
    pub fn legal_moves(&mut self) -> MoveList {
        let mut list = MoveList::new();
        self.gen(&mut list, Gen::All);
        self.filter_legal(&mut list, 0);
        list
    }

    pub fn capture_moves(&mut self) -> MoveList {
        let mut list = MoveList::new();
        self.gen(&mut list, Gen::Tactical);
        self.filter_legal(&mut list, 0);
        list
    }

    pub(crate) fn filter_legal(&mut self, list: &mut MoveList, start: usize) {
        let mut k = start;
        for i in start..list.len {
            let m = list.moves[i];
            // `gen` already guarantees pseudo-legality. Rechecking movement,
            // occupancy and pawn rules here was duplicated work for every move.
            if self.is_legal_generated(m) {
                list.moves[k] = m;
                k += 1;
            }
        }
        list.len = k;
    }

    pub fn is_legal(&mut self, m: Move) -> bool {
        if !self.is_pseudo_legal(m) {
            return false;
        }
        self.is_legal_generated(m)
    }

    fn is_legal_generated(&mut self, m: Move) -> bool {
        let m = self.decode_move(m);
        let us = self.side;
        let from = m.from();
        let to = m.to();

        if m.is_castle() {
            if self.in_check() {
                return false;
            }
            let step = if to.file() > from.file() { 1 } else { -1 };
            let mut f = from.file() as i8 + step;
            let dest = to.file() as i8;
            while f != dest {
                let sq = Square::new(f as u8, from.rank());
                if self.square_attacked(sq, us.flip()) {
                    return false;
                }
                f += step;
            }
            return !self.square_attacked(to, us.flip());
        }

        self.king_safe_after(m)
    }

    pub fn is_pseudo_legal(&self, m: Move) -> bool {
        let m = self.decode_move(m);
        if m.is_none() || m.from().0 > 63 || m.to().0 > 63 {
            return false;
        }
        let from = m.from();
        let to = m.to();
        let us = self.side;
        let p = self.piece_on(from);
        if p == NO_PIECE || piece_color(p) != us {
            return false;
        }
        let dest = self.piece_on(to);
        if dest != NO_PIECE && piece_color(dest) == us && !m.is_castle() {
            return false;
        }
        let pt = piece_type(p);
        let occ = self.occ();
        let them = us.flip();
        let theirs = self.by_color[them.idx()];

        if m.is_castle() {
            return self.castle_path_clear(us, to, occ);
        }

        if pt == PieceType::Pawn {
            let up: i8 = if us == Color::White { 8 } else { -8 };
            let from_i = from.0 as i8;
            let to_i = to.0 as i8;
            if m.is_en_passant() {
                return self.ep == to.0
                    && dest == NO_PIECE
                    && attacks::pawn_attacks(us, from) & to.bb() != 0;
            }
            let promo_rank = if us == Color::White { 7u8 } else { 0u8 };
            if m.is_promotion() != (to.rank() == promo_rank) {
                return false;
            }
            if dest == NO_PIECE {
                if to_i == from_i + up {
                    return true;
                }
                let start = if us == Color::White { 1u8 } else { 6u8 };
                return from.rank() == start
                    && to_i == from_i + 2 * up
                    && self.piece_on(Square((from_i + up) as u8)) == NO_PIECE;
            }
            return attacks::pawn_attacks(us, from) & to.bb() & theirs != 0;
        }

        let mut atk = match pt {
            PieceType::Knight => attacks::knight_attacks(from),
            PieceType::Bishop => attacks::bishop_attacks(from, occ),
            PieceType::Rook => attacks::rook_attacks(from, occ),
            PieceType::Queen => attacks::queen_attacks(from, occ),
            PieceType::King => attacks::king_attacks(from),
            PieceType::Pawn => 0,
        };
        atk &= !self.by_color[us.idx()];
        atk & to.bb() != 0
    }

    fn castle_path_clear(&self, us: Color, to: Square, occ: u64) -> bool {
        match (us, to) {
            (Color::White, G1) => {
                self.castling & WHITE_OO != 0
                    && occ & (F1.bb() | G1.bb()) == 0
                    && self.piece_on(H1) == make_piece(Color::White, PieceType::Rook)
            }
            (Color::White, C1) => {
                self.castling & WHITE_OOO != 0
                    && occ & (B1.bb() | C1.bb() | D1.bb()) == 0
                    && self.piece_on(A1) == make_piece(Color::White, PieceType::Rook)
            }
            (Color::Black, G8) => {
                self.castling & BLACK_OO != 0
                    && occ & (F8.bb() | G8.bb()) == 0
                    && self.piece_on(H8) == make_piece(Color::Black, PieceType::Rook)
            }
            (Color::Black, C8) => {
                self.castling & BLACK_OOO != 0
                    && occ & (B8.bb() | C8.bb() | D8.bb()) == 0
                    && self.piece_on(A8) == make_piece(Color::Black, PieceType::Rook)
            }
            _ => false,
        }
    }

    pub fn gen_pseudo(&self, list: &mut MoveList, captures_only: bool) {
        self.gen(
            list,
            if captures_only {
                Gen::Tactical
            } else {
                Gen::All
            },
        );
    }

    pub(crate) fn gen(&self, list: &mut MoveList, gen: Gen) {
        let us = self.side;
        let occ = self.occ();
        let ours = self.by_color[us.idx()];
        let theirs = self.by_color[us.flip().idx()];
        let empty = !occ;

        self.gen_pawns(list, gen, us, theirs, empty);
        self.gen_piece(list, gen, us, PieceType::Knight, occ, ours, theirs);
        self.gen_piece(list, gen, us, PieceType::Bishop, occ, ours, theirs);
        self.gen_piece(list, gen, us, PieceType::Rook, occ, ours, theirs);
        self.gen_piece(list, gen, us, PieceType::Queen, occ, ours, theirs);
        self.gen_piece(list, gen, us, PieceType::King, occ, ours, theirs);

        if gen != Gen::Tactical {
            self.gen_castling(list, us, occ);
        }
    }

    fn gen_pawns(
        &self,
        list: &mut MoveList,
        gen: Gen,
        us: Color,
        theirs: u64,
        empty: u64,
    ) {
        let pawns = self.pieces(us, PieceType::Pawn);
        let up: i8 = if us == Color::White { 8 } else { -8 };
        let promo_rank = if us == Color::White { RANK_8 } else { RANK_1 };

        if gen != Gen::Quiet {
            let mut pawns_bb = pawns;
            while pawns_bb != 0 {
                let from = Square(pawns_bb.trailing_zeros() as u8);
                pawns_bb &= pawns_bb - 1;
                let mut a = attacks::pawn_attacks(us, from) & theirs;
                while a != 0 {
                    let to = Square(a.trailing_zeros() as u8);
                    a &= a - 1;
                    if (to.bb() & promo_rank) != 0 {
                        push_promos(list, from, to);
                    } else {
                        list.push(Move::new(from, to, FLAG_NORMAL));
                    }
                }
            }
            if self.ep < 64 {
                let ep = Square(self.ep);
                let mut attackers = attacks::pawn_attacks(us.flip(), ep) & pawns;
                while attackers != 0 {
                    let from = Square(attackers.trailing_zeros() as u8);
                    attackers &= attackers - 1;
                    list.push(Move::new(from, ep, FLAG_EN_PASSANT));
                }
            }
        }

        let mut single = if us == Color::White {
            (pawns << 8) & empty
        } else {
            (pawns >> 8) & empty
        };
        if gen == Gen::Tactical {
            single &= promo_rank;
        }
        if gen != Gen::Tactical || single != 0 {
            let mut dbl = if gen != Gen::Tactical {
                if us == Color::White {
                    ((single & crate::bitboard::RANK_3) << 8) & empty
                } else {
                    ((single & crate::bitboard::RANK_6) >> 8) & empty
                }
            } else {
                0
            };
            while single != 0 {
                let to = Square(single.trailing_zeros() as u8);
                single &= single - 1;
                let from = Square((to.0 as i8 - up) as u8);
                if (to.bb() & promo_rank) != 0 {
                    if gen != Gen::Quiet {
                        push_promos(list, from, to);
                    }
                } else if gen != Gen::Tactical {
                    list.push(Move::new(from, to, FLAG_NORMAL));
                }
            }
            while dbl != 0 {
                let to = Square(dbl.trailing_zeros() as u8);
                dbl &= dbl - 1;
                let from = Square((to.0 as i8 - 2 * up) as u8);
                list.push(Move::new(from, to, FLAG_NORMAL));
            }
        }
    }

    fn gen_piece(
        &self,
        list: &mut MoveList,
        gen: Gen,
        us: Color,
        pt: PieceType,
        occ: u64,
        ours: u64,
        theirs: u64,
    ) {
        let mut bb = self.pieces(us, pt);
        while bb != 0 {
            let from = Square(bb.trailing_zeros() as u8);
            bb &= bb - 1;
            let mut atk = match pt {
                PieceType::Knight => attacks::knight_attacks(from),
                PieceType::Bishop => attacks::bishop_attacks(from, occ),
                PieceType::Rook => attacks::rook_attacks(from, occ),
                PieceType::Queen => attacks::queen_attacks(from, occ),
                PieceType::King => attacks::king_attacks(from),
                PieceType::Pawn => 0,
            };
            atk &= !ours;
            match gen {
                Gen::Tactical => atk &= theirs,
                Gen::Quiet => atk &= !theirs,
                Gen::All => {}
            }
            while atk != 0 {
                let to = Square(atk.trailing_zeros() as u8);
                atk &= atk - 1;
                list.push(Move::new(from, to, FLAG_NORMAL));
            }
        }
    }

    fn gen_castling(&self, list: &mut MoveList, us: Color, occ: u64) {
        if us == Color::White {
            if self.castling & WHITE_OO != 0
                && occ & (F1.bb() | G1.bb()) == 0
                && self.piece_on(H1) == make_piece(Color::White, PieceType::Rook)
            {
                list.push(Move::new(E1, G1, FLAG_CASTLE));
            }
            if self.castling & WHITE_OOO != 0
                && occ & (B1.bb() | C1.bb() | D1.bb()) == 0
                && self.piece_on(A1) == make_piece(Color::White, PieceType::Rook)
            {
                list.push(Move::new(E1, C1, FLAG_CASTLE));
            }
        } else {
            if self.castling & BLACK_OO != 0
                && occ & (F8.bb() | G8.bb()) == 0
                && self.piece_on(H8) == make_piece(Color::Black, PieceType::Rook)
            {
                list.push(Move::new(E8, G8, FLAG_CASTLE));
            }
            if self.castling & BLACK_OOO != 0
                && occ & (B8.bb() | C8.bb() | D8.bb()) == 0
                && self.piece_on(A8) == make_piece(Color::Black, PieceType::Rook)
            {
                list.push(Move::new(E8, C8, FLAG_CASTLE));
            }
        }
    }

    pub fn perft(&mut self, depth: u32) -> u64 {
        let moves = self.legal_moves();
        if depth == 1 {
            return moves.len as u64;
        }
        if depth == 0 {
            return 1;
        }
        let mut nodes = 0;
        for i in 0..moves.len {
            let m = moves.moves[i];
            self.make_unchecked(m);
            nodes += self.perft(depth - 1);
            self.unmake(m);
        }
        nodes
    }
}

fn push_promos(list: &mut MoveList, from: Square, to: Square) {
    list.push(Move::new(from, to, FLAG_PROMO_Q));
    list.push(Move::new(from, to, FLAG_PROMO_R));
    list.push(Move::new(from, to, FLAG_PROMO_B));
    list.push(Move::new(from, to, FLAG_PROMO_N));
}
