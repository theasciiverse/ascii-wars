//! The game: one `update` is one 60 Hz frame.
//!
//! Positions are the original's pixels, in its 1260x787.5 window with y
//! up, so its speeds, fire rates and hit sizes carry over as they are.
//!
//! Classic mode is the original, rule for rule (from its `game.rs`,
//! `player.rs`, `shoot.rs` and `enemies.rs`); the one change is how the
//! player is hit (see `Grid::sweep`). Arcade and Practice are ours: bosses
//! in phases that move and use lasers, rain, walls and homing shots, a
//! moment of safety after a hit, bombs, grazing and a score.

use crate::bosses::{self, Kind, Motion, Pattern, Rgb, Slot};
use std::f32::consts::{PI, TAU};
use std::time::Duration;

/// The original window: 1680x1050 at a resolution scale of 0.75.
pub const FIELD_W: f32 = 1680.0 * 0.75;
pub const FIELD_H: f32 = 1050.0 * 0.75;

pub const FPS: u32 = 60;
/// The original ran at the display's refresh with vsync; 60 Hz here.
pub const FRAME: Duration = Duration::from_nanos(1_000_000_000 / FPS as u64);
pub const DT: f32 = 1.0 / FPS as f32;

const PLAYER_SPEED: f32 = 250.0;
const FOCUS_SPEED: f32 = 110.0;
const PLAYER_BULLET_SPEED: f32 = 500.0;
const BULLET_LIFETIME: f32 = 10.0;
/// Every classic bullet hits the boss as a circle of this radius.
const BULLET_RADIUS: f32 = 8.0;
pub const MAX_LEVEL: i32 = 10;
const MAX_BOMBS: i32 = 5;
pub const GRAZES_PER_BOMB: u32 = 80;
const MAX_PARTICLES: usize = 900;
const ARCADE_FIRE_EVERY: u64 = 4;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }
    pub fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
    pub fn add_scaled(self, d: Vec2, s: f32) -> Vec2 {
        Vec2::new(self.x + d.x * s, self.y + d.y * s)
    }
    pub fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    /// Bevy's `normalize`; a zero vector gives NaNs, so callers that can
    /// meet one use `normalize_or`.
    pub fn normalize(self) -> Vec2 {
        let l = self.len();
        Vec2::new(self.x / l, self.y / l)
    }
    fn normalize_or(self, fallback: Vec2) -> Vec2 {
        let l = self.len();
        if l > 1e-6 { Vec2::new(self.x / l, self.y / l) } else { fallback }
    }
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }
    pub fn from_angle(a: f32) -> Vec2 {
        Vec2::new(a.cos(), a.sin())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Ten bosses in a row with one score.
    Arcade,
    /// One boss, from the boss select.
    Practice,
    /// The original game.
    Classic,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];

    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "Easy",
            Difficulty::Normal => "Normal",
            Difficulty::Hard => "Hard",
        }
    }
    fn speed(self) -> f32 {
        match self {
            Difficulty::Easy => 0.8,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.15,
        }
    }
    fn interval(self) -> f32 {
        match self {
            Difficulty::Easy => 1.3,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 0.85,
        }
    }
    fn iframes(self) -> f32 {
        match self {
            Difficulty::Easy => 2.0,
            Difficulty::Normal => 1.5,
            Difficulty::Hard => 1.0,
        }
    }
    fn score(self) -> f32 {
        match self {
            Difficulty::Easy => 0.5,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.5,
        }
    }
    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    /// Nothing running: the title and its menus.
    Title,
    /// The boss arrives.
    Intro,
    Playing,
    BossDying,
    PlayerDying,
    /// Arcade and Practice: the boss is down, the score is added up.
    Results,
    GameOver,
    /// Classic: "Completed!".
    FinishedLevel,
    WinGame,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sound {
    Shot,
    BossHit,
    Volley,
    LaserWarn,
    LaserFire,
    PlayerHit,
    Graze,
    Bomb,
    ExtraBomb,
    PhaseBreak,
    BossDown,
    PlayerDown,
    Warning,
    Win,
}

/// The original's `Pool`.
#[derive(Clone, Copy, Debug)]
pub struct Pool {
    pub health: f32,
    pub max_health: f32,
    pub damage: f32,
}

pub struct Player {
    pub pos: Vec2,
    /// Where the '>' points: always at the boss.
    pub facing: Vec2,
    pub pool: Pool,
    /// Seconds left of the blink after a hit, when nothing hurts.
    pub iframes: f32,
    pub focus: bool,
}

pub struct Boss {
    pub level: i32,
    pub name: &'static str,
    pub pos: Vec2,
    pub home: Vec2,
    pub glyph: char,
    pub art: &'static [&'static str],
    pub color: Rgb,
    pub pool: Pool,
    pub phase: usize,
    pub phases: usize,
    pub patterns: &'static [Pattern],
    motion: Motion,
    timers: Vec<f32>,
    /// Seconds into this phase.
    pub phase_t: f32,
    /// Seconds into the fight.
    pub fight_t: f32,
    /// Frames left of the white flash after a hit.
    pub flash: u8,
    /// Seconds left between phases, when it neither fires nor takes hits.
    pub invuln: f32,
    hop_to: Vec2,
    hop_t: f32,
}

#[derive(Clone, Debug)]
pub struct Bullet {
    pub pos: Vec2,
    /// Where it was a frame ago, so a fast bullet can't jump a cell.
    pub prev: Vec2,
    pub dir: Vec2,
    pub speed: f32,
    pub accel: f32,
    pub curve: f32,
    pub homing: f32,
    pub home_t: f32,
    pub bounce: u8,
    pub damage: f32,
    pub lifetime: f32,
    pub size: f32,
    pub glyph: char,
    pub color: Rgb,
    pub from_player: bool,
    pub grazed: bool,
}

