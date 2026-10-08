//! Time management for UCI go.

use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct TimeLimit {
    pub depth: Option<u32>,
    pub movetime: Option<u64>,
    pub wtime: Option<u64>,
    pub btime: Option<u64>,
    pub winc: u64,
    pub binc: u64,
    pub movestogo: Option<u32>,
    pub infinite: bool,
}

impl Default for TimeLimit {
    fn default() -> Self {
        TimeLimit {
            depth: None,
            movetime: None,
            wtime: None,
            btime: None,
            winc: 0,
            binc: 0,
            movestogo: None,
            infinite: false,
        }
    }
}

pub struct TimeManager {
    pub start: Instant,
    pub hard: Option<Duration>,
    pub soft: Option<Duration>,
    pub max_depth: u32,
}

impl TimeManager {
    pub fn new(limit: &TimeLimit, white_to_move: bool) -> TimeManager {
        let mut hard = None;
        let mut soft = None;
        let mut max_depth = 128;

        if let Some(d) = limit.depth {
            max_depth = d;
        }

        if limit.infinite {
            max_depth = 128;
        } else if let Some(ms) = limit.movetime {
            hard = Some(Duration::from_millis(ms.saturating_sub(15).max(1)));
        } else if let Some(our) = if white_to_move {
            limit.wtime
        } else {
            limit.btime
        } {
            let inc = if white_to_move { limit.winc } else { limit.binc };
            let mtg = limit.movestogo.unwrap_or(30).max(1) as u64;
            let budget = our / mtg + (inc * 4) / 5;
            let ms = budget.min(our / 2).saturating_sub(20).max(1);
            hard = Some(Duration::from_millis(ms));
            soft = Some(Duration::from_millis(((ms * 65) / 100).max(1)));
        }

        TimeManager {
            start: Instant::now(),
            hard,
            soft,
            max_depth,
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    pub fn expired(&self) -> bool {
        if let Some(d) = self.hard {
            self.start.elapsed() >= d
        } else {
            false
        }
    }

    pub fn should_stop_id(&self) -> bool {
        if let Some(d) = self.soft {
            self.start.elapsed() >= d
        } else {
            false
        }
    }
}
