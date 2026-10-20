//! Sound, all synthesised here; ASCII Wars! ships no audio files (as
//! Brickspire, Panzer and SIGPIPE do). Effects are rendered once at start;
//! the music is `music.rs`, one endless source on the same mixer.
//!
//! Without an audio device, or with `--mute`, every call does nothing.

use crate::game::Sound;
use crate::music::{self, Band, Ctrl};
use rodio::MixerDeviceSink;
use rodio::buffer::SamplesBuffer;
use std::f32::consts::TAU;
use std::num::NonZero;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const SR: u32 = 44_100;
const SRF: f32 = SR as f32;

const ALL: [Sound; 14] = [
    Sound::Shot,
    Sound::BossHit,
    Sound::Volley,
    Sound::LaserWarn,
    Sound::LaserFire,
    Sound::PlayerHit,
    Sound::Graze,
    Sound::Bomb,
    Sound::ExtraBomb,
    Sound::PhaseBreak,
    Sound::BossDown,
    Sound::PlayerDown,
    Sound::Warning,
    Sound::Win,
];

/// Menu blips, apart from the game's sounds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Ui {
    Move,
    Select,
}

pub struct Audio {
    // the sink must outlive everything playing on its mixer
    sink: Option<MixerDeviceSink>,
    bank: Vec<SamplesBuffer>,
    ui: [Option<SamplesBuffer>; 2],
    last: [Option<Instant>; ALL.len()],
    music: Arc<Mutex<Ctrl>>,
    pub sound: bool,
    pub music_on: bool,
}

impl Audio {
    pub fn new(off: bool, sound: bool, music_on: bool) -> Audio {
        let sink = if off {
            None
        } else {
            match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(mut s) => {
                    s.log_on_drop(false);
                    Some(s)
                }
                Err(e) => {
                    log::warn!("no audio: {e}");
                    None
                }
            }
        };
        let bank = if sink.is_some() { ALL.iter().map(|&s| buffer(render(s))).collect() } else { Vec::new() };
        let ui = if sink.is_some() {
            [Some(buffer(blip(880., 0.04, 0.08))), Some(buffer(chord(&[blip(660., 0.08, 0.1), blip(990., 0.12, 0.08)])))]
        } else {
            [None, None]
        };
        let music = Arc::new(Mutex::new(Ctrl::default()));
        if let Some(s) = &sink {
            s.mixer().add(Band::new(music.clone()));
        }
        let mut a = Audio { sink, bank, ui, last: [None; ALL.len()], music, sound, music_on };
        a.set_music(music_on);
        a
    }

    pub fn available(&self) -> bool {
        self.sink.is_some()
    }

    pub fn play(&mut self, s: Sound) {
        if !self.sound {
            return;
        }
        let Some(sink) = &self.sink else { return };
        let i = ALL.iter().position(|&x| x == s).unwrap_or(0);
        let now = Instant::now();
        let gap = match s {
            Sound::Shot | Sound::Volley | Sound::Graze | Sound::BossHit => Duration::from_millis(70),
            _ => Duration::from_millis(30),
        };
        if self.last[i].is_some_and(|t| now.duration_since(t) < gap) {
            return;
        }
        self.last[i] = Some(now);
        sink.mixer().add(self.bank[i].clone());
    }

    pub fn ui(&mut self, u: Ui) {
        if !self.sound {
            return;
        }
        if let (Some(sink), Some(b)) = (&self.sink, &self.ui[u as usize]) {
            sink.mixer().add(b.clone());
        }
    }

    /// The tune for a boss (1 to 10), the title (0), or silence (-1).
    pub fn tune(&mut self, n: i32) {
        music::request(&self.music, n);
    }

    /// How hard the music drives: 0 calm to 1 frantic.
    pub fn intensity(&mut self, k: f32) {
        music::intensity(&self.music, k);
    }

    pub fn set_music(&mut self, on: bool) {
        self.music_on = on;
        music::mute(&self.music, !on);
    }
}

fn buffer(samples: Vec<f32>) -> SamplesBuffer {
    SamplesBuffer::new(NonZero::new(1).unwrap(), NonZero::new(SR).unwrap(), samples)
}

// ------------------------------------------------------------------ helpers

struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

pub fn midi(n: f32) -> f32 {
    440.0 * 2f32.powf((n - 69.0) / 12.0)
}

pub fn square(ph: f32) -> f32 {
    if ph - ph.floor() < 0.5 { 1.0 } else { -1.0 }
}

pub fn tri(ph: f32) -> f32 {
    let p = ph - ph.floor();
    4.0 * (p - 0.5).abs() - 1.0
}

fn sine(ph: f32) -> f32 {
    (ph * TAU).sin()
}

fn synth(dur: f32, mut f: impl FnMut(f32, &mut Noise) -> f32) -> Vec<f32> {
    let mut n = Noise(0x1234_5678);
    (0..(dur * SRF) as usize).map(|i| f(i as f32 / SRF, &mut n)).collect()
}

fn blip(f: f32, dur: f32, gain: f32) -> Vec<f32> {
    synth(dur, |t, _| square(f * t) * (1. - t / dur) * gain)
}