impl Bullet {
    /// Drawn bold: the original's bullets at twice the size or more.
    pub fn bullet_big(&self) -> bool {
        self.size >= 2.0
    }
}

pub struct Laser {
    pub angle: f32,
    pub sweep: f32,
    pub warn: f32,
    pub beam: f32,
    pub t: f32,
    pub color: Rgb,
}

impl Laser {
    pub fn burning(&self) -> bool {
        self.t >= self.warn
    }
}

pub struct Particle {
    pub pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
    pub max: f32,
    pub glyph: char,
    pub color: Rgb,
}

pub struct Popup {
    pub pos: Vec2,
    pub text: String,
    pub life: f32,
    pub color: Rgb,
}

pub struct Star {
    pub pos: Vec2,
    pub layer: u8,
}

pub struct Banner {
    pub title: String,
    pub sub: String,
    pub t: f32,
    pub dur: f32,
    pub color: Rgb,
}

/// What a fight was worth, for the results panel.
#[derive(Clone, Debug, Default)]
pub struct Results {
    pub level: i32,
    pub time: f32,
    pub hits: u32,
    pub bombs_used: u32,
    pub time_bonus: u64,
    pub no_hit_bonus: u64,
    pub bomb_bonus: u64,
    pub score: u64,
}

/// A repeating Bevy `Timer`, as far as the original uses one.
#[derive(Clone, Copy, Debug, Default)]
pub struct Timer {
    duration: Duration,
    elapsed: Duration,
}

impl Timer {
    pub(crate) fn from_seconds(s: f32) -> Timer {
        Timer { duration: Duration::from_secs_f32(s), elapsed: Duration::ZERO }
    }

    /// True when it finished this tick; wraps like `TimerMode::Repeating`.
    pub(crate) fn tick(&mut self, delta: Duration) -> bool {
        self.elapsed += delta;
        if self.elapsed < self.duration {
            return false;
        }
        self.elapsed = if self.duration.is_zero() {
            Duration::ZERO
        } else {
            Duration::from_nanos((self.elapsed.as_nanos() % self.duration.as_nanos()) as u64)
        };
        true
    }
}

/// The terminal cells the field is drawn on, which decide hits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Grid {
    pub cols: u16,
    pub rows: u16,
}

impl Grid {
    /// The cell a point is drawn in; may lie outside the grid.
    pub fn cell(self, p: Vec2) -> (i32, i32) {
        let col = (p.x / FIELD_W * self.cols as f32).floor() as i32;
        let row = ((FIELD_H - p.y) / FIELD_H * self.rows as f32).floor() as i32;
        (col, row)
    }

    pub fn cell_px(self) -> (f32, f32) {
        (FIELD_W / self.cols as f32, FIELD_H / self.rows as f32)
    }

    pub fn contains(self, (c, r): (i32, i32)) -> bool {
        c >= 0 && r >= 0 && c < self.cols as i32 && r < self.rows as i32
    }

    /// Whether a straight move from `a` to `b` passes through a cell for
    /// which `f` holds. The player is hit by a bullet drawn in its cell or
    /// that passed through it since the last frame: what you see is what
    /// hits you. The original's test (an 8 pixel circle against a 2.6
    /// pixel box) is smaller than a cell is tall at 100x30, so a bullet
    /// could hit from the cell above or miss one drawn on the player; by
    /// area the two are about the same at 100x30.
    pub fn sweep(self, a: Vec2, b: Vec2, mut f: impl FnMut((i32, i32)) -> bool) -> bool {
        let (cw, ch) = self.cell_px();
        let d = b.sub(a);
        let steps = (d.x.abs() / (cw * 0.5)).max(d.y.abs() / (ch * 0.5)).ceil().max(1.0) as i32;
        (0..=steps).any(|i| f(self.cell(a.add_scaled(d, i as f32 / steps as f32))))
    }

    /// The cells a beam from `from` at `angle` burns, to the field's edge.
    pub fn beam(self, from: Vec2, angle: f32) -> Vec<(i32, i32)> {
        let (cw, ch) = self.cell_px();
        let step = cw.min(ch) * 0.5;
        let d = Vec2::from_angle(angle);
        let mut out: Vec<(i32, i32)> = Vec::new();
        let mut p = from;
        for _ in 0..4000 {
            let c = self.cell(p);
            if !self.contains(c) {
                break;
            }
            if out.last() != Some(&c) {
                out.push(c);
            }
            p = p.add_scaled(d, step);
        }
        out
    }
}

/// xorshift64*, seeded per run.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    /// 0 to 1.
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.unit()
    }
}

pub struct Game {
    pub mode: Mode,
    pub difficulty: Difficulty,
    pub state: State,
    pub paused: bool,
    pub level: i32,
    pub player: Option<Player>,
    pub boss: Option<Boss>,
    pub bullets: Vec<Bullet>,
    pub lasers: Vec<Laser>,
    pub particles: Vec<Particle>,
    pub popups: Vec<Popup>,
    pub stars: Vec<Star>,
    pub banner: Option<Banner>,
    /// The bomb's shock wave: centre, radius, and whether it has hit the
    /// boss yet.
    pub wave: Option<(Vec2, f32, bool)>,
    /// Frames of screen shake and of the red flash left.
    pub shake: u8,
    pub flash: u8,
    pub score: u64,
    pub graze: u32,
    pub bombs: i32,
    pub continues: u32,
    /// Seconds into the intro, a dying boss or a dying player.
    pub state_t: f32,
    pub results: Results,
    hits_this_fight: u32,
    hits_this_phase: u32,
    bombs_this_fight: u32,
    pub grazes_to_bomb: u32,
    pub events: Vec<Sound>,
    pub rng: Rng,
    /// Classic: one timer per `Slot`, Bevy resources that carry over from
    /// level to level and game to game, as in the original.
    timers: [Timer; 6],
    /// Bevy's virtual time, which runs in the menus too; classic rings
    /// turn with it.
    pub clock: Duration,
    pub frames: u64,
    /// What the health bar last showed; it stays up on the menus after
    /// the player is gone, like the original's HUD.
    pub hud_health: f32,
}

