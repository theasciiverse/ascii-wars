//! Keyboard and gamepad (taken over from Ceres).
//!
//! - Move: arrow keys or WASD, the d-pad or the left stick. Moving fires.
//! - Esc or P, pad Start: pause.
//! - Menus: up and down to choose, Enter, Space or pad A to press.
//!
//! The original reads held keys every frame. With the kitty keyboard
//! protocol presses and releases are exact. Without it the terminal only
//! sends key repeats, and a key counts as held while they keep arriving:
//! playable, but a move goes on a little after the key is let go.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::{Duration, Instant};

const PRE_REPEAT: Duration = Duration::from_millis(500);
const REPEAT_GAP: Duration = Duration::from_millis(100);
#[cfg(feature = "gamepad")]
const STICK: f32 = 0.5;

const LEFT: usize = 0;
const RIGHT: usize = 1;
const UP: usize = 2;
const DOWN: usize = 3;

/// Commands, as edges.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cmd {
    Pause,
    Confirm,
    Up,
    Down,
    Exit,
}

#[derive(Clone, Copy, Default)]
struct Key {
    down: bool,
    /// Pressed since the game last read the keys.
    latched: bool,
    last: Option<Instant>,
    repeats: u32,
}

#[cfg(feature = "gamepad")]
fn usable_pad(p: &gilrs::Gamepad) -> bool {
    p.mapping_source() == gilrs::MappingSource::SdlMappings || std::env::var("ASCII_WARS_ANY_PAD").is_ok_and(|v| v == "1")
}

pub struct Input {
    pub exact: bool,
    keys: [Key; 4],
    pub cmds: Vec<Cmd>,
    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
    pub pad_name: Option<String>,
    pad: [bool; 4],
    pad_latched: [bool; 4],
}

impl Input {
    pub fn new(exact: bool) -> Input {
        Input {
            exact,
            keys: [Key::default(); 4],
            cmds: Vec::new(),
            #[cfg(feature = "gamepad")]
            gilrs: None,
            pad_name: None,
            pad: [false; 4],
            pad_latched: [false; 4],
        }
    }

    pub fn on_key(&mut self, k: KeyEvent, now: Instant) {
        let press = k.kind != KeyEventKind::Release;
        let fresh = k.kind == KeyEventKind::Press;
        if fresh && k.modifiers.contains(KeyModifiers::CONTROL) && matches!(k.code, KeyCode::Char('c' | 'C')) {
            self.cmds.push(Cmd::Exit);
            return;
        }
        let dir = match k.code {
            KeyCode::Left | KeyCode::Char('a' | 'A') => Some(LEFT),
            KeyCode::Right | KeyCode::Char('d' | 'D') => Some(RIGHT),
            KeyCode::Up | KeyCode::Char('w' | 'W') => Some(UP),
            KeyCode::Down | KeyCode::Char('s' | 'S') => Some(DOWN),
            _ => None,
        };
        if let Some(d) = dir {
            let key = &mut self.keys[d];
            if self.exact {
                if press && !key.down {
                    key.latched = true;
                }
                key.down = press;
            } else {
                // without release events every repeat arrives as a press
                if held_by_repeat(key, now) {
                    key.repeats += 1;
                } else {
                    key.repeats = 0;
                    key.latched = true;
                }
                key.last = Some(now);
            }
        }
        if !press {
            return;
        }
        // menus step on repeats too
        match dir {
            Some(UP) => self.cmds.push(Cmd::Up),
            Some(DOWN) => self.cmds.push(Cmd::Down),
            _ => {}
        }
        if !fresh {
            return;
        }
        match k.code {
            KeyCode::Esc | KeyCode::Char('p' | 'P') => self.cmds.push(Cmd::Pause),
            KeyCode::Enter | KeyCode::Char(' ') => self.cmds.push(Cmd::Confirm),
            _ => {}
        }
    }

