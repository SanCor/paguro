//! Alpha-beta search with iterative deepening and quiescence.

use crate::board::Position;
use crate::eval;
use crate::movepick;
use crate::nnue::network::Network;
use crate::time::TimeManager;
use crate::tt::{self, TranspositionTable, BOUND_EXACT, BOUND_LOWER, BOUND_UPPER};
use crate::types::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

const HIST_MAX: i32 = 16_384;
const LMR_MAX: usize = 64;
const ASPIRATION: i32 = 25;

static LMR_TABLE: OnceLock<[[i32; LMR_MAX]; LMR_MAX]> = OnceLock::new();

pub fn init() {
    let _ = lmr_table();
}

fn lmr_table() -> &'static [[i32; LMR_MAX]; LMR_MAX] {
    LMR_TABLE.get_or_init(|| {
        let mut t = [[0i32; LMR_MAX]; LMR_MAX];
        for depth in 1..LMR_MAX {
            for moves in 1..LMR_MAX {
                let r = (depth as f64).ln() * (moves as f64).ln() / 2.0;
                t[depth][moves] = r.floor() as i32;
            }
        }
        t
    })
}

pub struct SearchResult {
    pub best: Move,
    pub score: i32,
    pub depth: u32,
    pub nodes: u64,
}

pub struct SearchContext<'a> {
    pub pos: &'a mut Position,
    pub tt: &'a mut TranspositionTable,
    pub net: Option<&'a Network>,
    pub stop: &'a AtomicBool,
    pub time: &'a TimeManager,
    pub nodes: u64,
    pub killers: [[Move; 2]; MAX_PLY],
    pub history: [[[i16; 64]; 64]; 2],
    pub evals: [i32; MAX_PLY],
    pub pv: Vec<[Move; MAX_PLY]>,
    pub pv_len: [usize; MAX_PLY],
}

pub fn go(
    pos: &mut Position,
    tt: &mut TranspositionTable,
    net: Option<&Network>,
    stop: &AtomicBool,
    time: &TimeManager,
) -> SearchResult {
    tt.new_search();
    let mut ctx = SearchContext {
        pos,
        tt,
        net,
        stop,
        time,
        nodes: 0,
        killers: [[Move::NONE; 2]; MAX_PLY],
        history: [[[0; 64]; 64]; 2],
        evals: [0; MAX_PLY],
        pv: vec![[Move::NONE; MAX_PLY]; MAX_PLY],
        pv_len: [0; MAX_PLY],
    };

    let root_moves = ctx.pos.legal_moves();
    if root_moves.len == 0 {
        return SearchResult {
            best: Move::NONE,
            score: 0,
            depth: 0,
            nodes: 0,
        };
    }
    let mut best = root_moves.moves[0];
    let mut best_score = 0;
    let mut last = 0i32;
    let mut completed_depth = 0u32;
    let mut completed_pv = [Move::NONE; MAX_PLY];
    let mut completed_pv_len = 0usize;

    for depth in 1..=time.max_depth {
        if stop.load(Ordering::Relaxed) || (depth > 1 && time.expired()) {
            break;
        }
        if depth > 1 && time.should_stop_id() {
            break;
        }

        let mut delta = ASPIRATION;
        let mut alpha = -INF;
        let mut beta = INF;
        if depth >= 5 && last.abs() < MATE_IN_MAX {
            alpha = (last - delta).max(-INF);
            beta = (last + delta).min(INF);
        }
        let mut aspiration_failures = 0u8;
        let mut interrupted = false;
        let mut iteration_candidate = Move::NONE;

        let score = loop {
            ctx.pv_len[0] = 0;
            let s = alpha_beta(&mut ctx, alpha, beta, depth as i32, 0, true);

            if stop.load(Ordering::Relaxed) && depth > 1 {
                interrupted = true;
                break s;
            }

            if s <= alpha && alpha > -INF {
                print_info(&ctx, depth, s, " upperbound");
                aspiration_failures += 1;
                beta = ((alpha + beta) / 2).min(beta);
                alpha = (s - delta).max(-INF);
                delta += delta / 2 + 5;
                if (ctx.pos.piece_count() <= 7 && aspiration_failures >= 3) || delta >= 700 {
                    alpha = -INF;
                    beta = INF;
                }
                continue;
            }
            if s >= beta && beta < INF {
                if ctx.pv_len[0] > 0 {
                    iteration_candidate = ctx.pv[0][0];
                }
                print_info(&ctx, depth, s, " lowerbound");
                aspiration_failures += 1;
                alpha = ((alpha + beta) / 2).max(alpha);
                beta = (s + delta).min(INF);
                delta += delta / 2 + 5;
                if (ctx.pos.piece_count() <= 7 && aspiration_failures >= 3) || delta >= 700 {
                    alpha = -INF;
                    beta = INF;
                }
                continue;
            }
            break s;
        };

        if stop.load(Ordering::Relaxed) && depth > 1 {
            if interrupted && completed_depth > 0 {
                ctx.pv[0] = completed_pv;
                ctx.pv_len[0] = completed_pv_len;
                print_info(&ctx, completed_depth, best_score, "");
            }
            if !iteration_candidate.is_none() {
                best = iteration_candidate;
            }
            break;
        }

        if ctx.pv_len[0] > 0 {
            best = ctx.pv[0][0];
        }
        best_score = score;
        last = score;
        completed_depth = depth;
        completed_pv = ctx.pv[0];
        completed_pv_len = ctx.pv_len[0];
        print_info(&ctx, depth, score, "");

        if time.should_stop_id() {
            break;
        }
    }

    SearchResult {
        best,
        score: best_score,
        depth: completed_depth,
        nodes: ctx.nodes,
    }
}

