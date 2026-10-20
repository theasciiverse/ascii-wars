//! Settings and records, a few `key value` lines in
//! `$XDG_DATA_HOME/ascii-wars/save.txt` (`~/.local/share` without it,
//! `%APPDATA%` on Windows). A missing or unreadable file is a fresh start.

use crate::game::{Difficulty, MAX_LEVEL};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Save {
    pub difficulty: Difficulty,
    pub sound: bool,
    pub music: bool,
    pub shake: bool,
    /// The furthest boss reached in Arcade; Practice offers up to it.
    pub reached: i32,
    /// Best Arcade score for each difficulty.
    pub best: [u64; 3],
    /// Fastest win against each boss, in seconds, any mode but Classic.
    pub best_time: [Option<f32>; MAX_LEVEL as usize],
    /// Arcade cleared, on any difficulty.
    pub cleared: bool,
}

impl Default for Save {
    fn default() -> Self {
        Save {
            difficulty: Difficulty::Normal,
            sound: true,
            music: true,
            shake: true,
            reached: 1,
            best: [0; 3],
            best_time: [None; MAX_LEVEL as usize],
            cleared: false,
        }
    }
}

pub fn path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").filter(|v| !v.is_empty()).map(|h| PathBuf::from(h).join(".local/share")))
    };
    base.map(|b| b.join("ascii-wars").join("save.txt"))
}

impl Save {
    pub fn load() -> Save {
        let Some(p) = path() else { return Save::default() };
        match std::fs::read_to_string(&p) {
            Ok(text) => Save::parse(&text),
            Err(_) => Save::default(),
        }
    }

    pub fn parse(text: &str) -> Save {
        let mut s = Save::default();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            let (Some(k), Some(v)) = (it.next(), it.next()) else { continue };
            let on = v == "on";
            match k {
                "difficulty" => s.difficulty = Difficulty::ALL.into_iter().find(|d| d.name() == v).unwrap_or(Difficulty::Normal),
                "sound" => s.sound = on,
                "music" => s.music = on,
                "shake" => s.shake = on,
                "reached" => s.reached = v.parse().unwrap_or(1).clamp(1, MAX_LEVEL),
                "cleared" => s.cleared = on,
                _ => {
                    if let Some(d) = k.strip_prefix("best.") {
                        if let Some(i) = Difficulty::ALL.iter().position(|x| x.name() == d) {
                            s.best[i] = v.parse().unwrap_or(0);
                        }
                    } else if let Some(n) = k.strip_prefix("time.") {
                        if let (Ok(n), Ok(t)) = (n.parse::<usize>(), v.parse::<f32>()) {
                            if (1..=MAX_LEVEL as usize).contains(&n) && t.is_finite() && t > 0. {
                                s.best_time[n - 1] = Some(t);
                            }
                        }
                    }
                }
            }
        }
        s
    }

    pub fn text(&self) -> String {
        let flag = |b: bool| if b { "on" } else { "off" };
        let mut out = format!(
            "difficulty {}\nsound {}\nmusic {}\nshake {}\nreached {}\ncleared {}\n",
            self.difficulty.name(),
            flag(self.sound),
            flag(self.music),
            flag(self.shake),
            self.reached,
            flag(self.cleared)
        );
        for d in Difficulty::ALL {
            out += &format!("best.{} {}\n", d.name(), self.best[d.index()]);
        }
        for (i, t) in self.best_time.iter().enumerate() {
            if let Some(t) = t {
                out += &format!("time.{} {:.2}\n", i + 1, t);
            }
        }
        out
    }

    pub fn store(&self) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&p, self.text()) {
            log::warn!("could not save: {e}");
        }
    }
}
