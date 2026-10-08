//! Position: bitboards, FEN, make/unmake.

use crate::attacks;
use crate::types::*;
use crate::zobrist;

pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[derive(Clone, Copy)]
pub struct DirtyPiece {
    pub piece: Piece,
    pub from: Square,
    pub to: Square,
}

#[derive(Clone)]
pub struct Accumulator {
    pub accumulation: [[i16; 512]; 2],
    pub psqt: [[i32; 8]; 2],
    pub computed: [bool; 2],
}

impl Default for Accumulator {
    fn default() -> Self {
        Accumulator {
            accumulation: [[0; 512]; 2],
            psqt: [[0; 8]; 2],
            computed: [false, false],
        }
    }
}

#[derive(Clone, Copy)]
pub struct State {
    pub key: u64,
    pub ep: u8,
    pub castling: u8,
    pub halfmove: u16,
    pub captured: Piece,
    pub played: Move,
    pub dirty_num: u8,
    pub dirty: [DirtyPiece; 3],
}

#[derive(Clone)]
pub struct Position {
    pub by_type: [u64; 6],
    pub by_color: [u64; 2],
    pub board: [Piece; 64],
    pub side: Color,
    pub ep: u8,
    pub castling: u8,
    pub halfmove: u16,
    pub fullmove: u16,
    pub key: u64,
    pub kings: [Square; 2],
    pub history: Vec<State>,
    pub dirty_num: u8,
    pub dirty: [DirtyPiece; 3],
    pub acc_stack: Vec<Accumulator>,
    pub acc_ply: usize,
}

impl Position {
    pub fn new() -> Position {
        Position::from_fen(START_FEN).expect("start fen")
    }

    pub fn empty() -> Position {
        Position {
            by_type: [0; 6],
            by_color: [0; 2],
            board: [NO_PIECE; 64],
            side: Color::White,
            ep: 64,
            castling: 0,
            halfmove: 0,
            fullmove: 1,
            key: 0,
            kings: [E1, E8],
            history: Vec::with_capacity(256),
            dirty_num: 0,
            dirty: [DirtyPiece {
                piece: NO_PIECE,
                from: SQ_NONE,
                to: SQ_NONE,
            }; 3],
            acc_stack: vec![Accumulator::default()],
            acc_ply: 0,
        }
    }

    pub fn from_fen(fen: &str) -> Result<Position, String> {
        let mut pos = Position::empty();
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.is_empty() {
            return Err("empty fen".into());
        }
        let mut sq_rank = 7i32;
        let mut sq_file = 0i32;
        for c in parts[0].chars() {
            match c {
                '/' => {
                    sq_file = 0;
                    sq_rank -= 1;
                }
                '1'..='8' => sq_file += (c as u8 - b'0') as i32,
                _ => {
                    let (color, pt) = char_to_piece(c).ok_or_else(|| format!("bad piece {c}"))?;
                    if !(0..8).contains(&sq_rank) || !(0..8).contains(&sq_file) {
                        return Err("fen overflow".into());
                    }
                    let sq = Square::new(sq_file as u8, sq_rank as u8);
                    pos.put_piece(make_piece(color, pt), sq);
                    sq_file += 1;
                }
            }
        }
        if parts.len() > 1 {
            pos.side = if parts[1].starts_with('b') {
                Color::Black
            } else {
                Color::White
            };
        }
        if parts.len() > 2 {
            pos.castling = 0;
            for c in parts[2].chars() {
                pos.castling |= match c {
                    'K' => WHITE_OO,
                    'Q' => WHITE_OOO,
                    'k' => BLACK_OO,
                    'q' => BLACK_OOO,
                    '-' => 0,
                    _ => 0,
                };
            }
        }
        if parts.len() > 3 && parts[3] != "-" {
            if let Some(sq) = Square::from_uci(parts[3]) {
                pos.ep = sq.0;
            }
        }
        if parts.len() > 4 {
            pos.halfmove = parts[4].parse().unwrap_or(0);
        }
        if parts.len() > 5 {
            pos.fullmove = parts[5].parse().unwrap_or(1);
        }
        pos.key = pos.compute_key();
        pos.acc_mut().computed = [false, false];
        Ok(pos)
    }