fn print_info(ctx: &SearchContext, depth: u32, score: i32, bound: &str) {
    let elapsed = ctx.time.elapsed_ms().max(1);
    let nps = ctx.nodes * 1000 / elapsed;
    let score_str = format_score(score);
    let pv_str = format_pv(ctx, 0);
    println!(
        "info depth {depth} score {score_str}{bound} nodes {} nps {nps} time {elapsed} pv {pv_str}",
        ctx.nodes
    );
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

fn format_score(score: i32) -> String {
    if score >= MATE_IN_MAX {
        format!("mate {}", (MATE - score + 1) / 2)
    } else if score <= -MATE_IN_MAX {
        format!("mate {}", -(MATE + score + 1) / 2)
    } else {
        format!("cp {score}")
    }
}

fn format_pv(ctx: &SearchContext, ply: usize) -> String {
    let mut s = String::new();
    for i in 0..ctx.pv_len[ply] {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&ctx.pv[ply][i].to_uci());
    }
    s
}

fn alpha_beta(
    ctx: &mut SearchContext,
    mut alpha: i32,
    beta: i32,
    depth: i32,
    ply: i32,
    is_pv: bool,
) -> i32 {
    ctx.pv_len[ply as usize] = 0;

    if ctx.stop.load(Ordering::Relaxed) || (ctx.nodes & 2047 == 0 && ctx.time.expired()) {
        ctx.stop.store(true, Ordering::Relaxed);
        return 0;
    }

    if ply > 0 && ctx.pos.is_draw() {
        return 0;
    }
    if ply as usize >= MAX_PLY - 1 {
        return eval::evaluate(ctx.pos, ctx.net);
    }

    let in_check = ctx.pos.in_check();
    let mut depth = depth;
    // Keep full check extensions on the principal variation and near the
    // horizon; non-PV check chains otherwise consume depth without limit.
    if in_check && (is_pv || depth <= 4) {
        depth += 1;
    }

    if depth <= 0 {
        return quiescence(ctx, alpha, beta, ply);
    }

    ctx.nodes += 1;

    let key = ctx.pos.key;
    let mut tt_move = Move::NONE;
    if let Some(e) = ctx.tt.probe(key) {
        tt_move = e.m;
        let tt_score = tt::from_tt(e.score as i32, ply);
        if !is_pv && e.depth as i32 >= depth {
            match e.bound {
                BOUND_EXACT => return tt_score,
                BOUND_LOWER if tt_score >= beta => return tt_score,
                BOUND_UPPER if tt_score <= alpha => return tt_score,
                _ => {}
            }
        }
    }

    // Internal iterative reduction: without a TT move, deep nodes have weak
    // ordering information. Search one ply shallower first, conservatively.
    if ply > 0 && tt_move.is_none() && depth >= 6 && (!is_pv || depth >= 8) {
        depth -= 1;
    }

    let static_eval = if in_check {
        -INF
    } else {
        eval::evaluate(ctx.pos, ctx.net)
    };
    ctx.evals[ply as usize] = static_eval;
    let improving = ply >= 2 && !in_check && static_eval > ctx.evals[ply as usize - 2];

    if !is_pv
        && !in_check
        && ply > 0
        && depth <= 8
        && static_eval.abs() < MATE_IN_MAX
        && beta.abs() < MATE_IN_MAX
    {
        let margin = if improving { 64 } else { 96 };
        if static_eval - margin * depth >= beta {
            return static_eval;
        }
    }

    if !is_pv
        && !in_check
        && ply > 0
        && depth >= 3
        && static_eval >= beta
        && beta.abs() < MATE_IN_MAX
        && null_move_allowed(ctx.pos)
        && !ctx.pos.last_was_null()
    {
        let r = 3 + depth / 3;
        ctx.pos.make_null();
        let score = -alpha_beta(ctx, -beta, -beta + 1, depth - 1 - r, ply + 1, false);
        ctx.pos.unmake(Move::NONE);
        if ctx.stop.load(Ordering::Relaxed) {
            return 0;
        }
        if score >= beta {
            let score = if score >= MATE_IN_MAX { beta } else { score };
            ctx.tt.store(key, depth, score, BOUND_LOWER, Move::NONE, ply);
            return score;
        }
    }

    // Probcut (reduced capture search above a raised beta) was tried and
    // dropped Bxf7, so it stays out.

    // Root moves use the same picker as every other node: TT move, then
    // captures by SEE, killers and history. Ordering the root by the node
    // count of the previous iteration was tried and did not improve results.
    let mut picker = movepick::MovePicker::main(
        tt_move,
        ctx.killers[ply as usize],
        in_check,
    );

    let mut best = -INF;
    let mut best_move = Move::NONE;
    let mut bound = BOUND_UPPER;
    let mut searched_quiets: [Move; 64] = [Move::NONE; 64];
    let mut n_quiets = 0usize;
    let mut i = 0usize;
    let us = ctx.pos.side;

    while let Some(m) = picker.next(ctx.pos, &ctx.history[us.idx()]) {
        let is_quiet = !ctx.pos.is_capture_decoded(m) && !m.is_promotion();
        let is_killer = m == ctx.killers[ply as usize][0] || m == ctx.killers[ply as usize][1];
        let hist = ctx.history[us.idx()][m.from().idx()][m.to().idx()] as i32;

        // Very bad quiets, late and shallow, are not searched. Checks stay.
        if !is_pv
            && !in_check
            && ply > 0
            && is_quiet
            && !is_killer
            && m != tt_move
            && i >= 4
            && depth <= 4
            && hist < -4096
            && !gives_check_on(ctx.pos, m)
        {
            i += 1;
            continue;
        }

        // Late quiets are not searched up to depth 7. Captures, checks and
        // killers stay; the picker still returns bad captures afterwards.
        if !is_pv
            && !in_check
            && ply > 0
            && is_quiet
            && !is_killer
            && depth <= 7
            && !gives_check_on(ctx.pos, m)
        {
            let base = 4 + depth * depth;
            let limit = if improving { base } else { base * 2 / 3 };
            if i as i32 >= limit {
                i += 1;
                continue;
            }
        }

        ctx.pos.make_unchecked(m);
        let gives_check = ctx.pos.in_check();

        if !is_pv
            && !in_check
            && ply > 0
            && depth <= 2
            && i > 0
            && is_quiet
            && !gives_check
            && static_eval.abs() < MATE_IN_MAX
            && alpha.abs() < MATE_IN_MAX
            && static_eval + 150 * depth <= alpha
        {
            ctx.pos.unmake(m);
            i += 1;
            continue;
        }

        let new_depth = depth - 1;
        let mut reduction = 0;
        let min_index = if is_pv { 3 } else { 2 };
        if i >= min_index
            && ply > 0
            && new_depth >= 2
            && depth >= 3
            && !in_check
            && !gives_check
            && m != tt_move
        {
            if is_quiet && !is_killer {
                reduction = lmr_reduction(depth, i, is_pv, improving, hist);
            } else if !is_quiet && !m.is_promotion() {
                // Captures lose one ply less than a quiet in the same slot.
                reduction = lmr_reduction(depth, i, is_pv, improving, hist).saturating_sub(1);
            }
            if reduction > 0 && new_depth - reduction < 1 {
                reduction = new_depth - 1;
            }
        }

        let score = if i == 0 {
            -alpha_beta(ctx, -beta, -alpha, new_depth, ply + 1, is_pv)
        } else {
            let mut score =
                -alpha_beta(ctx, -alpha - 1, -alpha, new_depth - reduction, ply + 1, false);
            if score > alpha && reduction > 0 {
                score = -alpha_beta(ctx, -alpha - 1, -alpha, new_depth, ply + 1, false);
            }
            if score > alpha && score < beta {
                score = -alpha_beta(ctx, -beta, -alpha, new_depth, ply + 1, is_pv);
            }
            score
        };
        ctx.pos.unmake(m);
        i += 1;

        if ctx.stop.load(Ordering::Relaxed) {
            return 0;
        }

        if score > best {
            best = score;
            best_move = m;
            update_pv(ctx, ply as usize, m);
            if score > alpha {
                alpha = score;
                bound = BOUND_EXACT;
                if score >= beta {
                    bound = BOUND_LOWER;
                    if is_quiet {
                        store_killer(&mut ctx.killers[ply as usize], m);
                        let bonus = (depth * depth).min(400);
                        add_history(&mut ctx.history[us.idx()], m, bonus);
                        for k in 0..n_quiets {
                            add_history(&mut ctx.history[us.idx()], searched_quiets[k], -bonus);
                        }
                    }
                    break;
                }
            }
        }
        if is_quiet && n_quiets < searched_quiets.len() {
            searched_quiets[n_quiets] = m;
            n_quiets += 1;
        }
    }

    if i == 0 {
        return if in_check { tt::mated_in(ply) } else { 0 };
    }

    ctx.tt.store(key, depth, best, bound, best_move, ply);
    best
}

