//! The music: a small chiptune band playing tunes made up here, one for
//! the title and one per boss. Each tune is a tempo, a key, a mode, four
//! chords, a bass line, an arpeggio, drums and a lead phrase drawn from a
//! seed, so every boss has its own. `intensity` (rising as a fight goes
//! on) brings in the lead, faster hats and an octave on the arpeggio.

use crate::audio::{SR, midi, square, tri};
use rodio::Source;
use std::num::NonZero;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SRF: f32 = SR as f32;

#[derive(Default)]
pub struct Ctrl {
    want: i32,
    intensity: f32,
    muted: bool,
}

pub fn request(ctrl: &Mutex<Ctrl>, n: i32) {
    if let Ok(mut c) = ctrl.lock() {
        c.want = n;
    }
}

pub fn intensity(ctrl: &Mutex<Ctrl>, k: f32) {
    if let Ok(mut c) = ctrl.lock() {
        c.intensity = k.clamp(0., 1.);
    }
}

pub fn mute(ctrl: &Mutex<Ctrl>, m: bool) {
    if let Ok(mut c) = ctrl.lock() {
        c.muted = m;
    }
}

/// Scales as semitone steps.
const MINOR: [i32; 7] = [0, 2, 3, 5, 7, 8, 10];
const PHRYGIAN: [i32; 7] = [0, 1, 3, 5, 7, 8, 10];
const DORIAN: [i32; 7] = [0, 2, 3, 5, 7, 9, 10];
const HARMONIC: [i32; 7] = [0, 2, 3, 5, 7, 8, 11];
const MAJOR: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

struct Tune {
    bpm: f32,
    root: i32,
    scale: [i32; 7],
    chords: [i32; 4],
    bass: u16,
    kick: u16,
    snare: u16,
    /// The lead: a scale degree per 16th of four bars, or none.
    lead: [Option<i32>; 64],
    arp_up: bool,
}

/// Tune `n`: 0 the title, 1 to 10 the bosses.
fn tune(n: i32) -> Tune {
    let (bpm, root, scale, chords) = match n {
        0 => (112., 57, MINOR, [0, 5, 2, 6]),
        1 => (138., 50, MINOR, [0, 5, 3, 4]),
        2 => (128., 53, DORIAN, [0, 3, 0, 6]),
        3 => (120., 52, PHRYGIAN, [0, 1, 0, 6]),
        4 => (150., 55, HARMONIC, [0, 5, 4, 4]),
        5 => (140., 48, DORIAN, [0, 6, 5, 6]),
        6 => (132., 50, MAJOR, [5, 3, 0, 4]),
        7 => (156., 53, MINOR, [0, 6, 5, 4]),
        8 => (126., 57, MINOR, [5, 6, 0, 0]),
        9 => (144., 45, HARMONIC, [0, 3, 4, 4]),
        _ => (168., 47, PHRYGIAN, [0, 1, 5, 4]),
    };
    let mut seed = 0x9e37_79b9u32.wrapping_mul(n as u32 + 7);
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    let mut lead = [None; 64];
    // a one-bar motif, answered with a variation
    let motif: Vec<Option<i32>> = (0..16).map(|i| if i % 2 == 0 || rnd() % 3 == 0 { Some((rnd() % 7) as i32) } else { None }).collect();
    for bar in 0..4 {
        for i in 0..16 {
            lead[bar * 16 + i] = motif[i].map(|d| if bar % 2 == 1 && i >= 8 { d + (rnd() % 3) as i32 - 1 } else { d } + chords[bar]);
        }
    }
    Tune {
        bpm,
        root,
        scale,
        chords,
        bass: [0b1000_1000_1010_1000, 0b1010_1010_1010_1010, 0b1001_0010_1001_0010][(n as usize) % 3],
        kick: if n == 0 { 0b1000_0000_1000_0000 } else { 0b1000_0010_1000_0000 },
        snare: 0b0000_1000_0000_1000,
        lead,
        arp_up: n % 2 == 0,
    }
}

impl Tune {
    fn note(&self, degree: i32, octave: i32) -> f32 {
        let o = degree.div_euclid(7);
        let d = degree.rem_euclid(7) as usize;
        (self.root + self.scale[d] + 12 * (o + octave)) as f32
    }
}

#[derive(Clone, Copy, Default)]
struct Voice {
    freq: f32,
    ph: f32,
    amp: f32,
    decay: f32,
    wave: u8,
}

impl Voice {
    fn hit(&mut self, note: f32, amp: f32, decay: f32, wave: u8) {
        self.freq = midi(note);
        self.amp = amp;
        self.decay = decay;
        self.wave = wave;
    }
    fn next(&mut self) -> f32 {
        if self.amp < 1e-4 {
            return 0.;
        }
        self.ph += self.freq / SRF;
        if self.ph > 1. {
            self.ph -= 1.;
        }
        let x = match self.wave {
            0 => tri(self.ph),
            1 => square(self.ph),
            _ => {
                if self.ph < 0.25 { 1. } else { -1. }
            }
        };
        self.amp *= self.decay;
        x * self.amp
    }
}