    #[inline]
    pub fn acc(&self) -> &Accumulator {
        &self.acc_stack[self.acc_ply]
    }

    #[inline]
    pub fn acc_mut(&mut self) -> &mut Accumulator {
        &mut self.acc_stack[self.acc_ply]
    }

    fn bump_acc(&mut self) {
        self.acc_ply += 1;
        if self.acc_stack.len() <= self.acc_ply {
            self.acc_stack.push(Accumulator::default());
        } else {
            self.acc_stack[self.acc_ply].computed = [false, false];
        }
    }

    pub fn to_fen(&self) -> String {
        let mut fen = String::new();
        for rank in (0..8).rev() {
            let mut empty = 0;
            for file in 0..8 {
                let p = self.board[Square::new(file, rank).idx()];
                if p == NO_PIECE {
                    empty += 1;
                } else {
                    if empty > 0 {
                        fen.push(char::from(b'0' + empty));
                        empty = 0;
                    }
                    fen.push(piece_to_char(p));
                }
            }
            if empty > 0 {
                fen.push(char::from(b'0' + empty));
            }
            if rank > 0 {
                fen.push('/');
            }
        }
        fen.push(' ');
        fen.push(if self.side == Color::White { 'w' } else { 'b' });
        fen.push(' ');
        if self.castling == 0 {
            fen.push('-');
        } else {
            if self.castling & WHITE_OO != 0 {
                fen.push('K');
            }
            if self.castling & WHITE_OOO != 0 {
                fen.push('Q');
            }
            if self.castling & BLACK_OO != 0 {
                fen.push('k');
            }
            if self.castling & BLACK_OOO != 0 {
                fen.push('q');
            }
        }
        fen.push(' ');
        if self.ep < 64 {
            fen.push_str(&Square(self.ep).to_string());
        } else {
            fen.push('-');
        }
        fen.push_str(&format!(" {} {}", self.halfmove, self.fullmove));
        fen
    }

    fn put_piece(&mut self, p: Piece, sq: Square) {
        self.board[sq.idx()] = p;
        let c = piece_color(p);
        let pt = piece_type(p);
        self.by_color[c.idx()] |= sq.bb();
        self.by_type[pt.idx()] |= sq.bb();
        if pt == PieceType::King {
            self.kings[c.idx()] = sq;
        }
    }

    fn remove_piece(&mut self, sq: Square) -> Piece {
        let p = self.board[sq.idx()];
        if p != NO_PIECE {
            let c = piece_color(p);
            let pt = piece_type(p);
            self.by_color[c.idx()] &= !sq.bb();
            self.by_type[pt.idx()] &= !sq.bb();
            self.board[sq.idx()] = NO_PIECE;
        }
        p
    }

    fn move_piece(&mut self, from: Square, to: Square) -> Piece {
        let p = self.board[from.idx()];
        if p == NO_PIECE {
            return NO_PIECE;
        }
        let c = piece_color(p);
        let pt = piece_type(p);
        let mask = from.bb() | to.bb();
        self.by_color[c.idx()] ^= mask;
        self.by_type[pt.idx()] ^= mask;
        self.board[from.idx()] = NO_PIECE;
        self.board[to.idx()] = p;
        if pt == PieceType::King {
            self.kings[c.idx()] = to;
        }
        p
    }

    #[inline]
    pub fn occ(&self) -> u64 {
        self.by_color[0] | self.by_color[1]
    }

    #[inline]
    pub fn pieces(&self, color: Color, pt: PieceType) -> u64 {
        self.by_color[color.idx()] & self.by_type[pt.idx()]
    }

