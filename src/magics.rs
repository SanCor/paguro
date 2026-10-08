//! Fancy magic bitboards for bishop and rook attacks.

use crate::bitboard::try_offset;
use crate::types::Square;
use std::sync::OnceLock;

#[derive(Clone, Copy)]
struct Magic {
    mask: u64,
    magic: u64,
    shift: u32,
    offset: usize,
}

pub struct MagicTables {
    bishop: [Magic; 64],
    rook: [Magic; 64],
    bishop_attacks: Vec<u64>,
    rook_attacks: Vec<u64>,
}

static TABLES: OnceLock<MagicTables> = OnceLock::new();

pub fn init() {
    let _ = tables();
}

fn tables() -> &'static MagicTables {
    TABLES.get_or_init(MagicTables::new)
}

impl MagicTables {
    fn new() -> MagicTables {
        let mut rng = 0xDEAD_BEEF_CAFE_BABEu64;
        let mut bishop = [Magic {
            mask: 0,
            magic: 0,
            shift: 0,
            offset: 0,
        }; 64];
        let mut rook = bishop;
        let mut bishop_attacks = Vec::new();
        let mut rook_attacks = Vec::new();

        for sq in 0..64u8 {
            fill_square(
                sq,
                true,
                &mut rng,
                &mut bishop[sq as usize],
                &mut bishop_attacks,
            );
            fill_square(
                sq,
                false,
                &mut rng,
                &mut rook[sq as usize],
                &mut rook_attacks,
            );
        }

        MagicTables {
            bishop,
            rook,
            bishop_attacks,
            rook_attacks,
        }
    }
}

fn fill_square(sq: u8, bishop: bool, rng: &mut u64, magic: &mut Magic, table: &mut Vec<u64>) {
    let mask = relevant_mask(sq, bishop);
    let bits = mask.count_ones();
    let shift = 64 - bits;
    let n = 1usize << bits;

    let mut occupancies = vec![0u64; n];
    let mut attacks = vec![0u64; n];
    let mut occ = 0u64;
    for i in 0..n {
        occupancies[i] = occ;
        attacks[i] = sliding_attacks(sq, occ, bishop);
        occ = occ.wrapping_sub(mask) & mask;
    }

    let found = loop {
        let mag = sparse_rand(rng);
        if (mask.wrapping_mul(mag) >> 56).count_ones() < 6 {
            continue;
        }
        let mut used = vec![0u64; n];
        let mut seen = vec![false; n];
        let mut ok = true;
        for i in 0..n {
            let idx = (occupancies[i].wrapping_mul(mag) >> shift) as usize;
            if !seen[idx] {
                used[idx] = attacks[i];
                seen[idx] = true;
            } else if used[idx] != attacks[i] {
                ok = false;
                break;
            }
        }
        if ok {
            break (mag, used);
        }
    };

    let offset = table.len();
    table.extend_from_slice(&found.1);
    *magic = Magic {
        mask,
        magic: found.0,
        shift,
        offset,
    };
}

fn sparse_rand(rng: &mut u64) -> u64 {
    rand_u64(rng) & rand_u64(rng) & rand_u64(rng)
}

fn rand_u64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

fn relevant_mask(sq: u8, bishop: bool) -> u64 {
    let mut mask = 0u64;
    let dirs: &[(i8, i8)] = if bishop {
        &[(1, 1), (1, -1), (-1, 1), (-1, -1)]
    } else {
        &[(1, 0), (-1, 0), (0, 1), (0, -1)]
    };
    for &(df, dr) in dirs {
        let mut f = (sq & 7) as i8 + df;
        let mut r = (sq >> 3) as i8 + dr;
        while (0..8).contains(&f) && (0..8).contains(&r) {
            let nf = f + df;
            let nr = r + dr;
            if !(0..8).contains(&nf) || !(0..8).contains(&nr) {
                break;
            }
            mask |= 1u64 << (f + r * 8);
            f = nf;
            r = nr;
        }
    }
    mask
}

pub fn sliding_attacks(sq: u8, occ: u64, bishop: bool) -> u64 {
    let mut attacks = 0u64;
    let dirs: &[(i8, i8)] = if bishop {
        &[(1, 1), (1, -1), (-1, 1), (-1, -1)]
    } else {
        &[(1, 0), (-1, 0), (0, 1), (0, -1)]
    };
    let origin = Square(sq);
    for &(df, dr) in dirs {
        let mut cur = origin;
        while let Some(nxt) = try_offset(cur, df, dr) {
            attacks |= nxt.bb();
            if occ & nxt.bb() != 0 {
                break;
            }
            cur = nxt;
        }
    }
    attacks
}

#[inline]
pub fn bishop_attacks(sq: Square, occ: u64) -> u64 {
    let t = tables();
    let m = t.bishop[sq.idx()];
    let idx = ((occ & m.mask).wrapping_mul(m.magic) >> m.shift) as usize;
    t.bishop_attacks[m.offset + idx]
}

#[inline]
pub fn rook_attacks(sq: Square, occ: u64) -> u64 {
    let t = tables();
    let m = t.rook[sq.idx()];
    let idx = ((occ & m.mask).wrapping_mul(m.magic) >> m.shift) as usize;
    t.rook_attacks[m.offset + idx]
}

#[inline]
pub fn queen_attacks(sq: Square, occ: u64) -> u64 {
    bishop_attacks(sq, occ) | rook_attacks(sq, occ)
}
