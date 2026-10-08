//! Polyglot opening book: hash, load, weighted probe.

use crate::attacks;
use crate::board::Position;
use crate::polyglot_keys::RANDOM64;
use crate::types::*;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const CASTLE_WHITE_OO: usize = 768;
const CASTLE_WHITE_OOO: usize = 769;
const CASTLE_BLACK_OO: usize = 770;
const CASTLE_BLACK_OOO: usize = 771;
const EP_FILE: usize = 772;
const TURN_WHITE: usize = 780;

#[derive(Clone, Copy, Debug)]
struct Entry {
    key: u64,
    move_raw: u16,
    weight: u16,
}

#[derive(Clone, Debug)]
pub struct Book {
    entries: Vec<Entry>,
    rng: u64,
}

impl Default for Book {
    fn default() -> Self {
        Book {
            entries: Vec::new(),
            rng: seed_from_time(),
        }
    }
}

impl Book {
    pub fn empty() -> Book {
        Book::default()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn load(path: &Path) -> Result<Book, String> {
        let data = fs::read(path).map_err(|e| e.to_string())?;
        if data.len() % 16 != 0 {
            return Err(format!(
                "invalid polyglot book size {} (not a multiple of 16)",
                data.len()
            ));
        }
        let mut entries = Vec::with_capacity(data.len() / 16);
        for chunk in data.chunks_exact(16) {
            let key = u64::from_be_bytes(chunk[0..8].try_into().unwrap());
            let move_raw = u16::from_be_bytes(chunk[8..10].try_into().unwrap());
            let weight = u16::from_be_bytes(chunk[10..12].try_into().unwrap());
            if weight == 0 {
                continue;
            }
            entries.push(Entry {
                key,
                move_raw,
                weight,
            });
        }
        entries.sort_by_key(|e| e.key);
        Ok(Book {
            entries,
            rng: seed_from_time(),
        })
    }

    /// Weighted random legal book move, or `None` to fall back to search.
    pub fn probe(&mut self, pos: &mut Position) -> Option<Move> {
        if self.entries.is_empty() {
            return None;
        }
        let key = hash(pos);
        let slice = range_for(&self.entries, key);
        if slice.is_empty() {
            return None;
        }

        let mut legal: Vec<(Move, u32)> = Vec::new();
        let mut total = 0u32;
        for e in slice {
            let m = polyglot_to_move(pos, e.move_raw);
            if pos.is_legal(m) {
                legal.push((m, e.weight as u32));
                total += e.weight as u32;
            }
        }
        if legal.is_empty() || total == 0 {
            return None;
        }
        let mut r = (self.next_u64() % total as u64) as u32;
        for (m, w) in legal {
            if r < w {
                return Some(m);
            }
            r -= w;
        }
        None
    }

    fn next_u64(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

fn seed_from_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E3779B97F4A7C15)
        | 1
}

fn range_for(entries: &[Entry], key: u64) -> &[Entry] {
    let i = entries.partition_point(|e| e.key < key);
    let mut j = i;
    while j < entries.len() && entries[j].key == key {
        j += 1;
    }
    &entries[i..j]
}

/// Polyglot Zobrist hash (independent of the engine's search Zobrist).
pub fn hash(pos: &Position) -> u64 {
    let mut k = 0u64;
    for sq in 0..64u8 {
        let p = pos.piece_on(Square(sq));
        if p == NO_PIECE {
            continue;
        }
        let kind = piece_type(p).idx() * 2 + if piece_color(p) == Color::White { 1 } else { 0 };
        let s = Square(sq);
        let idx = 64 * kind + 8 * s.rank() as usize + s.file() as usize;
        k ^= RANDOM64[idx];
    }
    if pos.castling & WHITE_OO != 0 {
        k ^= RANDOM64[CASTLE_WHITE_OO];
    }
    if pos.castling & WHITE_OOO != 0 {
        k ^= RANDOM64[CASTLE_WHITE_OOO];
    }
    if pos.castling & BLACK_OO != 0 {
        k ^= RANDOM64[CASTLE_BLACK_OO];
    }
    if pos.castling & BLACK_OOO != 0 {
        k ^= RANDOM64[CASTLE_BLACK_OOO];
    }
    if pos.ep < 64 {
        let ep = Square(pos.ep);
        let capturers =
            attacks::pawn_attacks(pos.side.flip(), ep) & pos.pieces(pos.side, PieceType::Pawn);
        if capturers != 0 {
            k ^= RANDOM64[EP_FILE + ep.file() as usize];
        }
    }
    if pos.side == Color::White {
        k ^= RANDOM64[TURN_WHITE];
    }
    k
}

fn polyglot_to_move(pos: &Position, raw: u16) -> Move {
    let to_file = (raw & 7) as u8;
    let to_rank = ((raw >> 3) & 7) as u8;
    let from_file = ((raw >> 6) & 7) as u8;
    let from_rank = ((raw >> 9) & 7) as u8;
    let promo = ((raw >> 12) & 7) as u8;
    let from = Square::new(from_file, from_rank);
    let mut to = Square::new(to_file, to_rank);
    let mut flag = match promo {
        1 => FLAG_PROMO_N,
        2 => FLAG_PROMO_B,
        3 => FLAG_PROMO_R,
        4 => FLAG_PROMO_Q,
        _ => FLAG_NORMAL,
    };
    let piece = pos.piece_on(from);
    if piece != NO_PIECE && piece_type(piece) == PieceType::King {
        if from == E1 && (to == H1 || to == G1) {
            to = G1;
            flag = FLAG_CASTLE;
        } else if from == E1 && (to == A1 || to == C1) {
            to = C1;
            flag = FLAG_CASTLE;
        } else if from == E8 && (to == H8 || to == G8) {
            to = G8;
            flag = FLAG_CASTLE;
        } else if from == E8 && (to == A8 || to == C8) {
            to = C8;
            flag = FLAG_CASTLE;
        }
    }
    pos.decode_move(Move::new(from, to, flag))
}

/// Encode a non-castle, non-promo move in Polyglot's bitfield (for tests).
#[cfg(test)]
fn encode_polyglot(from: Square, to: Square, promo: u8) -> u16 {
    (to.file() as u16)
        | ((to.rank() as u16) << 3)
        | ((from.file() as u16) << 6)
        | ((from.rank() as u16) << 9)
        | ((promo as u16) << 12)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Position, START_FEN};
    use std::io::Write;

