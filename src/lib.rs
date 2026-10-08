//! Paguro, a UCI chess engine.
//! Licensed under GPL-3.0-or-later.

pub mod attacks;
pub mod bitboard;
pub mod board;
pub mod book;
pub mod eval;
pub mod magics;
pub mod movegen;
pub mod movepick;
pub mod nnue;
pub mod polyglot_keys;
pub mod prefs;
pub mod search;
pub mod see;
pub mod time;
pub mod tt;
pub mod types;
pub mod uci;
pub mod zobrist;

pub fn init() {
    attacks::init();
    zobrist::init();
    search::init();
}
