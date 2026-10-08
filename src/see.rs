//! Static Exchange Evaluation.

use crate::board::Position;
use crate::types::*;

pub fn see_value(pt: PieceType) -> i32 {
    match pt {
        PieceType::Pawn => 100,
        PieceType::Knight => 320,
        PieceType::Bishop => 330,
        PieceType::Rook => 500,
        PieceType::Queen => 900,
        PieceType::King => 20_000,
    }
}

/// Approximate material change of capturing on `to`, assuming best recaptures.
pub fn see(pos: &Position, m: Move) -> i32 {
    let m = pos.decode_move(m);
    see_decoded(pos, m)
}

pub(crate) fn see_decoded(pos: &Position, m: Move) -> i32 {
    if m.is_castle() || m.is_none() {
        return 0;
    }

    let from = m.from();
    let to = m.to();
    let us = pos.side;
    let mut occ = pos.occ();
    let mut gain = [0i32; 32];

    let captured_val = if m.is_en_passant() {
        let cap_sq = Square(if us == Color::White { to.0 - 8 } else { to.0 + 8 });
        occ ^= cap_sq.bb();
        see_value(PieceType::Pawn)
    } else {
        let cap = pos.piece_on(to);
        if cap == NO_PIECE {
            0
        } else {
            see_value(piece_type(cap))
        }
    };

    if pos.piece_on(to) == NO_PIECE {
        occ |= to.bb();
    }

    let mut attacker_pt = piece_type(pos.piece_on(from));
    if let Some(promo) = m.promo_piece() {
        gain[0] = captured_val + see_value(promo) - see_value(PieceType::Pawn);
        attacker_pt = promo;
    } else {
        gain[0] = captured_val;
    }

    occ ^= from.bb();
    let mut stm = us.flip();
    let mut attackers = pos.attackers_to(to, occ) & occ;
    let mut d = 0;

    while attackers & pos.pieces_color(stm) != 0 {
        d += 1;
        if d >= 31 {
            break;
        }
        gain[d] = see_value(attacker_pt) - gain[d - 1];

        let Some((nfrom, npt)) = lva(pos, attackers, stm, occ) else {
            d -= 1;
            break;
        };
        occ ^= nfrom.bb();
        attackers |= xray(pos, to, occ);
        attackers &= occ;
        attacker_pt = npt;
        stm = stm.flip();
    }

    while d > 0 {
        gain[d - 1] = gain[d - 1].min(-gain[d]);
        d -= 1;
    }
    gain[0]
}

fn lva(pos: &Position, attackers: u64, stm: Color, occ: u64) -> Option<(Square, PieceType)> {
    for pt in PieceType::ALL {
        let bb = attackers & pos.pieces(stm, pt) & occ;
        if bb != 0 {
            return Some((Square(bb.trailing_zeros() as u8), pt));
        }
    }
    None
}

fn xray(pos: &Position, to: Square, occ: u64) -> u64 {
    let bishops = pos.by_type[PieceType::Bishop.idx()] | pos.by_type[PieceType::Queen.idx()];
    let rooks = pos.by_type[PieceType::Rook.idx()] | pos.by_type[PieceType::Queen.idx()];
    (crate::attacks::bishop_attacks(to, occ) & bishops)
        | (crate::attacks::rook_attacks(to, occ) & rooks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Position;

    #[test]
    fn see_equal_pawn_trade() {
        crate::init();
        let pos =
            Position::from_fen("rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2")
                .unwrap();
        let m = Move::from_uci("e4d5").unwrap();
        assert_eq!(see(&pos, m), 0);
    }

    #[test]
    fn see_hanging_pawn() {
        crate::init();
        let pos = Position::from_fen("4k3/8/8/3p4/8/8/8/3QK3 w - - 0 1").unwrap();
        let m = Move::from_uci("d1d5").unwrap();
        assert_eq!(see(&pos, m), 100);
    }

    #[test]
    fn see_queen_takes_protected_pawn() {
        crate::init();
        let pos = Position::from_fen("4k3/8/4p3/3p4/8/8/8/3QK3 w - - 0 1").unwrap();
        let m = Move::from_uci("d1d5").unwrap();
        assert_eq!(see(&pos, m), 100 - 900);
    }
}