impl Default for Game {
    fn default() -> Self {
        Game::new(1)
    }
}

impl Game {
    pub fn new(seed: u64) -> Game {
        let mut rng = Rng::new(seed);
        let stars = (0..90)
            .map(|i| Star { pos: Vec2::new(rng.range(0., FIELD_W), rng.range(0., FIELD_H)), layer: (i % 3) as u8 })
            .collect();
        Game {
            mode: Mode::Arcade,
            difficulty: Difficulty::Normal,
            state: State::Title,
            paused: false,
            level: 0,
            player: None,
            boss: None,
            bullets: Vec::new(),
            lasers: Vec::new(),
            particles: Vec::new(),
            popups: Vec::new(),
            stars,
            banner: None,
            wave: None,
            shake: 0,
            flash: 0,
            score: 0,
            graze: 0,
            bombs: 0,
            continues: 0,
            state_t: 0.,
            results: Results::default(),
            hits_this_fight: 0,
            hits_this_phase: 0,
            bombs_this_fight: 0,
            grazes_to_bomb: 0,
            events: Vec::new(),
            rng,
            timers: [Timer::default(); 6],
            clock: Duration::ZERO,
            frames: 0,
            hud_health: 1.0,
        }
    }

    pub fn classic(&self) -> bool {
        self.mode == Mode::Classic
    }

    /// Start a game in `mode` at boss `start` (1 to 10).
    pub fn start(&mut self, mode: Mode, difficulty: Difficulty, start: i32) {
        self.mode = mode;
        self.difficulty = difficulty;
        self.score = 0;
        self.graze = 0;
        self.continues = 0;
        self.grazes_to_bomb = 0;
        self.bombs = if mode == Mode::Classic { 0 } else { 3 };
        self.clean_up_level();
        self.level = start.clamp(1, MAX_LEVEL) - 1;
        self.prepare_level();
    }

    /// Classic's Play and Play Again.
    pub fn new_game(&mut self, start: i32) {
        self.start(Mode::Classic, Difficulty::Normal, start);
    }

    /// Continue to the next boss, or to the end.
    pub fn next_level(&mut self) {
        let carry = self.player.as_ref().map(|p| p.pool.health);
        self.clean_up_level();
        self.prepare_level();
        // Arcade: health carries over, with a little back
        if self.mode == Mode::Arcade {
            if let (Some(p), Some(h)) = (&mut self.player, carry) {
                p.pool.health = (h + 4.).min(p.pool.max_health);
                self.hud_health = p.pool.health / p.pool.max_health;
            }
            self.bombs = self.bombs.max(2);
        }
    }

    /// Arcade and Practice: try this boss again, from no score.
    pub fn retry(&mut self) {
        self.continues += 1;
        self.score = 0;
        self.bombs = 3;
        self.clean_up_level();
        self.level -= 1;
        self.prepare_level();
    }

    pub fn to_title(&mut self) {
        self.clean_up_level();
        self.state = State::Title;
    }

    fn clean_up_level(&mut self) {
        self.player = None;
        self.boss = None;
        self.bullets.clear();
        self.lasers.clear();
        self.particles.clear();
        self.popups.clear();
        self.banner = None;
        self.wave = None;
        self.paused = false;
    }

    fn prepare_level(&mut self) {
        if self.level >= MAX_LEVEL {
            self.state = State::WinGame;
            self.events.push(Sound::Win);
            return;
        }
        self.level += 1;
        let cx = FIELD_W * 0.5;
        let cy = FIELD_H * 0.5;
        let pool = |health| Pool { health, max_health: health, damage: 2. };
        if self.classic() {
            let (name, glyph, patterns) = bosses::classic_def(self.level);
            self.player = Some(Player { pos: Vec2::new(cx, cy - 300.), facing: Vec2::new(0., 1.), pool: pool(10.), iframes: 0., focus: false });
            self.boss = Some(Boss {
                level: self.level,
                name,
                pos: Vec2::new(cx, cy),
                home: Vec2::new(cx, cy),
                glyph,
                art: &[],
                color: (235, 235, 235),
                pool: pool(1500.),
                phase: 0,
                phases: 1,
                patterns,
                motion: Motion::Still,
                timers: Vec::new(),
                phase_t: 0.,
                fight_t: 0.,
                flash: 0,
                invuln: 0.,
                hop_to: Vec2::new(cx, cy),
                hop_t: 0.,
            });
            self.hud_health = 1.0;
            self.state = State::Playing;
            log::info!("classic level {} {}", self.level, name);
            return;
        }
        let def = bosses::arcade(self.level);
        let home = Vec2::new(cx, FIELD_H * 0.72);
        let first = &def.phases[0];
        self.player = Some(Player { pos: Vec2::new(cx, FIELD_H * 0.14), facing: Vec2::new(0., 1.), pool: pool(10.), iframes: 0., focus: false });
        self.boss = Some(Boss {
            level: self.level,
            name: def.name,
            pos: Vec2::new(cx, FIELD_H + 90.),
            home,
            glyph: def.glyph,
            art: def.art,
            color: def.color,
            pool: pool(first.hp),
            phase: 0,
            phases: def.phases.len(),
            patterns: first.patterns,
            motion: first.motion,
            timers: first.patterns.iter().map(|p| p.delay).collect(),
            phase_t: 0.,
            fight_t: 0.,
            flash: 0,
            invuln: 0.,
            hop_to: home,
            hop_t: 0.,
        });
        self.hud_health = 1.0;
        self.hits_this_fight = 0;
        self.hits_this_phase = 0;
        self.bombs_this_fight = 0;
        self.state = State::Intro;
        self.state_t = 0.;
        self.banner = Some(Banner {
            title: format!("WARNING  {}  WARNING", def.name.to_uppercase()),
            sub: def.epithet.to_string(),
            t: 0.,
            dur: 2.4,
            color: def.color,
        });
        self.events.push(Sound::Warning);
        log::info!("level {} {} ({:?}, {:?})", self.level, def.name, self.mode, self.difficulty);
    }