    #[inline]
    pub fn pieces_color(&self, color: Color) -> u64 {
        self.by_color[color.idx()]
    }

    #[inline]
    pub fn piece_on(&self, sq: Square) -> Piece {
        self.board[sq.idx()]
    }

    #[inline]
    pub fn king_sq(&self, color: Color) -> Square {
        self.kings[color.idx()]
    }

    #[inline]
    pub fn piece_count(&self) -> u32 {
        self.occ().count_ones()
    }

    pub fn attackers_to(&self, sq: Square, occ: u64) -> u64 {
        let mut atk = 0u64;
        atk |= attacks::pawn_attacks(Color::Black, sq) & self.pieces(Color::White, PieceType::Pawn);
        atk |= attacks::pawn_attacks(Color::White, sq) & self.pieces(Color::Black, PieceType::Pawn);
        atk |= attacks::knight_attacks(sq) & self.by_type[PieceType::Knight.idx()];
        atk |= attacks::king_attacks(sq) & self.by_type[PieceType::King.idx()];
        atk |= attacks::bishop_attacks(sq, occ)
            & (self.by_type[PieceType::Bishop.idx()] | self.by_type[PieceType::Queen.idx()]);
        atk |= attacks::rook_attacks(sq, occ)
            & (self.by_type[PieceType::Rook.idx()] | self.by_type[PieceType::Queen.idx()]);
        atk
    }

    #[inline]
    pub fn in_check(&self) -> bool {
        let ksq = self.king_sq(self.side);
        self.attackers_to(ksq, self.occ()) & self.by_color[self.side.flip().idx()] != 0
    }

    pub fn square_attacked(&self, sq: Square, by: Color) -> bool {
        self.attackers_to(sq, self.occ()) & self.by_color[by.idx()] != 0
    }

    fn compute_key(&self) -> u64 {
        let mut k = 0u64;
        for sq in 0..64u8 {
            let p = self.board[sq as usize];
            if p != NO_PIECE {
                k ^= zobrist::piece_key(p, Square(sq));
            }
        }
        k ^= zobrist::castle_key(self.castling);
        if self.ep < 64 {
            k ^= zobrist::ep_key(Square(self.ep).file());
        }
        if self.side == Color::Black {
            k ^= zobrist::side_key();
        }
        k
    }

    pub fn decode_move(&self, m: Move) -> Move {
        let from = m.from();
        let to = m.to();
        let p = self.board[from.idx()];
        if p == NO_PIECE {
            return m;
        }
        let mut flag = m.flag();
        if piece_type(p) == PieceType::King && (from.file() as i8 - to.file() as i8).abs() == 2 {
            flag = FLAG_CASTLE;
        } else if piece_type(p) == PieceType::Pawn
            && self.ep < 64
            && to.0 == self.ep
            && from.file() != to.file()
        {
            flag = FLAG_EN_PASSANT;
        }
        Move::new(from, to, flag)
    }

    pub fn make(&mut self, m: Move) {
        let m = self.decode_move(m);
        self.make_unchecked(m);
    }

