//! UCI protocol loop.

use crate::board::{Position, START_FEN};
use crate::book::Book;
use crate::nnue::network::Network;
use crate::prefs::{self, Prefs};
use crate::search;
use crate::time::{TimeLimit, TimeManager};
use crate::tt::TranspositionTable;
use crate::types::*;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

pub const ENGINE_NAME: &str = "Paguro";
pub const ENGINE_AUTHOR: &str = "Sandro Corsini";
pub const DEFAULT_EVAL: &str = "nn-3475407dc199.nnue";

pub struct Engine {
    pos: Position,
    tt: Arc<Mutex<TranspositionTable>>,
    net: Option<Arc<Network>>,
    stop: Arc<AtomicBool>,
    search: Option<JoinHandle<()>>,
    hash_mb: usize,
    eval_file: String,
    own_book: bool,
    book_file: String,
    book: Book,
}

impl Engine {
    pub fn new() -> Engine {
        let prefs = Prefs::load();
        let eval_file = DEFAULT_EVAL.to_string();
        let mut eng = Engine {
            pos: Position::new(),
            tt: Arc::new(Mutex::new(TranspositionTable::new(16))),
            net: None,
            stop: Arc::new(AtomicBool::new(false)),
            search: None,
            hash_mb: 16,
            eval_file,
            own_book: prefs.own_book,
            book_file: prefs.book_file,
            book: Book::empty(),
        };
        eng.load_net();
        eng.load_book();
        eng
    }

    fn load_net(&mut self) {
        if let Some(path) = find_eval_file(&self.eval_file) {
            match Network::load(&path) {
                Ok(net) => {
                    self.net = Some(Arc::new(net));
                    println!("info string loaded NNUE {}", path.display());
                }
                Err(e) => {
                    self.net = None;
                    println!("info string NNUE load failed: {e}");
                }
            }
            let _ = io::stdout().flush();
        } else {
            self.net = None;
            println!(
                "info string NNUE file not found (looked for {}); using HCE",
                self.eval_file
            );
            let _ = io::stdout().flush();
        }
    }

    fn load_book(&mut self) {
        self.book = Book::empty();
        let name = self.book_file.trim();
        if name.is_empty() {
            if self.own_book {
                println!("info string opening book disabled (BookFile empty)");
                let _ = io::stdout().flush();
            }
            return;
        }
        match prefs::find_file(name, &["book", "books"]) {
            Some(path) => match Book::load(&path) {
                Ok(book) => {
                    println!(
                        "info string loaded Polyglot book {} ({} entries)",
                        path.display(),
                        book.len()
                    );
                    self.book = book;
                }
                Err(e) => {
                    println!("info string book load failed: {e}");
                }
            },
            None => {
                if self.own_book {
                    println!("info string opening book not found (looked for {name})");
                }
            }
        }
        let _ = io::stdout().flush();
    }

    fn stop_search(&mut self) {
        if let Some(h) = self.search.take() {
            self.stop.store(true, Ordering::SeqCst);
            let _ = h.join();
        }
        self.stop.store(false, Ordering::SeqCst);
    }

    fn join_search(&mut self) {
        if let Some(h) = self.search.take() {
            let _ = h.join();
        }
        self.stop.store(false, Ordering::SeqCst);
    }

    fn handle(&mut self, line: &str) -> bool {
        let line = line.trim();
        if line.is_empty() {
            return true;
        }
        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or("");
        match cmd {
            "uci" => {
                println!("id name {ENGINE_NAME}");
                println!("id author {ENGINE_AUTHOR}");
                println!("option name Hash type spin default 16 min 1 max 4096");
                println!("option name Threads type spin default 1 min 1 max 1");
                println!("option name EvalFile type string default {DEFAULT_EVAL}");
                let own = if self.own_book { "true" } else { "false" };
                println!("option name OwnBook type check default {own}");
                println!("option name BookFile type string default {}", self.book_file);
                println!("uciok");
            }
            "isready" => {
                if self.net.is_none() {
                    self.load_net();
                }
                println!("readyok");
            }
            "ucinewgame" => {
                self.stop_search();
                self.tt.lock().unwrap().clear();
                self.pos = Position::new();
            }
            "setoption" => {
                self.stop_search();
                parse_setoption(line, self);
            }
            "position" => {
                self.stop_search();
                parse_position(line, &mut self.pos);
            }
            "go" => {
                self.stop_search();
                let limit = parse_go(line);
                if self.own_book && !limit.infinite {
                    if let Some(m) = self.book.probe(&mut self.pos) {
                        println!("info string book {}", m.to_uci());
                        println!("bestmove {}", m.to_uci());
                        let _ = io::stdout().flush();
                        return true;
                    }
                }
                let mut pos = self.pos.clone();
                let tt = self.tt.clone();
                let net = self.net.clone();
                let stop = self.stop.clone();
                stop.store(false, Ordering::SeqCst);
                self.search = Some(thread::spawn(move || {
                    let time = TimeManager::new(&limit, pos.side == Color::White);
                    println!(
                        "info string search max_depth={} hard_ms={} soft_ms={}",
                        time.max_depth,
                        time.hard.map(|d| d.as_millis()).unwrap_or(0),
                        time.soft.map(|d| d.as_millis()).unwrap_or(0)
                    );
                    let mut tt = tt.lock().unwrap();
                    let res = search::go(&mut pos, &mut tt, net.as_deref(), &stop, &time);
                    let mv = if res.best.is_none() {
                        "0000".to_string()
                    } else {
                        res.best.to_uci()
                    };
                    println!("bestmove {mv}");
                    let _ = io::stdout().flush();
                }));
            }
            "stop" => {
                self.stop.store(true, Ordering::SeqCst);
                if let Some(h) = self.search.take() {
                    let _ = h.join();
                }
            }
            "quit" => {
                self.stop_search();
                return false;
            }
            "eval" => {
                let score = crate::eval::evaluate(&mut self.pos, self.net.as_deref());
                println!("info string eval {score} cp (stm)");
            }
            "d" => {
                println!("{}", self.pos.to_fen());
            }
            _ => {}
        }
        let _ = io::stdout().flush();
        true
    }
}