    /// Bomb: clears the boss's shots round the player (Arcade and Practice).
    pub fn bomb(&mut self) {
        if self.classic() || self.state != State::Playing || self.paused || self.bombs <= 0 || self.wave.is_some() {
            return;
        }
        let Some(p) = &mut self.player else { return };
        self.bombs -= 1;
        self.bombs_this_fight += 1;
        p.iframes = p.iframes.max(2.0);
        self.wave = Some((p.pos, 0., false));
        self.lasers.clear();
        self.shake = self.shake.max(10);
        self.events.push(Sound::Bomb);
        log::info!("bomb, {} left", self.bombs);
    }

    fn add_score(&mut self, points: f32) {
        self.score += (points * self.difficulty.score()).round() as u64;
    }

    /// One frame. `mv` is the held direction (each axis -1, 0 or 1).
    pub fn update(&mut self, mv: (i8, i8), focus: bool, grid: Grid) {
        self.frames += 1;
        self.clock += FRAME;
        if self.paused {
            return;
        }
        self.update_fx();
        if self.classic() {
            if self.state == State::Playing {
                self.update_classic(mv, grid);
            }
            return;
        }
        match self.state {
            State::Intro => {
                self.state_t += DT;
                self.move_player(mv, focus, false);
                if let Some(b) = &mut self.boss {
                    let k = (self.state_t / 1.8).min(1.0);
                    let ease = 1.0 - (1.0 - k).powi(3);
                    b.pos = Vec2::new(b.home.x, FIELD_H + 90. + (b.home.y - FIELD_H - 90.) * ease);
                }
                if self.state_t >= 2.4 {
                    self.state = State::Playing;
                    self.phase_banner();
                }
            }
            State::Playing => self.update_arcade(mv, focus, grid),
            State::BossDying => {
                self.state_t += DT;
                self.move_player(mv, focus, false);
                if let Some(b) = &self.boss {
                    if self.frames % 5 == 0 && self.state_t < 1.6 {
                        let at = b.pos.add_scaled(Vec2::from_angle(self.rng.range(0., TAU)), self.rng.range(0., 50.));
                        let c = b.color;
                        self.explode(at, 14, c, '*');
                        self.shake = self.shake.max(4);
                        self.events.push(Sound::BossHit);
                    }
                }
                if self.state_t >= 1.6 && self.boss.is_some() {
                    let b = self.boss.take().unwrap();
                    self.explode(b.pos, 70, b.color, b.glyph);
                    self.explode(b.pos, 40, (255, 255, 255), '.');
                    self.shake = 18;
                    self.events.push(Sound::BossDown);
                }
                if self.state_t >= 3.0 {
                    self.state = State::Results;
                }
            }
            State::PlayerDying => {
                self.state_t += DT;
                self.update_bullets();
                if self.state_t >= 2.0 {
                    self.state = State::GameOver;
                    self.lasers.clear();
                }
            }
            _ => {}
        }
    }

    fn update_fx(&mut self) {
        let fly = if self.state == State::Title { 1.0 } else { 0.6 };
        for s in &mut self.stars {
            s.pos.y -= [12., 30., 60.][s.layer as usize] * fly * DT;
            if s.pos.y < 0. {
                s.pos.y += FIELD_H;
                s.pos.x = (s.pos.x * 7.31 + 97.) % FIELD_W;
            }
        }
        for p in &mut self.particles {
            p.pos = p.pos.add_scaled(p.vel, DT);
            p.vel = Vec2::new(p.vel.x * 0.95, p.vel.y * 0.95);
            p.life -= DT;
        }
        self.particles.retain(|p| p.life > 0.);
        for p in &mut self.popups {
            p.pos.y += 40. * DT;
            p.life -= DT;
        }
        self.popups.retain(|p| p.life > 0.);
        if let Some(b) = &mut self.banner {
            b.t += DT;
            if b.t >= b.dur {
                self.banner = None;
            }
        }
        self.shake = self.shake.saturating_sub(1);
        self.flash = self.flash.saturating_sub(1);
    }

    fn explode(&mut self, at: Vec2, n: usize, color: Rgb, glyph: char) {
        for _ in 0..n {
            if self.particles.len() >= MAX_PARTICLES {
                break;
            }
            let a = self.rng.range(0., TAU);
            let s = self.rng.range(60., 420.);
            let life = self.rng.range(0.4, 1.3);
            let v = Vec2::from_angle(a);
            self.particles.push(Particle { pos: at, vel: Vec2::new(v.x * s, v.y * s), life, max: life, glyph, color });
        }
    }

    fn spark(&mut self, at: Vec2, color: Rgb) {
        if self.particles.len() >= MAX_PARTICLES {
            return;
        }
        let a = self.rng.range(0., TAU);
        let s = self.rng.range(80., 220.);
        let glyph = ['\'', '.', ',', '`'][(self.rng.next() % 4) as usize];
        let v = Vec2::from_angle(a);
        self.particles.push(Particle { pos: at, vel: Vec2::new(v.x * s, v.y * s), life: 0.3, max: 0.3, glyph, color });
    }