    /// Makes a move whose special flag has already been decoded.
    pub(crate) fn make_unchecked(&mut self, m: Move) {
        let from = m.from();
        let to = m.to();
        let us = self.side;
        let them = us.flip();
        let piece = self.board[from.idx()];
        let pt = piece_type(piece);

        let st = State {
            key: self.key,
            ep: self.ep,
            castling: self.castling,
            halfmove: self.halfmove,
            captured: NO_PIECE,
            played: m,
            dirty_num: self.dirty_num,
            dirty: self.dirty,
        };
        self.history.push(st);
        self.bump_acc();
        self.dirty_num = 0;
        self.push_dirty(piece, from, to);

        self.key ^= zobrist::castle_key(self.castling);
        if self.ep < 64 {
            self.key ^= zobrist::ep_key(Square(self.ep).file());
        }
        self.ep = 64;
        self.halfmove += 1;
        if us == Color::Black {
            self.fullmove += 1;
        }

        let mut captured = NO_PIECE;

        if m.is_castle() {
            self.move_piece(from, to);
            self.key ^= zobrist::piece_key(piece, from) ^ zobrist::piece_key(piece, to);
            let (rook_from, rook_to) = castle_rook_squares(to);
            let rook = self.board[rook_from.idx()];
            self.move_piece(rook_from, rook_to);
            self.key ^= zobrist::piece_key(rook, rook_from) ^ zobrist::piece_key(rook, rook_to);
            self.push_dirty(rook, rook_from, rook_to);
        } else if m.is_en_passant() {
            self.move_piece(from, to);
            self.key ^= zobrist::piece_key(piece, from) ^ zobrist::piece_key(piece, to);
            let cap_sq = Square(if us == Color::White { to.0 - 8 } else { to.0 + 8 });
            captured = self.remove_piece(cap_sq);
            self.key ^= zobrist::piece_key(captured, cap_sq);
            self.push_dirty(captured, cap_sq, SQ_NONE);
            self.halfmove = 0;
        } else {
            captured = self.board[to.idx()];
            if captured != NO_PIECE {
                self.remove_piece(to);
                self.key ^= zobrist::piece_key(captured, to);
                self.push_dirty(captured, to, SQ_NONE);
                self.halfmove = 0;
            }
            self.move_piece(from, to);
            self.key ^= zobrist::piece_key(piece, from) ^ zobrist::piece_key(piece, to);
            if pt == PieceType::Pawn {
                self.halfmove = 0;
                if (from.rank() as i8 - to.rank() as i8).abs() == 2 {
                    self.ep = if us == Color::White { from.0 + 8 } else { from.0 - 8 };
                }
                if let Some(promo) = m.promo_piece() {
                    self.remove_piece(to);
                    let np = make_piece(us, promo);
                    self.put_piece(np, to);
                    self.key ^= zobrist::piece_key(piece, to) ^ zobrist::piece_key(np, to);
                    // rewrite last dirty: pawn removed from `from`, promo added on `to`
                    self.dirty_num = 0;
                    self.push_dirty(piece, from, SQ_NONE);
                    self.push_dirty(np, SQ_NONE, to);
                    if captured != NO_PIECE {
                        self.push_dirty(captured, to, SQ_NONE);
                    }
                }
            }
        }

        self.castling &= CASTLING_MASK[from.idx()] & CASTLING_MASK[to.idx()];
        self.key ^= zobrist::castle_key(self.castling);
        if self.ep < 64 {
            self.key ^= zobrist::ep_key(Square(self.ep).file());
        }
        self.key ^= zobrist::side_key();
        self.side = them;

        if let Some(st_mut) = self.history.last_mut() {
            st_mut.captured = captured;
        }
    }

    pub fn make_null(&mut self) {
        let us = self.side;
        let st = State {
            key: self.key,
            ep: self.ep,
            castling: self.castling,
            halfmove: self.halfmove,
            captured: NO_PIECE,
            played: Move::NONE,
            dirty_num: self.dirty_num,
            dirty: self.dirty,
        };
        self.history.push(st);
        self.bump_acc();
        self.dirty_num = 0;

        if self.ep < 64 {
            self.key ^= zobrist::ep_key(Square(self.ep).file());
        }
        self.ep = 64;
        self.halfmove += 1;
        if us == Color::Black {
            self.fullmove += 1;
        }
        self.key ^= zobrist::side_key();
        self.side = us.flip();
    }

    pub fn last_was_null(&self) -> bool {
        self.history.last().map(|s| s.played.is_none()).unwrap_or(false)
    }

