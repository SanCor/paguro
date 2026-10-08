//! Staged move picker: TT, good captures, killers, quiets, bad captures.

use crate::board::Position;
use crate::movegen::{Gen, MoveList};
use crate::types::*;

const STAGE_TT: u8 = 0;
const STAGE_GEN_CAP: u8 = 1;
const STAGE_GOOD_CAP: u8 = 2;
const STAGE_KILLERS: u8 = 3;
const STAGE_GEN_QUIET: u8 = 4;
const STAGE_QUIET: u8 = 5;
const STAGE_BAD_CAP: u8 = 6;
const STAGE_DONE: u8 = 7;

const GOOD_SEE: i32 = 0;

pub struct MovePicker {
    tt_move: Move,
    killers: [Move; 2],
    stage: u8,
    list: MoveList,
    index: usize,
    killer_i: usize,
    bad_start: usize,
    bad_end: usize,
    qsearch: bool,
    in_check: bool,
    pub last_see: i32,
}

impl MovePicker {
    pub fn main(tt_move: Move, killers: [Move; 2], in_check: bool) -> MovePicker {
        MovePicker {
            tt_move,
            killers,
            stage: STAGE_TT,
            list: MoveList::new(),
            index: 0,
            killer_i: 0,
            bad_start: 0,
            bad_end: 0,
            qsearch: false,
            in_check,
            last_see: 0,
        }
    }

    pub fn qsearch(in_check: bool) -> MovePicker {
        MovePicker {
            tt_move: Move::NONE,
            killers: [Move::NONE; 2],
            stage: if in_check { STAGE_TT } else { STAGE_GEN_CAP },
            list: MoveList::new(),
            index: 0,
            killer_i: 0,
            bad_start: 0,
            bad_end: 0,
            qsearch: true,
            in_check,
            last_see: 0,
        }
    }

    pub fn next(&mut self, pos: &mut Position, history: &[[i16; 64]; 64]) -> Option<Move> {
        if self.in_check {
            return self.next_evasion(pos, history);
        }
        loop {
            match self.stage {
                STAGE_TT => {
                    self.stage = STAGE_GEN_CAP;
                    if usable(pos, self.tt_move) {
                        self.last_see = if pos.is_capture_decoded(self.tt_move)
                            || self.tt_move.is_promotion()
                        {
                            pos.see_decoded(self.tt_move)
                        } else {
                            0
                        };
                        return Some(self.tt_move);
                    }
                }
                STAGE_GEN_CAP => {
                    pos.gen(&mut self.list, Gen::Tactical);
                    pos.filter_legal(&mut self.list, 0);
                    for i in 0..self.list.len {
                        self.list.scores[i] = pos.see_decoded(self.list.moves[i]);
                    }
                    self.index = 0;
                    self.bad_end = self.list.len;
                    self.stage = STAGE_GOOD_CAP;
                }
                STAGE_GOOD_CAP => {
                    if let Some(m) = self.pick_best_if(GOOD_SEE) {
                        if same(pos, m, self.tt_move) {
                            continue;
                        }
                        self.last_see = self.list.scores[self.index - 1];
                        return Some(m);
                    }
                    self.bad_start = self.index;
                    self.stage = if self.qsearch {
                        STAGE_DONE
                    } else {
                        STAGE_KILLERS
                    };
                }
                STAGE_KILLERS => {
                    while self.killer_i < 2 {
                        let m = self.killers[self.killer_i];
                        self.killer_i += 1;
                        if m.is_none() || same(pos, m, self.tt_move) {
                            continue;
                        }
                        if pos.is_capture_decoded(m) || m.is_promotion() {
                            continue;
                        }
                        if usable(pos, m) {
                            self.last_see = 0;
                            return Some(m);
                        }
                    }
                    self.stage = STAGE_GEN_QUIET;
                }
                STAGE_GEN_QUIET => {
                    let start = self.list.len;
                    pos.gen(&mut self.list, Gen::Quiet);
                    pos.filter_legal(&mut self.list, start);
                    for i in start..self.list.len {
                        let m = self.list.moves[i];
                        self.list.scores[i] = history[m.from().idx()][m.to().idx()] as i32;
                    }
                    self.index = start;
                    self.stage = STAGE_QUIET;
                }
                STAGE_QUIET => {
                    if let Some(m) = self.pick_best_if(i32::MIN) {
                        if same(pos, m, self.tt_move)
                            || same(pos, m, self.killers[0])
                            || same(pos, m, self.killers[1])
                        {
                            continue;
                        }
                        self.last_see = 0;
                        return Some(m);
                    }
                    self.index = self.bad_start;
                    self.stage = STAGE_BAD_CAP;
                }
                STAGE_BAD_CAP => {
                    let saved_len = self.list.len;
                    self.list.len = self.bad_end;
                    let m = self.pick_best_if(i32::MIN);
                    self.list.len = saved_len;
                    if let Some(m) = m {
                        if self.index > self.bad_end {
                            self.stage = STAGE_DONE;
                            continue;
                        }
                        if same(pos, m, self.tt_move) {
                            continue;
                        }
                        self.last_see = self.list.scores[self.index - 1];
                        return Some(m);
                    }
                    self.stage = STAGE_DONE;
                }
                _ => return None,
            }
        }
    }