fn quiescence(ctx: &mut SearchContext, mut alpha: i32, beta: i32, ply: i32) -> i32 {
    if ctx.stop.load(Ordering::Relaxed) || (ctx.nodes & 2047 == 0 && ctx.time.expired()) {
        ctx.stop.store(true, Ordering::Relaxed);
        return 0;
    }
    if ply as usize >= MAX_PLY - 1 {
        return eval::evaluate(ctx.pos, ctx.net);
    }
    if ply > 0 && ctx.pos.is_draw() {
        return 0;
    }

    ctx.nodes += 1;
    let in_check = ctx.pos.in_check();

    let mut best = if in_check {
        -INF
    } else {
        eval::evaluate(ctx.pos, ctx.net)
    };
    if !in_check {
        if best >= beta {
            return best;
        }
        if best > alpha {
            alpha = best;
        }
    }

    let mut picker = movepick::MovePicker::qsearch(in_check);
    let us = ctx.pos.side;
    let mut move_count = 0u32;

    while let Some(m) = picker.next(ctx.pos, &ctx.history[us.idx()]) {
        move_count += 1;
        if !in_check {
            let see = picker.last_see;
            if see < 0 {
                continue;
            }
            if best + see + 200 < alpha {
                continue;
            }
        }
        ctx.pos.make_unchecked(m);
        let score = -quiescence(ctx, -beta, -alpha, ply + 1);
        ctx.pos.unmake(m);

        if ctx.stop.load(Ordering::Relaxed) {
            return 0;
        }
        if score > best {
            best = score;
            if score > alpha {
                alpha = score;
                if score >= beta {
                    return best;
                }
            }
        }
    }

    if in_check && move_count == 0 {
        return tt::mated_in(ply);
    }
    best
}

