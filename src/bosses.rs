//! The ten bosses, as the original's `enemies.rs` sets them up: a glyph
//! and up to six shot patterns each. The names are the original's own
//! (comments on `BOSSES_GLYPH`).

/// Which of the original's six pattern systems fires a pattern. Each has
/// its own timer, shared by every level (see `game::Timers`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    P1,
    P2,
    P3,
    P4,
    P5,
    /// Aimed at the player rather than spread round a ring.
    Direct,
}

impl Slot {
    pub const ALL: [Slot; 6] = [Slot::P1, Slot::P2, Slot::P3, Slot::P4, Slot::P5, Slot::Direct];

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Pattern {
    pub slot: Slot,
    /// Pixels a second, in the original's 1260x787.5 window.
    pub bullet_speed: f32,
    pub spawn_count: i32,
    /// Radians a second the ring turns with the game clock.
    pub rotation_speed: f32,
    /// Seconds between volleys.
    pub fire_rate: f32,
    /// Sprite scale of the bullet; only how it looks, never how it hits.
    pub bullet_size: f32,
    pub glyph: char,
}

pub struct BossDef {
    pub name: &'static str,
    pub glyph: char,
    pub patterns: &'static [Pattern],
}

const fn ring(slot: Slot, bullet_speed: f32, spawn_count: i32, rotation_speed: f32, fire_rate: f32, bullet_size: f32, glyph: char) -> Pattern {
    Pattern { slot, bullet_speed, spawn_count, rotation_speed, fire_rate, bullet_size, glyph }
}

const fn direct(bullet_speed: f32, spawn_count: i32, fire_rate: f32, bullet_size: f32, glyph: char) -> Pattern {
    Pattern { slot: Slot::Direct, bullet_speed, spawn_count, rotation_speed: 0.0, fire_rate, bullet_size, glyph }
}

use Slot::*;

pub const BOSSES: [BossDef; 10] = [
    BossDef {
        name: "The Sentinel",
        glyph: '#',
        patterns: &[ring(P1, 100., 8, 0., 0.8, 3.0, '.'), direct(100., 1, 1., 3.0, '|')],
    },
    BossDef {
        name: "The Hoarder",
        glyph: '$',
        patterns: &[ring(P1, 80., 64, 0., 2., 1.3, 'x'), ring(P4, 100., 8, -0.8, 0.3, 1.0, '$')],
    },
    BossDef {
        name: "The Watcher",
        glyph: '%',
        patterns: &[ring(P1, 100., 64, 0., 1., 1.0, 'o'), direct(300., 1, 3., 3.0, '|')],
    },
    BossDef {
        name: "The Infernal Imp",
        glyph: '@',
        patterns: &[ring(P1, 300., 4, 0.3, 0., 1.0, '~'), direct(250., 1, 2., 2.0, 'O'), ring(P2, 100., 32, 0., 2., 1.0, 'o')],
    },
    BossDef {
        name: "The Spiral Beast",
        glyph: '&',
        patterns: &[ring(P1, 250., 16, 0.8, 0.2, 1.5, '0'), ring(P2, 100., 6, 0.8, 0.05, 1.0, 'x'), ring(P3, 80., 64, 0., 5., 2.0, '|')],
    },
    BossDef {
        name: "The Guardian of Order",
        glyph: '+',
        patterns: &[
            ring(P1, 300., 4, 0., 0.4, 1.5, 'x'),
            direct(80., 1, 2., 2.0, 'o'),
            ring(P2, 300., 8, 0.3, 0.1, 1., '|'),
            ring(P3, 50., 64, 0.3, 3.5, 1., '#'),
        ],
    },
    BossDef {
        name: "The Swarm King",
        glyph: '*',
        patterns: &[
            ring(P1, 300., 5, 0., 0.3, 1.3, 'x'),
            ring(P2, 100., 64, 0., 2., 1., 'O'),
            ring(P3, 150., 16, 2., 0.5, 1., '%'),
            direct(450., 1, 1., 3.0, '.'),
        ],
    },
    BossDef {
        name: "The Celestial Tyrant",
        glyph: 'O',
        patterns: &[
            ring(P1, 300., 8, 0.0, 0.2, 1.3, '|'),
            ring(P2, 600., 64, 0.0, 5., 1.3, 'o'),
            direct(200., 1, 0.4, 2.0, 'x'),
            ring(P3, 250., 5, 0.5, 0.1, 1.5, '*'),
            ring(P4, 250., 5, -0.5, 0.1, 1.5, '*'),
        ],
    },
    BossDef {
        name: "The Clockwork Leviathan",
        glyph: '=',
        patterns: &[
            ring(P1, 250., 12, 0.0, 0.1, 1., '='),
            ring(P2, 300., 1, -3.0, 0.05, 1.5, '0'),
            ring(P3, 200., 64, 0., 1., 1., '#'),
            ring(P4, 250., 1, -5.0, 0.05, 2.0, '|'),
            direct(500., 1, 2.35, 2.0, 'x'),
        ],
    },
    BossDef {
        name: "The Chaos Reaper",
        glyph: '~',
        patterns: &[
            ring(P1, 200., 6, -0.5, 0.1, 1.5, '*'),
            ring(P2, 200., 6, 0.5, 0.1, 1.5, '*'),
            ring(P3, 400., 16, 0.8, 0.2, 1.0, '.'),
            ring(P4, 300., 16, -0.8, 0.3, 1.0, 'o'),
            ring(P5, 100., 64, 0.0, 5.2, 1.0, '&'),
            direct(500., 1, 5.3, 2.0, 'x'),
        ],
    },
];

/// The boss for a level, 1 to 10; past 10 the original falls back to the
/// Chaos Reaper.
pub fn boss(level: i32) -> &'static BossDef {
    &BOSSES[(level.clamp(1, 10) - 1) as usize]
}