    fn next_evasion(&mut self, pos: &mut Position, history: &[[i16; 64]; 64]) -> Option<Move> {
        if self.stage == STAGE_TT {
            self.list = pos.legal_moves();
            score_all(pos, &mut self.list, self.tt_move, self.killers, history);
            self.index = 0;
            self.stage = STAGE_GOOD_CAP;
        }
        if self.index >= self.list.len {
            return None;
        }
        let m = pick(&mut self.list, self.index);
        self.last_see = if pos.is_capture_decoded(m) || m.is_promotion() {
            pos.see_decoded(m)
        } else {
            0
        };
        self.index += 1;
        Some(m)
    }

    fn pick_best_if(&mut self, min_score: i32) -> Option<Move> {
        if self.index >= self.list.len {
            return None;
        }
        let mut best = self.index;
        for i in (self.index + 1)..self.list.len {
            if self.list.scores[i] > self.list.scores[best] {
                best = i;
            }
        }
        if self.list.scores[best] < min_score {
            return None;
        }
        self.list.moves.swap(self.index, best);
        self.list.scores.swap(self.index, best);
        let m = self.list.moves[self.index];
        self.index += 1;
        Some(m)
    }
}

#[inline]
fn same(_pos: &Position, a: Move, b: Move) -> bool {
    !a.is_none() && !b.is_none() && a == b
}

fn usable(pos: &mut Position, m: Move) -> bool {
    pos.is_legal(m)
}

fn score_all(
    pos: &Position,
    list: &mut MoveList,
    tt_move: Move,
    killers: [Move; 2],
    history: &[[i16; 64]; 64],
) {
    for i in 0..list.len {
        let m = list.moves[i];
        list.scores[i] = if same(pos, m, tt_move) {
            1_000_000
        } else if pos.is_capture_decoded(m) || m.is_promotion() {
            let s = pos.see_decoded(m);
            if s >= 0 {
                200_000 + s
            } else {
                -20_000 + s
            }
        } else if same(pos, m, killers[0]) {
            90_000
        } else if same(pos, m, killers[1]) {
            80_000
        } else {
            history[m.from().idx()][m.to().idx()] as i32
        };
    }
}

pub fn pick(list: &mut MoveList, start: usize) -> Move {
    let mut best = start;
    for i in (start + 1)..list.len {
        if list.scores[i] > list.scores[best] {
            best = i;
        }
    }
    list.moves.swap(start, best);
    list.scores.swap(start, best);
    list.moves[start]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Position;

    fn collect(pos: &mut Position, tt: Move, killers: [Move; 2]) -> Vec<u16> {
        let hist = [[0i16; 64]; 64];
        let mut p = MovePicker::main(tt, killers, pos.in_check());
        let mut v = Vec::new();
        while let Some(m) = p.next(pos, &hist) {
            v.push(pos.decode_move(m).0);
        }
        v.sort_unstable();
        v
    }

    fn legal_keys(pos: &mut Position) -> Vec<u16> {
        let legal = pos.legal_moves();
        let mut a: Vec<u16> = legal.iter().map(|m| pos.decode_move(m).0).collect();
        a.sort_unstable();
        a
    }

    #[test]
    fn picker_matches_legal_startpos() {
        crate::init();
        let mut pos = Position::new();
        let a = legal_keys(&mut pos);
        let b = collect(&mut pos, Move::NONE, [Move::NONE; 2]);
        assert_eq!(a, b);
    }

    #[test]
    fn picker_matches_legal_kiwipete() {
        crate::init();
        let mut pos = Position::from_fen(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        )
        .unwrap();
        let legal = pos.legal_moves();
        let tt = legal.moves[3];
        let mut a: Vec<u16> = legal.iter().map(|m| pos.decode_move(m).0).collect();
        a.sort_unstable();
        let b = collect(&mut pos, tt, [Move::NONE; 2]);
        assert_eq!(a.len(), legal.len);
        assert_eq!(a, b);
    }
}