fn update_pv(ctx: &mut SearchContext, ply: usize, m: Move) {
    ctx.pv[ply][0] = m;
    let next = ply + 1;
    let n = ctx.pv_len[next.min(MAX_PLY - 1)];
    for i in 0..n {
        ctx.pv[ply][i + 1] = ctx.pv[next][i];
    }
    ctx.pv_len[ply] = 1 + n;
}

fn gives_check_on(pos: &Position, m: Move) -> bool {
    if m.is_castle() {
        return true;
    }
    let from = m.from();
    let to = m.to();
    let piece = pos.piece_on(from);
    if piece == NO_PIECE {
        return false;
    }
    let us = pos.side;
    let king = pos.king_sq(us.flip());
    let occ = (pos.occ() ^ from.bb()) | to.bb();
    let direct = match piece_type(piece) {
        PieceType::Pawn => crate::attacks::pawn_attacks(us, to),
        PieceType::Knight => crate::attacks::knight_attacks(to),
        PieceType::Bishop => crate::attacks::bishop_attacks(to, occ),
        PieceType::Rook => crate::attacks::rook_attacks(to, occ),
        PieceType::Queen => crate::attacks::queen_attacks(to, occ),
        PieceType::King => crate::attacks::king_attacks(to),
    };
    if direct & king.bb() != 0 {
        return true;
    }
    let discovered = pos.attackers_to(king, occ) & pos.pieces_color(us) & !from.bb();
    discovered != 0
}

fn store_killer(slot: &mut [Move; 2], m: Move) {
    if slot[0] != m {
        slot[1] = slot[0];
        slot[0] = m;
    }
}

#[inline]
fn null_move_allowed(pos: &Position) -> bool {
    if pos.non_pawn_material(pos.side) == 0 {
        return false;
    }
    if pos.piece_count() > 7 {
        return true;
    }
    // In sparse minor-piece endings, zugzwang makes null move unreliable.
    // Rook and queen endings still benefit substantially from it.
    pos.pieces(pos.side, PieceType::Rook) != 0
        || pos.pieces(pos.side, PieceType::Queen) != 0
}