    fn popup(&mut self, at: Vec2, text: String, color: Rgb) {
        self.popups.push(Popup { pos: at, text, life: 1.4, color });
    }

    fn phase_banner(&mut self) {
        let Some(b) = &self.boss else { return };
        let def = bosses::arcade(b.level);
        let ph = &def.phases[b.phase];
        let sub = if b.phases > 1 { format!("phase {} of {}", b.phase + 1, b.phases) } else { String::new() };
        self.banner = Some(Banner { title: format!("<< {} >>", ph.name), sub, t: 0., dur: 2.2, color: def.color });
    }

    fn move_player(&mut self, mv: (i8, i8), focus: bool, can_fire: bool) {
        let classic = self.classic();
        let Some(player) = &mut self.player else { return };
        player.focus = focus;
        player.iframes = (player.iframes - DT).max(0.);
        let moving = mv != (0, 0);
        if moving {
            let dir = Vec2::new(mv.0 as f32, mv.1 as f32).normalize();
            let speed = if focus && !classic { FOCUS_SPEED } else { PLAYER_SPEED };
            player.pos = player.pos.add_scaled(dir, speed * DT);
            let half = 8.0 * 0.5;
            player.pos.x = player.pos.x.clamp(half, FIELD_W - half);
            player.pos.y = player.pos.y.clamp(half, FIELD_H - half);
        }
        if let Some(boss) = &self.boss {
            player.facing = boss.pos.sub(player.pos).normalize_or(Vec2::new(0., 1.));
        }
        // the original fires every frame for 2; Arcade fires every fourth
        // frame for 8, the same damage a second without a solid line of
        // shots across the screen
        if moving && can_fire && self.frames % ARCADE_FIRE_EVERY == 0 {
            let damage = player.pool.damage * ARCADE_FIRE_EVERY as f32;
            let b = bullet(player.pos, player.facing, PLAYER_BULLET_SPEED, damage, 1.0, '*', (255, 235, 140), true);
            self.bullets.push(b);
            if self.frames % (ARCADE_FIRE_EVERY * 2) == 0 {
                self.events.push(Sound::Shot);
            }
        }
    }

    /// The original, frame by frame.
    fn update_classic(&mut self, mv: (i8, i8), grid: Grid) {
        let (Some(player), Some(boss)) = (&mut self.player, &self.boss) else { return };
        let mut spawned = Vec::new();

        // input.rs and player.rs: move, stay in the window, face the boss,
        // and fire at the boss on every frame the player moves
        let moving = mv != (0, 0);
        if moving {
            let dir = Vec2::new(mv.0 as f32, mv.1 as f32).normalize();
            player.pos = player.pos.add_scaled(dir, PLAYER_SPEED * DT);
            let half = 8.0 * 0.5;
            player.pos.x = player.pos.x.clamp(half, FIELD_W - half);
            player.pos.y = player.pos.y.clamp(half, FIELD_H - half);
        }
        player.facing = boss.pos.sub(player.pos).normalize();
        if moving {
            spawned.push(bullet(player.pos, player.facing, PLAYER_BULLET_SPEED, player.pool.damage, 1.0, '*', (235, 235, 235), true));
        }

        // enemies.rs: one timer per pattern system, ticked every frame
        // whether or not this boss has that pattern
        let t = self.clock.as_secs_f32();
        for slot in Slot::ALL {
            let timer = &mut self.timers[slot.index()];
            if !timer.tick(FRAME) {
                continue;
            }
            let Some(p) = boss.patterns.iter().find(|p| p.slot == Some(slot)) else { continue };
            *timer = Timer::from_seconds(p.rate);
            for i in 0..p.count {
                let dir = if slot == Slot::Direct {
                    player.pos.sub(boss.pos).normalize()
                } else {
                    Vec2::from_angle(i as f32 / p.count as f32 * TAU + t * p.rotation)
                };
                spawned.push(bullet(boss.pos, dir, p.speed, boss.pool.damage, p.size, p.glyph, p.color, false));
            }
        }

        // shoot.rs: bullets fly for ten seconds
        for b in &mut self.bullets {
            b.prev = b.pos;
            b.pos = b.pos.add_scaled(b.dir, b.speed * DT);
            b.lifetime -= DT;
        }
        self.bullets.retain(|b| b.lifetime > 0.);

        // shoot.rs: hits; the state changes once the frame is done, as
        // Bevy's NextState does
        let mut next = None;
        let (player, boss) = (self.player.as_mut().unwrap(), self.boss.as_mut().unwrap());
        self.bullets.retain(|b| {
            let (pool, hit) = if b.from_player {
                (&mut boss.pool, boss_hit_classic(b, boss.pos))
            } else {
                (&mut player.pool, grid.sweep(b.prev, b.pos, |c| c == grid.cell(player.pos)))
            };
            if !hit {
                return true;
            }
            pool.health -= b.damage;
            if pool.health < 0. {
                next = Some(if b.from_player { State::FinishedLevel } else { State::GameOver });
            }
            false
        });
        self.hud_health = (player.pool.health / player.pool.max_health).clamp(0., 1.);
        self.bullets.extend(spawned);

        match next {
            Some(State::GameOver) => {
                log::info!("game over on level {}", self.level);
                self.clean_up_level();
                self.state = State::GameOver;
            }
            Some(_) if self.level >= MAX_LEVEL => {
                log::info!("last boss down");
                self.next_level();
            }
            Some(s) => {
                log::info!("level {} done", self.level);
                self.state = s;
            }
            None => {}
        }
    }

    fn update_arcade(&mut self, mv: (i8, i8), focus: bool, grid: Grid) {
        self.move_player(mv, focus, true);
        self.move_boss();
        self.fire_patterns();
        self.update_bullets();
        self.update_wave();
        if self.state != State::Playing {
            return;
        }
        self.hit_boss(grid);
        if self.state != State::Playing {
            return;
        }
        self.hit_player(grid);
        if let Some(p) = &self.player {
            self.hud_health = (p.pool.health / p.pool.max_health).clamp(0., 1.);
        }
    }

