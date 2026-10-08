//! Transposition table.

use crate::types::{Move, MAX_PLY};

pub const BOUND_NONE: u8 = 0;
pub const BOUND_EXACT: u8 = 1;
pub const BOUND_LOWER: u8 = 2;
pub const BOUND_UPPER: u8 = 3;

#[derive(Clone, Copy, Default)]
pub struct TTEntry {
    pub key: u16,
    pub m: Move,
    pub score: i16,
    pub depth: i8,
    pub bound: u8,
    pub generation: u8,
}

const CLUSTER_SIZE: usize = 4;
type Cluster = [TTEntry; CLUSTER_SIZE];

pub struct TranspositionTable {
    table: Vec<Cluster>,
    mask: usize,
    generation: u8,
}

impl TranspositionTable {
    pub fn new(mb: usize) -> TranspositionTable {
        let bytes = mb.max(1) * 1024 * 1024;
        let mut n = 1usize;
        while n * std::mem::size_of::<Cluster>() < bytes {
            n *= 2;
        }
        n /= 2;
        n = n.max(1024);
        TranspositionTable {
            table: vec![[TTEntry::default(); CLUSTER_SIZE]; n],
            mask: n - 1,
            generation: 0,
        }
    }

    pub fn resize(&mut self, mb: usize) {
        *self = TranspositionTable::new(mb);
    }

    pub fn clear(&mut self) {
        for cluster in self.table.iter_mut() {
            *cluster = [TTEntry::default(); CLUSTER_SIZE];
        }
    }

    pub fn new_search(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    #[inline]
    fn index(&self, key: u64) -> usize {
        (key as usize) & self.mask
    }

    pub fn probe(&self, key: u64) -> Option<TTEntry> {
        let key16 = (key >> 48) as u16;
        self.table[self.index(key)]
            .iter()
            .filter(|e| e.key == key16 && e.bound != BOUND_NONE)
            .max_by_key(|e| e.depth)
            .copied()
    }

    pub fn store(&mut self, key: u64, depth: i32, score: i32, bound: u8, m: Move, ply: i32) {
        let key16 = (key >> 48) as u16;
        let generation = self.generation;
        let cluster = &mut self.table[(key as usize) & self.mask];
        let replace = cluster
            .iter()
            .position(|e| e.key == key16 && e.bound != BOUND_NONE)
            .or_else(|| cluster.iter().position(|e| e.bound == BOUND_NONE))
            .unwrap_or_else(|| {
                cluster
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, e)| {
                        let age = generation.wrapping_sub(e.generation) as i16;
                        e.depth as i16 - age * 8
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            });
        let e = &mut cluster[replace];
        if e.key != key16
            || depth as i8 >= e.depth
            || bound == BOUND_EXACT
            || e.bound == BOUND_NONE
        {
            e.key = key16;
            e.m = m;
            e.score = to_tt(score, ply) as i16;
            e.depth = depth as i8;
            e.bound = bound;
            e.generation = generation;
        }
    }
}

pub fn to_tt(score: i32, ply: i32) -> i32 {
    if score >= crate::types::MATE_IN_MAX {
        score + ply
    } else if score <= -crate::types::MATE_IN_MAX {
        score - ply
    } else {
        score
    }
}

pub fn from_tt(score: i32, ply: i32) -> i32 {
    if score >= crate::types::MATE_IN_MAX {
        score - ply
    } else if score <= -crate::types::MATE_IN_MAX {
        score + ply
    } else {
        score
    }
}

pub fn mate_in(ply: i32) -> i32 {
    crate::types::MATE - ply
}

pub fn mated_in(ply: i32) -> i32 {
    -crate::types::MATE + ply
}

#[allow(dead_code)]
fn _max_ply() -> usize {
    MAX_PLY
}