fn lmr_reduction(depth: i32, move_index: usize, is_pv: bool, improving: bool, hist: i32) -> i32 {
    let d = (depth as usize).min(LMR_MAX - 1);
    let m = move_index.min(LMR_MAX - 1);
    let mut r = lmr_table()[d][m];
    if is_pv {
        r -= 1;
    }
    if improving {
        r -= 1;
    }
    // Bad-history late moves search up to four plies shallower; trusted moves
    // still recover at most three plies and all reduced fail-highs are verified.
    r -= (hist / 1536).clamp(-4, 3);
    r.max(0)
}

fn add_history(hist: &mut [[i16; 64]; 64], m: Move, bonus: i32) {
    let f = m.from().idx();
    let t = m.to().idx();
    let v = hist[f][t] as i32 + bonus;
    hist[f][t] = v.clamp(-HIST_MAX, HIST_MAX) as i16;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Position;
    use crate::time::{TimeLimit, TimeManager};
    use crate::tt::TranspositionTable;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn go_startpos_depth8_sane() {
        crate::init();
        let mut pos = Position::new();
        let fen = pos.to_fen();
        let mut tt = TranspositionTable::new(16);
        let stop = AtomicBool::new(false);
        let mut limit = TimeLimit::default();
        limit.depth = Some(8);
        let time = TimeManager::new(&limit, true);
        let res = go(&mut pos, &mut tt, None, &stop, &time);
        assert_eq!(pos.to_fen(), fen, "search must restore the position");
        assert!(
            res.score.abs() < 2000,
            "implausible score {} best {}",
            res.score,
            res.best.to_uci()
        );
        assert!(!res.best.is_none());
    }

    #[test]
    fn lmr_history_scales() {
        crate::init();
        let mid = lmr_reduction(32, 32, false, false, 0);
        let good = lmr_reduction(32, 32, false, false, 8_192);
        let bad = lmr_reduction(32, 32, false, false, -8_192);
        assert!(good < mid);
        assert!(bad > mid);
        assert_eq!(mid - good, 3);
        assert_eq!(bad - mid, 4);
    }

    #[test]
    fn check_seen_before_make() {
        crate::init();
        let mut pos = Position::from_fen("4k3/8/8/8/4N3/8/8/4K3 w - - 0 1").unwrap();
        let check = Move::from_uci("e4d6").unwrap();
        let quiet = Move::from_uci("e4c3").unwrap();
        assert!(gives_check_on(&pos, check));
        assert!(!gives_check_on(&pos, quiet));
        pos.make(check);
        assert!(pos.in_check());
    }

    #[test]
    fn null_move_is_conservative_in_sparse_endgames() {
        crate::init();
        let rook_endgame =
            Position::from_fen("8/8/8/1r3kpR/5p1P/8/8/5K2 b - - 0 69").unwrap();
        assert_eq!(rook_endgame.piece_count(), 7);
        assert!(null_move_allowed(&rook_endgame));

        let minor_endgame =
            Position::from_fen("8/8/8/1n3kp1/5p1P/8/8/4NK2 b - - 0 1").unwrap();
        assert_eq!(minor_endgame.piece_count(), 7);
        assert!(!null_move_allowed(&minor_endgame));

        let start = Position::new();
        assert!(null_move_allowed(&start));
    }

    #[test]
    fn timeout_returns_last_completed_iteration() {
        crate::init();
        let mut timed_pos = Position::new();
        let mut timed_tt = TranspositionTable::new(16);
        let timed_stop = AtomicBool::new(false);
        let mut timed_limit = TimeLimit::default();
        timed_limit.movetime = Some(30);
        let timed = TimeManager::new(&timed_limit, true);
        let timed_result = go(
            &mut timed_pos,
            &mut timed_tt,
            None,
            &timed_stop,
            &timed,
        );
        assert!(timed_result.depth > 0);

        let mut fixed_pos = Position::new();
        let mut fixed_tt = TranspositionTable::new(16);
        let fixed_stop = AtomicBool::new(false);
        let mut fixed_limit = TimeLimit::default();
        fixed_limit.depth = Some(timed_result.depth);
        let fixed = TimeManager::new(&fixed_limit, true);
        let fixed_result = go(
            &mut fixed_pos,
            &mut fixed_tt,
            None,
            &fixed_stop,
            &fixed,
        );

        assert_eq!(timed_result.best, fixed_result.best);
        assert_eq!(timed_result.score, fixed_result.score);
        assert_eq!(timed_result.depth, fixed_result.depth);
    }
}