    pub fn unmake(&mut self, _m: Move) {
        let st = self.history.pop().expect("unmake without make");
        let m = st.played;
        self.side = self.side.flip();
        let us = self.side;

        if m.is_none() {
            self.key = st.key;
            self.ep = st.ep;
            self.castling = st.castling;
            self.halfmove = st.halfmove;
            self.dirty_num = st.dirty_num;
            self.dirty = st.dirty;
            self.acc_ply -= 1;
            if us == Color::Black {
                self.fullmove -= 1;
            }
            return;
        }

        let from = m.from();
        let to = m.to();

        if m.is_castle() {
            let (rook_from, rook_to) = castle_rook_squares(to);
            self.move_piece(to, from);
            self.move_piece(rook_to, rook_from);
        } else if m.is_en_passant() {
            self.move_piece(to, from);
            let cap_sq = Square(if us == Color::White { to.0 - 8 } else { to.0 + 8 });
            self.put_piece(st.captured, cap_sq);
        } else if m.is_promotion() {
            self.remove_piece(to);
            self.put_piece(make_piece(us, PieceType::Pawn), from);
            if st.captured != NO_PIECE {
                self.put_piece(st.captured, to);
            }
        } else {
            self.move_piece(to, from);
            if st.captured != NO_PIECE {
                self.put_piece(st.captured, to);
            }
        }

        self.key = st.key;
        self.ep = st.ep;
        self.castling = st.castling;
        self.halfmove = st.halfmove;
        self.dirty_num = st.dirty_num;
        self.dirty = st.dirty;
        self.acc_ply -= 1;
        if us == Color::Black {
            self.fullmove -= 1;
        }
    }

    fn push_dirty(&mut self, piece: Piece, from: Square, to: Square) {
        let i = self.dirty_num as usize;
        self.dirty[i] = DirtyPiece { piece, from, to };
        self.dirty_num += 1;
    }

    pub fn is_repetition(&self) -> bool {
        let hm = self.halfmove as usize;
        if hm < 4 {
            return false;
        }
        let mut hits = 0;
        for st in self.history.iter().rev().take(hm) {
            if st.key == self.key {
                hits += 1;
                if hits >= 1 {
                    return true;
                }
            }
        }
        false
    }

    pub fn is_draw(&self) -> bool {
        if self.halfmove >= 100 {
            return true;
        }
        if self.is_repetition() {
            return true;
        }
        // K vs K, KN vs K, KB vs K
        let n = self.piece_count();
        if n == 2 {
            return true;
        }
        if n == 3
            && (self.by_type[PieceType::Knight.idx()] != 0
                || self.by_type[PieceType::Bishop.idx()] != 0)
        {
            return true;
        }
        false
    }

    pub fn gives_check(&self, m: Move) -> bool {
        self.make_gives_check(m)
    }

    fn make_gives_check(&self, m: Move) -> bool {
        let mut tmp = self.clone();
        tmp.make(m);
        tmp.in_check()
    }

    pub fn is_capture(&self, m: Move) -> bool {
        let m = self.decode_move(m);
        self.is_capture_decoded(m)
    }

    #[inline]
    pub(crate) fn is_capture_decoded(&self, m: Move) -> bool {
        m.is_en_passant() || self.board[m.to().idx()] != NO_PIECE
    }

    /// Tests king safety for an already decoded, pseudo-legal non-castling
    /// move. This changes only piece placement and immediately restores it;
    /// legality probes do not need Zobrist, clocks, history or NNUE state.
    pub(crate) fn king_safe_after(&mut self, m: Move) -> bool {
        debug_assert!(!m.is_castle());
        let from = m.from();
        let to = m.to();
        let us = self.side;
        let piece = self.board[from.idx()];

        let captured = if m.is_en_passant() {
            let cap_sq = Square(if us == Color::White { to.0 - 8 } else { to.0 + 8 });
            let captured = self.remove_piece(cap_sq);
            self.move_piece(from, to);
            let safe = self.attackers_to(self.king_sq(us), self.occ())
                & self.pieces_color(us.flip())
                == 0;
            self.move_piece(to, from);
            self.put_piece(captured, cap_sq);
            return safe;
        } else {
            self.remove_piece(to)
        };

        if let Some(promo) = m.promo_piece() {
            self.remove_piece(from);
            self.put_piece(make_piece(us, promo), to);
        } else {
            self.move_piece(from, to);
        }

        let safe =
            self.attackers_to(self.king_sq(us), self.occ()) & self.pieces_color(us.flip()) == 0;

        if m.is_promotion() {
            self.remove_piece(to);
            self.put_piece(piece, from);
        } else {
            self.move_piece(to, from);
        }
        if captured != NO_PIECE {
            self.put_piece(captured, to);
        }
        safe
    }