    fn move_boss(&mut self) {
        let player_x = self.player.as_ref().map_or(FIELD_W / 2., |p| p.pos.x);
        let Some(b) = &mut self.boss else { return };
        b.phase_t += DT;
        b.fight_t += DT;
        b.flash = b.flash.saturating_sub(1);
        b.invuln = (b.invuln - DT).max(0.);
        let t = b.phase_t;
        let k = (4.0 * DT).min(1.0);
        let target = match b.motion {
            Motion::Still => b.home,
            Motion::Sway { ax, ay, fx, fy } => Vec2::new(b.home.x + ax * (TAU * fx * t).sin(), b.home.y + ay * (TAU * fy * t).sin()),
            Motion::Hop { every } => {
                b.hop_t -= DT;
                if b.hop_t <= 0. {
                    b.hop_t = every;
                    b.hop_to = Vec2::new(self.rng.range(170., FIELD_W - 170.), self.rng.range(FIELD_H * 0.55, FIELD_H * 0.85));
                }
                b.hop_to
            }
            // the ease below moves 4 times the gap a second, so a gap of a
            // quarter of `speed` moves at `speed`
            Motion::Chase { speed } => {
                let dx = (player_x - b.pos.x).clamp(-speed / 4., speed / 4.);
                Vec2::new(b.pos.x + dx, b.home.y + 25. * (TAU * 0.2 * t).sin())
            }
        };
        b.pos = Vec2::new(b.pos.x + (target.x - b.pos.x) * k, b.pos.y + (target.y - b.pos.y) * k);
        b.pos.x = b.pos.x.clamp(90., FIELD_W - 90.);
        b.pos.y = b.pos.y.clamp(FIELD_H * 0.35, FIELD_H - 60.);
    }

    fn fire_patterns(&mut self) {
        let Some(b) = &mut self.boss else { return };
        if b.invuln > 0. {
            return;
        }
        let Some(player) = &self.player else { return };
        let (speed_k, rate_k) = (self.difficulty.speed(), self.difficulty.interval());
        let aim = player.pos.sub(b.pos).angle();
        let mut fired = false;
        for (i, p) in b.patterns.iter().enumerate() {
            b.timers[i] -= DT;
            if b.timers[i] > 0. {
                continue;
            }
            b.timers[i] = (b.timers[i] + p.rate * rate_k).max(0.0001);
            fired = true;
            let speed = p.speed * speed_k;
            let shot = |pos: Vec2, a: f32, bullets: &mut Vec<Bullet>| {
                let mut s = bullet(pos, Vec2::from_angle(a), speed, 2., p.size, p.glyph, p.color, false);
                s.accel = p.accel;
                s.curve = p.curve;
                s.homing = p.homing;
                s.home_t = if p.homing > 0. { 1.5 } else { 0. };
                s.bounce = p.bounce;
                bullets.push(s);
            };
            match p.kind {
                Kind::Ring => {
                    let base = b.phase_t * p.rotation;
                    for k in 0..p.count {
                        shot(b.pos, base + k as f32 / p.count as f32 * TAU, &mut self.bullets);
                    }
                }
                Kind::Burst => {
                    let base = self.rng.range(0., TAU);
                    for k in 0..p.count {
                        shot(b.pos, base + k as f32 / p.count as f32 * TAU, &mut self.bullets);
                    }
                }
                Kind::Aimed => {
                    for _ in 0..p.count {
                        shot(b.pos, aim, &mut self.bullets);
                    }
                }
                Kind::Fan { spread } => {
                    for k in 0..p.count {
                        let off = if p.count > 1 { -spread / 2. + spread * k as f32 / (p.count - 1) as f32 } else { 0. };
                        shot(b.pos, aim + off, &mut self.bullets);
                    }
                }
                Kind::Rain => {
                    for _ in 0..p.count {
                        let pos = Vec2::new(self.rng.range(10., FIELD_W - 10.), FIELD_H + 8.);
                        let a = -PI / 2. + self.rng.range(-0.15, 0.15);
                        shot(pos, a, &mut self.bullets);
                    }
                }
                Kind::Wall { gap } => {
                    let spacing = 28.;
                    let n = (FIELD_W / spacing) as i32;
                    let g = (self.rng.unit() * (n - gap - 2) as f32) as i32 + 1;
                    for k in 0..n {
                        if k >= g && k < g + gap {
                            continue;
                        }
                        shot(Vec2::new(spacing * (k as f32 + 0.5), FIELD_H + 8.), -PI / 2., &mut self.bullets);
                    }
                }
                Kind::Laser { spread, warn, beam, sweep, aimed } => {
                    let base = if aimed { aim } else { -PI / 2. };
                    for k in 0..p.count {
                        let off = (k as f32 - (p.count - 1) as f32 / 2.) * spread;
                        self.lasers.push(Laser { angle: base + off, sweep, warn: warn * rate_k.min(1.15), beam, t: 0., color: p.color });
                    }
                    self.events.push(Sound::LaserWarn);
                }
            }
        }
        if fired && self.frames % 4 == 0 {
            self.events.push(Sound::Volley);
        }
    }

