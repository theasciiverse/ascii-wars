//! The game, from the original's `game.rs`, `player.rs`, `shoot.rs` and
//! `enemies.rs`, without Bevy: one `update` is one 60 Hz frame.
//!
//! Positions are the original's pixels, in its 1260x787.5 window with y
//! up, so speeds, fire rates and hit sizes carry over as they are. The
//! only rule that changes is how the player is hit (see `player_hit`).

use crate::bosses::{self, Pattern, Slot};
use std::time::Duration;

/// The original window: 1680x1050 at a resolution scale of 0.75.
pub const FIELD_W: f32 = 1680.0 * 0.75;
pub const FIELD_H: f32 = 1050.0 * 0.75;

pub const FPS: u32 = 60;
/// The original ran at the display's refresh with vsync; 60 Hz here.
pub const FRAME: Duration = Duration::from_nanos(1_000_000_000 / FPS as u64);
const DT: f32 = 1.0 / FPS as f32;

const PLAYER_SPEED: f32 = 250.0;
const PLAYER_BULLET_SPEED: f32 = 500.0;
const BULLET_LIFETIME: f32 = 10.0;
/// Every bullet hits as a circle of this radius, whatever its size.
const BULLET_RADIUS: f32 = 8.0;
pub const MAX_LEVEL: i32 = 10;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
    fn add_scaled(self, d: Vec2, s: f32) -> Vec2 {
        Vec2::new(self.x + d.x * s, self.y + d.y * s)
    }
    fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    /// Bevy's `normalize`: a zero vector gives NaNs, which never happens
    /// here because the player and the boss never share a spot exactly.
    fn normalize(self) -> Vec2 {
        let l = self.len();
        Vec2::new(self.x / l, self.y / l)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    MainMenu,
    Playing,
    PauseMenu,
    GameOver,
    FinishedLevel,
    WinGame,
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
}

pub struct Boss {
    pub pos: Vec2,
    pub glyph: char,
    pub pool: Pool,
    pub patterns: &'static [Pattern],
}

#[derive(Clone, Debug)]
pub struct Bullet {
    pub pos: Vec2,
    /// Where it was a frame ago, so a fast bullet can't jump over the
    /// player's cell.
    pub prev: Vec2,
    pub dir: Vec2,
    pub speed: f32,
    pub damage: f32,
    pub lifetime: f32,
    pub size: f32,
    pub glyph: char,
    pub from_player: bool,
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

/// The terminal cells the field is drawn on, which decide when the player
/// is hit.
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

    fn cell_px(self) -> (f32, f32) {
        (FIELD_W / self.cols as f32, FIELD_H / self.rows as f32)
    }
}

pub struct Game {
    pub state: State,
    pub level: i32,
    pub player: Option<Player>,
    pub boss: Option<Boss>,
    pub bullets: Vec<Bullet>,
    /// One per `Slot`. Bevy resources: they carry over from level to
    /// level and from game to game, as in the original.
    timers: [Timer; 6],
    /// Bevy's virtual time, which runs in the menus too; the rings turn
    /// with it.
    pub clock: Duration,
    pub frames: u64,
    /// What the health bar last showed; it stays on the menus after the
    /// player is gone, like the original's HUD.
    pub hud_health: f32,
}

impl Default for Game {
    fn default() -> Self {
        Game::new()
    }
}

impl Game {
    pub fn new() -> Game {
        Game {
            state: State::MainMenu,
            level: 0,
            player: None,
            boss: None,
            bullets: Vec::new(),
            timers: [Timer::default(); 6],
            clock: Duration::ZERO,
            frames: 0,
            hud_health: 1.0,
        }
    }

    /// Play, or Play Again: from the first boss, or from `start` (1 to 10).
    pub fn new_game(&mut self, start: i32) {
        self.clean_up_level();
        self.level = start.clamp(1, MAX_LEVEL) - 1;
        self.prepare_level();
    }

    /// Continue to next level.
    pub fn next_level(&mut self) {
        self.clean_up_level();
        self.prepare_level();
    }

    pub fn pause(&mut self) {
        if self.state == State::Playing {
            self.state = State::PauseMenu;
        }
    }

    pub fn resume(&mut self) {
        if self.state == State::PauseMenu {
            self.state = State::Playing;
        }
    }

