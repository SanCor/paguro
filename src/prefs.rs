//! Preferences file (`paguro.ini`) and data-file lookup.

use std::fs;
use std::path::{Path, PathBuf};

pub const PREFS_FILE: &str = "paguro.ini";
pub const DEFAULT_BOOK_FILE: &str = "book.bin";

#[derive(Clone, Debug)]
pub struct Prefs {
    pub own_book: bool,
    pub book_file: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            own_book: true,
            book_file: DEFAULT_BOOK_FILE.to_string(),
        }
    }
}

impl Prefs {
    pub fn load() -> Prefs {
        match find_file(PREFS_FILE, &[]) {
            Some(path) => match fs::read_to_string(&path) {
                Ok(text) => {
                    println!("info string loaded preferences {}", path.display());
                    parse_ini(&text)
                }
                Err(e) => {
                    println!("info string preferences read failed: {e}");
                    Prefs::default()
                }
            },
            None => Prefs::default(),
        }
    }
}

fn parse_ini(text: &str) -> Prefs {
    let mut prefs = Prefs::default();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let (key, value) = match line.split_once('=') {
            Some((k, v)) => (k.trim(), strip_quotes(v.trim())),
            None => continue,
        };
        match key.to_ascii_lowercase().as_str() {
            "ownbook" | "own_book" => {
                if let Some(b) = parse_bool(value) {
                    prefs.own_book = b;
                }
            }
            "bookfile" | "book_file" => {
                prefs.book_file = value.to_string();
            }
            _ => {}
        }
    }
    prefs
}

fn strip_quotes(s: &str) -> &str {
    if s.len() >= 2 {
        let b = s.as_bytes();
        if (b[0] == b'"' && b[s.len() - 1] == b'"') || (b[0] == b'\'' && b[s.len() - 1] == b'\'') {
            return &s[1..s.len() - 1];
        }
    }
    s
}

pub fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Look for `name` in the working directory, extra subdirs, the engine directory,
/// and the crate root (useful when running from `cargo`).
pub fn find_file(name: &str, extra_subdirs: &[&str]) -> Option<PathBuf> {
    if name.trim().is_empty() {
        return None;
    }
    let p = PathBuf::from(name);
    if p.exists() {
        return Some(p);
    }

    let mut cands: Vec<PathBuf> = Vec::new();
    cands.push(p.clone());
    for d in extra_subdirs {
        cands.push(PathBuf::from(d).join(&p));
        if let Some(file) = p.file_name() {
            cands.push(PathBuf::from(d).join(file));
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            push_dir_cands(&mut cands, dir, &p, extra_subdirs);
        }
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    push_dir_cands(&mut cands, &manifest, &p, extra_subdirs);

    cands.into_iter().find(|c| Path::new(c).exists())
}

fn push_dir_cands(cands: &mut Vec<PathBuf>, dir: &Path, p: &Path, extra_subdirs: &[&str]) {
    cands.push(dir.join(p));
    if let Some(file) = p.file_name() {
        cands.push(dir.join(file));
        for d in extra_subdirs {
            cands.push(dir.join(d).join(file));
        }
    }
    for d in extra_subdirs {
        cands.push(dir.join(d).join(p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample_ini() {
        let p = parse_ini(
            "# comment\nOwnBook = false\nBookFile = C:\\books\\mine.bin\n",
        );
        assert!(!p.own_book);
        assert_eq!(p.book_file, "C:\\books\\mine.bin");
    }
}