    fn update_bullets(&mut self) {
        let target = self.player.as_ref().map(|p| p.pos);
        for b in &mut self.bullets {
            b.prev = b.pos;
            if b.home_t > 0. {
                b.home_t -= DT;
                if let Some(t) = target {
                    let want = t.sub(b.pos).angle();
                    let have = b.dir.angle();
                    let mut d = want - have;
                    while d > PI {
                        d -= TAU;
                    }
                    while d < -PI {
                        d += TAU;
                    }
                    b.dir = Vec2::from_angle(have + d.clamp(-b.homing * DT, b.homing * DT));
                }
            }
            if b.curve != 0. {
                b.dir = Vec2::from_angle(b.dir.angle() + b.curve * DT);
            }
            if b.accel != 0. {
                b.speed = (b.speed + b.accel * DT).clamp(30., 600.);
            }
            b.pos = b.pos.add_scaled(b.dir, b.speed * DT);
            if b.bounce > 0 {
                if b.pos.x < 0. || b.pos.x > FIELD_W {
                    b.dir.x = -b.dir.x;
                    b.pos.x = b.pos.x.clamp(0., FIELD_W);
                    b.bounce -= 1;
                } else if b.pos.y < 0. || (b.pos.y > FIELD_H && b.dir.y > 0.) {
                    b.dir.y = -b.dir.y;
                    b.pos.y = b.pos.y.clamp(0., FIELD_H);
                    b.bounce -= 1;
                }
            }
            b.lifetime -= DT;
        }
        let m = 40.;
        self.bullets.retain(|b| b.lifetime > 0. && b.pos.x > -m && b.pos.x < FIELD_W + m && b.pos.y > -m && b.pos.y < FIELD_H + m + 20.);

        let mut fire = false;
        for l in &mut self.lasers {
            let was = l.burning();
            l.t += DT;
            l.angle += l.sweep * DT;
            if !was && l.burning() {
                fire = true;
            }
        }
        if fire {
            self.events.push(Sound::LaserFire);
            self.shake = self.shake.max(3);
        }
        self.lasers.retain(|l| l.t < l.warn + l.beam);
    }

    fn update_wave(&mut self) {
        let Some((at, r, hit_boss)) = self.wave else { return };
        let r = r + 900. * DT;
        let mut cancelled = 0;
        let mut sparks = Vec::new();
        self.bullets.retain(|b| {
            if !b.from_player && b.pos.sub(at).len() < r {
                cancelled += 1;
                if cancelled % 3 == 0 {
                    sparks.push(b.pos);
                }
                false
            } else {
                true
            }
        });
        for s in sparks {
            self.spark(s, (180, 240, 255));
        }
        self.add_score(5. * cancelled as f32);
        let mut hit = hit_boss;
        if !hit {
            if let Some(b) = &mut self.boss {
                if b.pos.sub(at).len() < r && b.invuln <= 0. {
                    b.pool.health -= 50.;
                    b.flash = 8;
                    hit = true;
                }
            }
        }
        self.wave = if r < 760. { Some((at, r, hit)) } else { None };
        if self.boss.as_ref().is_some_and(|b| b.pool.health <= 0.) {
            self.boss_phase_down();
        }
    }

    /// The boss's art in cells: left, top, width, height.
    pub fn boss_rect(&self, grid: Grid) -> Option<(i32, i32, i32, i32)> {
        let b = self.boss.as_ref()?;
        let (c, r) = grid.cell(b.pos);
        if b.art.is_empty() {
            return Some((c, r, 1, 1));
        }
        let h = b.art.len() as i32;
        let w = b.art.iter().map(|l| l.chars().count()).max().unwrap_or(1) as i32;
        Some((c - w / 2, r - h / 2, w, h))
    }

    fn hit_boss(&mut self, grid: Grid) {
        let Some((x, y, w, h)) = self.boss_rect(grid) else { return };
        let inside = |c: (i32, i32)| c.0 >= x && c.0 < x + w && c.1 >= y && c.1 < y + h;
        let invuln = self.boss.as_ref().is_some_and(|b| b.invuln > 0.);
        let mut hits = 0;
        let mut damage = 0.;
        let mut spark_at = Vec::new();
        self.bullets.retain(|b| {
            if !b.from_player || !grid.sweep(b.prev, b.pos, inside) {
                return true;
            }
            if !invuln {
                hits += 1;
                damage += b.damage;
                spark_at.push(b.pos);
            }
            false
        });
        if hits == 0 {
            return;
        }
        let color = self.boss.as_ref().map_or((255, 255, 255), |b| b.color);
        for s in spark_at {
            self.spark(s, color);
        }
        let boss = self.boss.as_mut().unwrap();
        boss.pool.health -= damage;
        boss.flash = 2;
        self.add_score(5. * damage);
        self.events.push(Sound::BossHit);
        if self.boss.as_ref().unwrap().pool.health <= 0. {
            self.boss_phase_down();
        }
    }