    fn key(fen: &str) -> u64 {
        crate::init();
        hash(&Position::from_fen(fen).unwrap())
    }

    #[test]
    fn polyglot_spec_keys() {
        crate::init();
        assert_eq!(
            hash(&Position::from_fen(START_FEN).unwrap()),
            0x463B96181691FC9C
        );
        // After e2e4 the EP square is e3, but Black cannot capture it.
        assert_eq!(
            key("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"),
            0x823C9B50FD114196
        );
        assert_eq!(
            key("rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2"),
            0x0756B94461C50FB0
        );
        assert_eq!(
            key("rnbqkbnr/ppp1pppp/8/3pP3/8/8/PPPP1PPP/RNBQKBNR b KQkq - 0 2"),
            0x662FAFB965DB29D4
        );
        assert_eq!(
            key("rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPP1PPP/RNBQKBNR w KQkq f6 0 3"),
            0x22A48B5A8E47FF78
        );
        assert_eq!(
            key("rnbqkbnr/ppp1p1pp/8/3pPp2/8/8/PPPPKPPP/RNBQ1BNR b kq - 0 3"),
            0x652A607CA3F242C1
        );
        assert_eq!(
            key("rnbq1bnr/ppp1pkpp/8/3pPp2/8/8/PPPPKPPP/RNBQ1BNR w - - 0 4"),
            0x00FDD303C946BDD9
        );
        // EP c3 is capturable by the black pawn on b4.
        assert_eq!(
            key("rnbqkbnr/p1pppppp/8/8/PpP4P/8/1P1PPPP1/RNBQKBNR b KQkq c3 0 3"),
            0x3C8123EA7B067637
        );
        assert_eq!(
            key("rnbqkbnr/p1pppppp/8/8/P6P/R1p5/1P1PPPP1/1NBQKBNR b Kkq - 0 4"),
            0x5C3F9B829B279560
        );
    }

    #[test]
    fn probe_startpos_e2e4() {
        crate::init();
        let mut pos = Position::new();
        let key = hash(&pos);
        let raw = encode_polyglot(E2, E4, 0);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&key.to_be_bytes());
        bytes.extend_from_slice(&raw.to_be_bytes());
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());

        let dir = std::env::temp_dir();
        let path = dir.join("paguro_test_book.bin");
        {
            let mut f = fs::File::create(&path).unwrap();
            f.write_all(&bytes).unwrap();
        }
        let mut book = Book::load(&path).unwrap();
        let m = book.probe(&mut pos).expect("book hit");
        assert_eq!(m.to_uci(), "e2e4");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn castle_king_captures_rook() {
        crate::init();
        let mut pos =
            Position::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
        let m = polyglot_to_move(&pos, encode_polyglot(E1, H1, 0));
        assert!(m.is_castle());
        assert_eq!(m.to_uci(), "e1g1");
        assert!(pos.is_legal(m));
        let m = polyglot_to_move(&pos, encode_polyglot(E1, A1, 0));
        assert_eq!(m.to_uci(), "e1c1");
        assert!(pos.is_legal(m));
    }
}