    pub fn see(&self, m: Move) -> i32 {
        crate::see::see(self, m)
    }

    #[inline]
    pub(crate) fn see_decoded(&self, m: Move) -> i32 {
        crate::see::see_decoded(self, m)
    }

    pub fn see_capture_value(&self, m: Move) -> i32 {
        let m = self.decode_move(m);
        if m.is_en_passant() {
            return material_value(PieceType::Pawn);
        }
        let cap = self.board[m.to().idx()];
        if cap == NO_PIECE {
            0
        } else {
            material_value(piece_type(cap))
        }
    }

    pub fn non_pawn_material(&self, color: Color) -> i32 {
        let mut npm = 0;
        for pt in [PieceType::Knight, PieceType::Bishop, PieceType::Rook, PieceType::Queen] {
            npm += self.pieces(color, pt).count_ones() as i32 * material_value(pt);
        }
        npm
    }
}

pub(crate) fn castle_rook_squares(king_to: Square) -> (Square, Square) {
    match king_to {
        G1 => (H1, F1),
        C1 => (A1, D1),
        G8 => (H8, F8),
        C8 => (A8, D8),
        _ => (H1, F1),
    }
}

fn char_to_piece(c: char) -> Option<(Color, PieceType)> {
    let color = if c.is_uppercase() {
        Color::White
    } else {
        Color::Black
    };
    let pt = match c.to_ascii_lowercase() {
        'p' => PieceType::Pawn,
        'n' => PieceType::Knight,
        'b' => PieceType::Bishop,
        'r' => PieceType::Rook,
        'q' => PieceType::Queen,
        'k' => PieceType::King,
        _ => return None,
    };
    Some((color, pt))
}

fn piece_to_char(p: Piece) -> char {
    let c = match piece_type(p) {
        PieceType::Pawn => 'p',
        PieceType::Knight => 'n',
        PieceType::Bishop => 'b',
        PieceType::Rook => 'r',
        PieceType::Queen => 'q',
        PieceType::King => 'k',
    };
    if piece_color(p) == Color::White {
        c.to_ascii_uppercase()
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fen_roundtrip_start() {
        crate::init();
        let p = Position::from_fen(START_FEN).unwrap();
        assert_eq!(p.side, Color::White);
        assert_eq!(p.king_sq(Color::White), E1);
        assert_eq!(p.piece_count(), 32);
    }

    #[test]
    fn acc_ply_tracks_make_unmake() {
        crate::init();
        let mut p = Position::new();
        assert_eq!(p.acc_ply, 0);
        let m = p.legal_moves().moves[0];
        p.make(m);
        assert_eq!(p.acc_ply, 1);
        assert!(!p.acc().computed[0]);
        p.unmake(m);
        assert_eq!(p.acc_ply, 0);
    }

    #[test]
    fn null_move_roundtrip() {
        crate::init();
        let mut p = Position::new();
        let fen = p.to_fen();
        let key = p.key;
        p.make_null();
        assert_eq!(p.side, Color::Black);
        assert!(p.last_was_null());
        p.unmake(Move::NONE);
        assert_eq!(p.to_fen(), fen);
        assert_eq!(p.key, key);
        assert_eq!(p.side, Color::White);
    }
}