    fn boss_phase_down(&mut self) {
        let Some(b) = &mut self.boss else { return };
        if self.state != State::Playing {
            return;
        }
        let at = b.pos;
        let color = b.color;
        let phase_no = b.phase as f32 + 1.;
        let clean = self.hits_this_phase == 0;
        // every shot on screen turns into points
        let cancelled = self.bullets.iter().filter(|x| !x.from_player).count();
        let gone: Vec<Vec2> = self.bullets.iter().filter(|x| !x.from_player).map(|x| x.pos).step_by(2).collect();
        self.bullets.retain(|x| x.from_player);
        self.lasers.clear();
        for p in gone {
            self.spark(p, (255, 255, 255));
        }
        let bonus = 1000. * phase_no + if clean { 3000. } else { 0. } + 10. * cancelled as f32;
        self.add_score(bonus);
        let shown = (bonus * self.difficulty.score()).round();
        self.popup(at.add_scaled(Vec2::new(0., 1.), 50.), format!("+{shown}"), (255, 255, 160));
        self.hits_this_phase = 0;
        let b = self.boss.as_mut().unwrap();
        if b.phase + 1 < b.phases {
            b.phase += 1;
            let def = bosses::arcade(b.level);
            let ph = &def.phases[b.phase];
            b.pool = Pool { health: ph.hp, max_health: ph.hp, damage: 2. };
            b.patterns = ph.patterns;
            b.motion = ph.motion;
            b.timers = ph.patterns.iter().map(|p| p.delay).collect();
            b.phase_t = 0.;
            b.invuln = 1.6;
            b.flash = 12;
            b.hop_t = 0.;
            log::info!("phase {} of boss {}", b.phase + 1, b.level);
            self.explode(at, 30, color, '*');
            self.shake = self.shake.max(8);
            self.events.push(Sound::PhaseBreak);
            self.phase_banner();
        } else {
            b.pool.health = 0.;
            let time = b.fight_t;
            self.state = State::BossDying;
            self.state_t = 0.;
            self.wave = None;
            let mult = self.difficulty.score();
            let level = self.level;
            let time_bonus = ((90. - time).max(0.) * 200.) * mult;
            let no_hit = if self.hits_this_fight == 0 { 20000. * mult } else { 0. };
            let bomb_bonus = if self.bombs_this_fight == 0 { 5000. * mult } else { 0. };
            self.score += (time_bonus + no_hit + bomb_bonus).round() as u64;
            self.results = Results {
                level,
                time,
                hits: self.hits_this_fight,
                bombs_used: self.bombs_this_fight,
                time_bonus: time_bonus.round() as u64,
                no_hit_bonus: no_hit.round() as u64,
                bomb_bonus: bomb_bonus.round() as u64,
                score: self.score,
            };
            self.events.push(Sound::PhaseBreak);
            log::info!("boss {} down in {:.1}s, score {}", level, time, self.score);
        }
    }

    fn hit_player(&mut self, grid: Grid) {
        let Some(p) = &self.player else { return };
        let me = grid.cell(p.pos);
        let safe = p.iframes > 0.;
        let mut hurt = false;
        let mut grazes = 0;
        for b in &mut self.bullets {
            if b.from_player {
                continue;
            }
            let hit = grid.sweep(b.prev, b.pos, |c| c == me);
            if hit && !safe {
                hurt = true;
                b.lifetime = 0.;
            } else if !hit && !b.grazed {
                let c = grid.cell(b.pos);
                if (c.0 - me.0).abs() <= 1 && (c.1 - me.1).abs() <= 1 {
                    b.grazed = true;
                    grazes += 1;
                }
            }
        }
        self.bullets.retain(|b| b.lifetime > 0.);
        if !safe {
            if let Some(b) = &self.boss {
                let burning = self.lasers.iter().filter(|l| l.burning()).any(|l| grid.beam(b.pos, l.angle).contains(&me));
                let (x, y, w, h) = self.boss_rect(grid).unwrap();
                let touching = me.0 >= x && me.0 < x + w && me.1 >= y && me.1 < y + h;
                hurt |= burning || touching;
            }
        }
        if grazes > 0 {
            self.graze += grazes;
            self.grazes_to_bomb += grazes;
            self.add_score(50. * grazes as f32);
            let at = self.player.as_ref().unwrap().pos;
            self.spark(at, (255, 255, 255));
            if self.frames % 4 == 0 {
                self.events.push(Sound::Graze);
            }
            if self.grazes_to_bomb >= GRAZES_PER_BOMB {
                self.grazes_to_bomb -= GRAZES_PER_BOMB;
                if self.bombs < MAX_BOMBS {
                    self.bombs += 1;
                    self.popup(at, "BOMB +1".into(), (120, 220, 255));
                    self.events.push(Sound::ExtraBomb);
                }
            }
        }
        if hurt {
            self.hurt_player();
        }
    }

    fn hurt_player(&mut self) {
        let iframes = self.difficulty.iframes();
        let Some(p) = &mut self.player else { return };
        p.pool.health -= 2.;
        p.iframes = iframes;
        let at = p.pos;
        let dead = p.pool.health < 0.;
        self.hits_this_fight += 1;
        self.hits_this_phase += 1;
        self.shake = self.shake.max(10);
        self.flash = 6;
        // clear the shots right round the player, so one hit stays one hit
        self.bullets.retain(|b| b.from_player || b.pos.sub(at).len() > 100.);
        self.explode(at, 16, (255, 90, 90), '+');
        log::info!("player hit, health {}", self.player.as_ref().unwrap().pool.health);
        if dead {
            self.hud_health = 0.;
            let p = self.player.take().unwrap();
            self.explode(p.pos, 60, (120, 230, 255), '>');
            self.explode(p.pos, 30, (255, 255, 255), '*');
            self.shake = 20;
            self.state = State::PlayerDying;
            self.state_t = 0.;
            self.wave = None;
            self.events.push(Sound::PlayerDown);
        } else {
            self.events.push(Sound::PlayerHit);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn bullet(pos: Vec2, dir: Vec2, speed: f32, damage: f32, size: f32, glyph: char, color: Rgb, from_player: bool) -> Bullet {
    Bullet {
        pos,
        prev: pos,
        dir,
        speed,
        accel: 0.,
        curve: 0.,
        homing: 0.,
        home_t: 0.,
        bounce: 0,
        damage,
        lifetime: BULLET_LIFETIME,
        size,
        glyph,
        color,
        from_player,
        grazed: false,
    }
}

/// The original test: the bullet's circle against the boss's box, whose
/// half size is the sprite's scale (2x2 pixels).
fn boss_hit_classic(b: &Bullet, centre: Vec2) -> bool {
    let half = 2.0;
    let cx = b.pos.x.clamp(centre.x - half, centre.x + half);
    let cy = b.pos.y.clamp(centre.y - half, centre.y + half);
    let (dx, dy) = (b.pos.x - cx, b.pos.y - cy);
    dx * dx + dy * dy <= BULLET_RADIUS * BULLET_RADIUS
}