pub struct Band {
    ctrl: Arc<Mutex<Ctrl>>,
    cur: i32,
    tune: Option<Tune>,
    sample: u64,
    step_len: u64,
    step: u64,
    bass: Voice,
    arp: Voice,
    lead: Voice,
    kick: f32,
    kick_ph: f32,
    snare: f32,
    hat: f32,
    noise: u32,
    fade: f32,
    poll: u32,
    intensity: f32,
    muted: bool,
}

impl Band {
    pub fn new(ctrl: Arc<Mutex<Ctrl>>) -> Band {
        Band {
            ctrl,
            cur: -1,
            tune: None,
            sample: 0,
            step_len: 1,
            step: 0,
            bass: Voice::default(),
            arp: Voice::default(),
            lead: Voice::default(),
            kick: 0.,
            kick_ph: 0.,
            snare: 0.,
            hat: 0.,
            noise: 0x2468_ace1,
            fade: 0.,
            poll: 0,
            intensity: 0.,
            muted: false,
        }
    }

    fn noise(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise as f32 / u32::MAX as f32) * 2. - 1.
    }

    fn on_step(&mut self) {
        let Some(t) = &self.tune else { return };
        let s = (self.step % 64) as usize;
        let i = s % 16;
        let bar = s / 16;
        let chord = t.chords[bar];
        let bit = |m: u16| m & (0x8000 >> i) != 0;
        if bit(t.bass) {
            let n = t.note(chord, -1);
            self.bass.hit(n, 0.5, 0.99985, 0);
        }
        let k = self.intensity;
        if i % 2 == 0 || k > 0.35 {
            let order = [0, 2, 4, 7];
            let j = if t.arp_up { i % 4 } else { 3 - i % 4 };
            let oct = if k > 0.7 && (i / 4) % 2 == 1 { 2 } else { 1 };
            let n = t.note(chord + order[j], oct);
            self.arp.hit(n, 0.16, 0.9993, 2);
        }
        if k > 0.15 {
            if let Some(d) = t.lead[s] {
                let n = t.note(d, 1);
                self.lead.hit(n, 0.15, 0.99975, 1);
            }
        }
        if bit(t.kick) || (k > 0.6 && i == 14) {
            self.kick = 1.;
            self.kick_ph = 0.;
        }
        if bit(t.snare) {
            self.snare = 0.6;
        }
        if i % 2 == 1 || k > 0.5 {
            self.hat = 0.25;
        }
    }
}

impl Iterator for Band {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.poll == 0 {
            self.poll = 512;
            if let Ok(c) = self.ctrl.lock() {
                if c.want != self.cur && self.fade <= 0.001 {
                    self.cur = c.want;
                    self.tune = (c.want >= 0).then(|| tune(c.want));
                    if let Some(t) = &self.tune {
                        self.step_len = (SRF * 60. / t.bpm / 4.) as u64;
                    }
                    self.sample = 0;
                    self.step = 0;
                }
                self.intensity += (c.intensity - self.intensity) * 0.05;
                self.muted = c.muted;
                // fade out before a change, in after
                let target = if c.want != self.cur || self.muted { 0. } else { 1. };
                self.fade += (target - self.fade) * 0.15;
                if self.fade < 0.002 {
                    self.fade = 0.;
                }
            }
        }
        self.poll -= 1;
        if self.tune.is_none() {
            return Some(0.);
        }
        if self.sample % self.step_len == 0 {
            self.on_step();
            self.step += 1;
        }
        self.sample += 1;
        let mut x = self.bass.next() * 0.9 + self.arp.next() + self.lead.next();
        if self.kick > 1e-3 {
            self.kick_ph += (45. + 120. * self.kick * self.kick) / SRF;
            x += (self.kick_ph * std::f32::consts::TAU).sin() * self.kick * 0.7;
            self.kick *= 0.9993;
        }
        if self.snare > 1e-3 {
            let n = self.noise();
            x += n * self.snare * 0.35;
            self.snare *= 0.9985;
        }
        if self.hat > 1e-3 {
            let n = self.noise();
            x += n * self.hat * 0.12;
            self.hat *= 0.996;
        }
        let gain = 0.22 * self.fade;
        Some((x * gain).tanh())
    }
}

impl Source for Band {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> rodio::ChannelCount {
        NonZero::new(1).unwrap()
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        NonZero::new(SR).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tune_plays_in_range() {
        for n in 0..=10 {
            let ctrl = Arc::new(Mutex::new(Ctrl::default()));
            request(&ctrl, n);
            intensity(&ctrl, 1.0);
            let mut band = Band::new(ctrl);
            let v: Vec<f32> = (&mut band).take(SR as usize * 3).collect();
            let peak = v.iter().fold(0f32, |m, x| m.max(x.abs()));
            assert!(v.iter().all(|x| x.is_finite()));
            assert!(peak > 0.02 && peak <= 1.0, "tune {n} peak {peak}");
        }
    }
}