pub fn run() {
    let mut engine = Engine::new();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(line) => {
                if !engine.handle(&line) {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    engine.join_search();
}

fn parse_setoption(line: &str, eng: &mut Engine) {
    // setoption name Hash value 32
    let rest = line.strip_prefix("setoption").unwrap_or(line).trim();
    let rest = rest.strip_prefix("name").unwrap_or(rest).trim();
    if let Some(idx) = rest.to_ascii_lowercase().find(" value") {
        let name = rest[..idx].trim();
        let value = rest[idx + 6..].trim();
        match name.to_ascii_lowercase().as_str() {
            "hash" => {
                if let Ok(mb) = value.parse::<usize>() {
                    eng.hash_mb = mb.max(1);
                    *eng.tt.lock().unwrap() = TranspositionTable::new(eng.hash_mb);
                }
            }
            "evalfile" => {
                eng.eval_file = value.to_string();
                eng.load_net();
            }
            "ownbook" => {
                if let Some(b) = prefs::parse_bool(value) {
                    eng.own_book = b;
                }
            }
            "bookfile" => {
                eng.book_file = value.to_string();
                eng.load_book();
            }
            _ => {}
        }
    }
}

fn parse_position(line: &str, pos: &mut Position) {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let mut i = 1;
    if i < tokens.len() && tokens[i] == "startpos" {
        *pos = Position::from_fen(START_FEN).unwrap();
        i += 1;
    } else if i < tokens.len() && tokens[i] == "fen" {
        i += 1;
        let mut fen = String::new();
        while i < tokens.len() && tokens[i] != "moves" {
            if !fen.is_empty() {
                fen.push(' ');
            }
            fen.push_str(tokens[i]);
            i += 1;
        }
        if let Ok(p) = Position::from_fen(&fen) {
            *pos = p;
        }
    }
    if i < tokens.len() && tokens[i] == "moves" {
        i += 1;
        while i < tokens.len() {
            if let Some(m) = Move::from_uci(tokens[i]) {
                let m = pos.decode_move(m);
                pos.make(m);
            }
            i += 1;
        }
    }
}

fn parse_go(line: &str) -> TimeLimit {
    let mut lim = TimeLimit::default();
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let mut i = 1;
    while i < tokens.len() {
        match tokens[i] {
            "depth" => {
                i += 1;
                lim.depth = tokens.get(i).and_then(|s| s.parse().ok());
            }
            "movetime" => {
                i += 1;
                lim.movetime = tokens.get(i).and_then(|s| s.parse().ok());
            }
            "wtime" => {
                i += 1;
                lim.wtime = tokens.get(i).and_then(|s| s.parse().ok());
            }
            "btime" => {
                i += 1;
                lim.btime = tokens.get(i).and_then(|s| s.parse().ok());
            }
            "winc" => {
                i += 1;
                lim.winc = tokens.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            "binc" => {
                i += 1;
                lim.binc = tokens.get(i).and_then(|s| s.parse().ok()).unwrap_or(0);
            }
            "movestogo" => {
                i += 1;
                lim.movestogo = tokens.get(i).and_then(|s| s.parse().ok());
            }
            "infinite" => {
                lim.infinite = true;
            }
            _ => {}
        }
        i += 1;
    }
    if lim.depth.is_none()
        && lim.movetime.is_none()
        && lim.wtime.is_none()
        && lim.btime.is_none()
        && !lim.infinite
    {
        lim.depth = Some(6);
    }
    lim
}

pub fn find_eval_file(explicit: &str) -> Option<PathBuf> {
    if let Some(p) = prefs::find_file(explicit, &["nets"]) {
        return Some(p);
    }
    if explicit != DEFAULT_EVAL {
        prefs::find_file(DEFAULT_EVAL, &["nets"])
    } else {
        None
    }
}