/// A pitch gliding from `from` to `to` Hz on `wave`, fading out.
fn glide(dur: f32, from: f32, to: f32, gain: f32, wave: fn(f32) -> f32) -> Vec<f32> {
    let mut ph = 0.0f32;
    synth(dur, |t, _| {
        let k = t / dur;
        ph += (from + (to - from) * k) / SRF;
        wave(ph) * (t / 0.004).min(1.0) * (1.0 - k) * gain
    })
}

/// Noise through a low-pass whose cutoff sweeps from `from` to `to` Hz.
fn rumble(dur: f32, from: f32, to: f32, gain: f32, seed: u32) -> Vec<f32> {
    let mut n = Noise(seed);
    let mut lp = 0.0f32;
    let mut lp2 = 0.0f32;
    (0..(dur * SRF) as usize)
        .map(|i| {
            let t = i as f32 / SRF;
            let k = t / dur;
            let c = (TAU * (from + (to - from) * k) / SRF).min(0.9);
            lp += (n.next() - lp) * c;
            lp2 += (lp - lp2) * c;
            lp2 * (t / 0.01).min(1.0) * (1.0 - k).powf(1.5) * gain
        })
        .collect()
}

/// A little tune: (midi note, seconds) on `wave`.
fn tune(notes: &[(f32, f32)], gain: f32, wave: fn(f32) -> f32) -> Vec<f32> {
    let mut out = Vec::new();
    let mut ph = 0.0f32;
    for &(n, d) in notes {
        let len = (d * SRF) as usize;
        for i in 0..len {
            let t = i as f32 / SRF;
            if n > 0.0 {
                ph += midi(n) / SRF;
            }
            let env = (t / 0.006).min(1.0) * ((d - t) / 0.03).clamp(0.0, 1.0);
            out.push(if n > 0.0 { wave(ph) * env * gain } else { 0.0 });
        }
    }
    out
}

fn chord(parts: &[Vec<f32>]) -> Vec<f32> {
    let len = parts.iter().map(Vec::len).max().unwrap_or(0);
    (0..len).map(|i| parts.iter().map(|p| p.get(i).copied().unwrap_or(0.0)).sum()).collect()
}

fn render(s: Sound) -> Vec<f32> {
    match s {
        // the player's stream: a soft tick
        Sound::Shot => synth(0.03, |t, n| (square(1800. * t) * 0.4 + n.next() * 0.2) * (-t * 160.).exp() * 0.05),
        Sound::BossHit => chord(&[rumble(0.08, 3000., 800., 0.5, 3), glide(0.06, 300., 180., 0.08, square)]),
        // the boss fires: a short zap down
        Sound::Volley => glide(0.07, 1300., 500., 0.045, tri),
        // charging: a rising whine
        Sound::LaserWarn => glide(0.5, 300., 1400., 0.06, sine),
        // the beam: a buzz with a crack
        Sound::LaserFire => synth(0.45, |t, n| {
            let k = t / 0.45;
            (square(110. * t) * 0.4 + square(165. * t) * 0.25 + n.next() * 0.25) * (t / 0.005).min(1.0) * (1.0 - k) * 0.18
        }),
        Sound::PlayerHit => chord(&[rumble(0.3, 2500., 200., 1.0, 7), glide(0.25, 500., 90., 0.2, square)]),
        // a bullet brushing past: a bright tick
        Sound::Graze => synth(0.04, |t, _| (TAU * 3200. * t).sin() * (-t * 120.).exp() * 0.06),
        Sound::Bomb => chord(&[rumble(1.2, 4000., 60., 0.9, 11), glide(0.8, 900., 40., 0.12, sine)]),
        Sound::ExtraBomb => tune(&[(76., 0.06), (79., 0.06), (84., 0.06), (88., 0.16)], 0.12, square),
        // a phase broken: shattering glass and a chord
        Sound::PhaseBreak => chord(&[
            rumble(0.5, 6000., 1500., 0.6, 17),
            tune(&[(72., 0.08), (79., 0.08), (84., 0.3)], 0.12, tri),
        ]),
        Sound::BossDown => chord(&[rumble(2.2, 2000., 40., 1.0, 23), glide(1.4, 600., 50., 0.1, square), rumble(0.4, 8000., 2000., 0.3, 29)]),
        Sound::PlayerDown => chord(&[rumble(1.4, 1500., 50., 1.3, 31), tune(&[(64., 0.2), (60., 0.2), (55., 0.2), (48., 0.6)], 0.14, square)]),
        // the boss is coming: two siren sweeps
        Sound::Warning => {
            let mut v = glide(0.45, 400., 800., 0.1, square);
            v.extend(glide(0.45, 400., 800., 0.1, square));
            v
        }
        Sound::Win => chord(&[
            tune(&[(72., 0.15), (76., 0.15), (79., 0.15), (84., 0.15), (79., 0.15), (84., 0.7)], 0.14, square),
            tune(&[(48., 0.3), (55., 0.3), (60., 0.9)], 0.14, tri),
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounds_render_and_stay_in_range() {
        for s in ALL {
            let v = render(s);
            assert!(!v.is_empty(), "{s:?} is empty");
            let peak = v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(v.iter().all(|x| x.is_finite()), "{s:?} not finite");
            assert!(peak > 0.01 && peak <= 1.0, "{s:?} peak {peak}");
        }
    }
}