    /// The window lost focus: releases will not arrive, so let go of
    /// everything.
    pub fn release_all(&mut self) {
        for k in &mut self.keys {
            k.down = false;
            k.last = None;
        }
    }

    pub fn open_pad(&mut self) {
        #[cfg(feature = "gamepad")]
        match gilrs::Gilrs::new() {
            Ok(g) => {
                let all: Vec<_> = g.gamepads().map(|(_, p)| (p.name().to_string(), usable_pad(&p))).collect();
                log::info!("gamepads (name, used): {all:?}");
                self.pad_name = all.into_iter().find(|(_, used)| *used).map(|(n, _)| n);
                self.gilrs = Some(g);
            }
            Err(e) => log::warn!("gamepad support unavailable: {e}"),
        }
    }

    /// Drain gamepad events; call before reading the keys.
    pub fn poll_pad(&mut self) {
        #[cfg(feature = "gamepad")]
        {
            use gilrs::{Axis, Button, EventType};
            let Some(g) = &mut self.gilrs else { return };
            let mut events = Vec::new();
            while let Some(ev) = g.next_event() {
                events.push(ev);
            }
            for ev in events {
                let g = self.gilrs.as_ref().unwrap();
                let pad = g.gamepad(ev.id);
                if !usable_pad(&pad) {
                    continue;
                }
                match ev.event {
                    EventType::Connected => {
                        self.pad_name = Some(pad.name().to_string());
                        log::info!("gamepad connected: {:?}", self.pad_name);
                    }
                    EventType::Disconnected => {
                        log::info!("gamepad disconnected");
                        self.pad_name = None;
                    }
                    EventType::ButtonPressed(b, _) => {
                        log::debug!("pad press {b:?}");
                        match b {
                            Button::South | Button::East => self.cmds.push(Cmd::Confirm),
                            Button::Start | Button::Select => self.cmds.push(Cmd::Pause),
                            _ => {}
                        }
                    }
                    EventType::ButtonReleased(b, _) => log::debug!("pad release {b:?}"),
                    _ => {}
                }
                let on = |b: Button| pad.is_pressed(b);
                let axis = |a: Axis| pad.axis_data(a).map_or(0.0, |d| d.value());
                let (sx, sy) = (axis(Axis::LeftStickX), axis(Axis::LeftStickY));
                let now = [
                    on(Button::DPadLeft) || sx < -STICK,
                    on(Button::DPadRight) || sx > STICK,
                    on(Button::DPadUp) || sy > STICK,
                    on(Button::DPadDown) || sy < -STICK,
                ];
                for d in 0..4 {
                    if now[d] && !self.pad[d] {
                        self.pad_latched[d] = true;
                        match d {
                            UP => self.cmds.push(Cmd::Up),
                            DOWN => self.cmds.push(Cmd::Down),
                            _ => {}
                        }
                    }
                }
                self.pad = now;
            }
        }
    }

    /// The held direction for this frame, each axis -1, 0 or 1 (y up, as
    /// in the original). Clears the latches.
    pub fn direction(&mut self, now: Instant) -> (i8, i8) {
        let mut held = [false; 4];
        for (d, h) in held.iter_mut().enumerate() {
            let k = &mut self.keys[d];
            let down = if self.exact { k.down } else { held_by_repeat(k, now) };
            *h = down || k.latched || self.pad[d] || self.pad_latched[d];
            k.latched = false;
            self.pad_latched[d] = false;
        }
        // as the original's input.rs: right wins over left, up and down cancel
        let x = if held[RIGHT] { 1 } else if held[LEFT] { -1 } else { 0 };
        let y = if held[UP] && !held[DOWN] {
            1
        } else if held[DOWN] {
            if held[UP] { 0 } else { -1 }
        } else {
            0
        };
        (x, y)
    }
}

fn held_by_repeat(k: &Key, now: Instant) -> bool {
    let Some(last) = k.last else { return false };
    let window = if k.repeats == 0 { PRE_REPEAT } else { REPEAT_GAP };
    now.duration_since(last) <= window
}