    fn clean_up_level(&mut self) {
        self.player = None;
        self.boss = None;
        self.bullets.clear();
    }

    fn prepare_level(&mut self) {
        if self.level >= MAX_LEVEL {
            self.state = State::WinGame;
            return;
        }
        self.level += 1;
        let cx = FIELD_W * 0.5;
        let cy = FIELD_H * 0.5;
        self.player = Some(Player {
            pos: Vec2::new(cx, cy - 300.),
            facing: Vec2::new(0., 1.),
            pool: Pool { health: 10., max_health: 10., damage: 2. },
        });
        let def = bosses::boss(self.level);
        self.boss = Some(Boss {
            pos: Vec2::new(cx, cy),
            glyph: def.glyph,
            pool: Pool { health: 1500., max_health: 1500., damage: 2. },
            patterns: def.patterns,
        });
        self.hud_health = 1.0;
        self.state = State::Playing;
        log::info!("level {} {}", self.level, def.name);
    }

    /// One frame. `mv` is the held direction (each axis -1, 0 or 1).
    pub fn update(&mut self, mv: (i8, i8), grid: Grid) {
        self.frames += 1;
        self.clock += FRAME;
        if self.state != State::Playing {
            return;
        }
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
            spawned.push(bullet(player.pos, player.facing, PLAYER_BULLET_SPEED, player.pool.damage, 1.0, '*', true));
        }

        // enemies.rs: one timer per pattern system, ticked every frame
        // whether or not this boss has that pattern
        let t = self.clock.as_secs_f32();
        for slot in Slot::ALL {
            let timer = &mut self.timers[slot.index()];
            if !timer.tick(FRAME) {
                continue;
            }
            let Some(p) = boss.patterns.iter().find(|p| p.slot == slot) else { continue };
            *timer = Timer::from_seconds(p.fire_rate);
            for i in 0..p.spawn_count {
                let dir = if slot == Slot::Direct {
                    player.pos.sub(boss.pos).normalize()
                } else {
                    let a = i as f32 / p.spawn_count as f32 * std::f32::consts::TAU + t * p.rotation_speed;
                    Vec2::new(a.cos(), a.sin())
                };
                spawned.push(bullet(boss.pos, dir, p.bullet_speed, boss.pool.damage, p.bullet_size, p.glyph, false));
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
                (&mut boss.pool, boss_hit(b, boss.pos))
            } else {
                (&mut player.pool, player_hit(b, player.pos, grid))
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
}

impl Bullet {
    /// Drawn bold: the original's bullets at twice the size or more.
    pub fn bullet_big(&self) -> bool {
        self.size >= 2.0
    }
}

fn bullet(pos: Vec2, dir: Vec2, speed: f32, damage: f32, size: f32, glyph: char, from_player: bool) -> Bullet {
    Bullet { pos, prev: pos, dir, speed, damage, lifetime: BULLET_LIFETIME, size, glyph, from_player }
}

/// The original test: the bullet's circle against the boss's box, whose
/// half size is the sprite's scale (2x2 pixels).
fn boss_hit(b: &Bullet, centre: Vec2) -> bool {
    let half = 2.0;
    let cx = b.pos.x.clamp(centre.x - half, centre.x + half);
    let cy = b.pos.y.clamp(centre.y - half, centre.y + half);
    let (dx, dy) = (b.pos.x - cx, b.pos.y - cy);
    dx * dx + dy * dy <= BULLET_RADIUS * BULLET_RADIUS
}

/// The player is hit by a bullet that is drawn in the player's cell, or
/// passed through it since the last frame: what you see is what hits you.
/// The original's test (an 8 pixel circle against a 2.6 pixel box) is
/// smaller than a cell is tall at 100x30, so a bullet could hit from the
/// cell above or miss one drawn on top of the player; by area the two are
/// about the same at 100x30.
fn player_hit(b: &Bullet, pos: Vec2, grid: Grid) -> bool {
    let target = grid.cell(pos);
    let (cw, ch) = grid.cell_px();
    let d = b.pos.sub(b.prev);
    let steps = (d.x.abs() / (cw * 0.5)).max(d.y.abs() / (ch * 0.5)).ceil().max(1.0) as i32;
    (0..=steps).any(|i| grid.cell(b.prev.add_scaled(d, i as f32 / steps as f32)) == target)
}
