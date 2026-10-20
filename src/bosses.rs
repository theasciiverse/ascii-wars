//! The bosses. `CLASSIC` is the original's ten, as its `enemies.rs` sets
//! them up (names from its comments on `BOSSES_GLYPH`). `ARCADE` is ours:
//! the same ten grown into fights in phases, each opening with its classic
//! attack, then moving, aiming, raining, sweeping lasers and homing in.

use std::f32::consts::PI;

/// Which of the original's six pattern systems fires a classic pattern.
/// Each has its own timer, shared by every level (see `game.rs`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    P1,
    P2,
    P3,
    P4,
    P5,
    Direct,
}

impl Slot {
    pub const ALL: [Slot; 6] = [Slot::P1, Slot::P2, Slot::P3, Slot::P4, Slot::P5, Slot::Direct];

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    /// `count` bullets round a circle, turned by `rotation` radians a
    /// second of the fight; fired fast with few bullets it is a spiral.
    Ring,
    /// Like a ring, at a random turn each volley.
    Burst,
    /// `count` bullets straight at the player (the original stacks them).
    Aimed,
    /// `count` bullets spread over `spread` radians, centred on the player.
    Fan { spread: f32 },
    /// `count` bullets dropping from random places along the top.
    Rain,
    /// A row across the top with a gap of `gap` bullets, falling.
    Wall { gap: i32 },
    /// `count` beams from the boss, `spread` radians apart, aimed at the
    /// player or starting straight down; dotted for `warn` seconds, then
    /// burning for `beam` seconds, turning `sweep` radians a second.
    Laser { spread: f32, warn: f32, beam: f32, sweep: f32, aimed: bool },
}

pub type Rgb = (u8, u8, u8);

#[derive(Clone, Copy, Debug)]
pub struct Pattern {
    pub kind: Kind,
    pub count: i32,
    pub rotation: f32,
    /// Seconds between volleys.
    pub rate: f32,
    /// Seconds into a phase before the first volley.
    pub delay: f32,
    /// Pixels a second, in the original's 1260x787.5 window.
    pub speed: f32,
    /// Pixels a second, every second; speed stays between 30 and 600.
    pub accel: f32,
    /// Radians a second the bullet turns.
    pub curve: f32,
    /// Radians a second it turns towards the player, for its first 1.5 s.
    pub homing: f32,
    /// Times it bounces off the edges of the window.
    pub bounce: u8,
    /// The original's sprite scale: 2 and up is drawn bold.
    pub size: f32,
    pub glyph: char,
    pub color: Rgb,
    /// Classic patterns only: the original's timer slot.
    pub slot: Option<Slot>,
}

impl Pattern {
    const fn new(kind: Kind, count: i32, rate: f32, speed: f32, glyph: char, color: Rgb) -> Pattern {
        Pattern {
            kind,
            count,
            rotation: 0.0,
            rate,
            delay: 0.6,
            speed,
            accel: 0.0,
            curve: 0.0,
            homing: 0.0,
            bounce: 0,
            size: 1.0,
            glyph,
            color,
            slot: None,
        }
    }
    const fn rot(mut self, r: f32) -> Pattern {
        self.rotation = r;
        self
    }
    const fn accel(mut self, a: f32) -> Pattern {
        self.accel = a;
        self
    }
    const fn curve(mut self, c: f32) -> Pattern {
        self.curve = c;
        self
    }
    const fn homing(mut self, h: f32) -> Pattern {
        self.homing = h;
        self
    }
    const fn bounce(mut self, b: u8) -> Pattern {
        self.bounce = b;
        self
    }
    const fn big(mut self) -> Pattern {
        self.size = 2.0;
        self
    }
    const fn delay(mut self, d: f32) -> Pattern {
        self.delay = d;
        self
    }
}

const fn ring(count: i32, rate: f32, speed: f32, glyph: char, color: Rgb) -> Pattern {
    Pattern::new(Kind::Ring, count, rate, speed, glyph, color)
}
const fn burst(count: i32, rate: f32, speed: f32, glyph: char, color: Rgb) -> Pattern {
    Pattern::new(Kind::Burst, count, rate, speed, glyph, color)
}
const fn aimed(rate: f32, speed: f32, glyph: char, color: Rgb) -> Pattern {
    Pattern::new(Kind::Aimed, 1, rate, speed, glyph, color)
}
const fn fan(count: i32, spread: f32, rate: f32, speed: f32, glyph: char, color: Rgb) -> Pattern {
    Pattern::new(Kind::Fan { spread }, count, rate, speed, glyph, color)
}
const fn rain(count: i32, rate: f32, speed: f32, glyph: char, color: Rgb) -> Pattern {
    Pattern::new(Kind::Rain, count, rate, speed, glyph, color)
}
const fn wall(gap: i32, rate: f32, speed: f32, glyph: char, color: Rgb) -> Pattern {
    Pattern::new(Kind::Wall { gap }, 0, rate, speed, glyph, color)
}
const fn laser(count: i32, spread: f32, warn: f32, beam: f32, sweep: f32, aimed: bool, rate: f32, color: Rgb) -> Pattern {
    Pattern::new(Kind::Laser { spread, warn, beam, sweep, aimed }, count, rate, 0.0, '|', color)
}

/// How a boss moves round its home, in pixels and cycles a second.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Motion {
    Still,
    /// A Lissajous figure round home.
    Sway { ax: f32, ay: f32, fx: f32, fy: f32 },
    /// Glide to a new random spot every `every` seconds.
    Hop { every: f32 },
    /// Follow the player along the top at `speed`.
    Chase { speed: f32 },
}

pub struct Phase {
    /// The attack's name, shown as it starts.
    pub name: &'static str,
    pub hp: f32,
    pub motion: Motion,
    pub patterns: &'static [Pattern],
}

pub struct BossDef {
    pub name: &'static str,
    pub epithet: &'static str,
    pub glyph: char,
    /// Drawn round the boss's position, one glyph a cell.
    pub art: &'static [&'static str],
    pub color: Rgb,
    pub phases: &'static [Phase],
}

// ------------------------------------------------------------------ classic

const WHITE: Rgb = (235, 235, 235);

const fn classic(slot: Slot, speed: f32, count: i32, rotation: f32, rate: f32, size: f32, glyph: char) -> Pattern {
    let kind = if matches!(slot, Slot::Direct) { Kind::Aimed } else { Kind::Ring };
    let mut p = Pattern::new(kind, count, rate, speed, glyph, WHITE);
    p.rotation = rotation;
    p.size = size;
    p.delay = 0.0;
    p.slot = Some(slot);
    p
}

use Slot::*;

/// The original's ten, in its order.
pub const CLASSIC: [(&str, char, &[Pattern]); 10] = [
    ("The Sentinel", '#', &[classic(P1, 100., 8, 0., 0.8, 3.0, '.'), classic(Direct, 100., 1, 0., 1., 3.0, '|')]),
    ("The Hoarder", '$', &[classic(P1, 80., 64, 0., 2., 1.3, 'x'), classic(P4, 100., 8, -0.8, 0.3, 1.0, '$')]),
    ("The Watcher", '%', &[classic(P1, 100., 64, 0., 1., 1.0, 'o'), classic(Direct, 300., 1, 0., 3., 3.0, '|')]),
    (
        "The Infernal Imp",
        '@',
        &[classic(P1, 300., 4, 0.3, 0., 1.0, '~'), classic(Direct, 250., 1, 0., 2., 2.0, 'O'), classic(P2, 100., 32, 0., 2., 1.0, 'o')],
    ),
    (
        "The Spiral Beast",
        '&',
        &[classic(P1, 250., 16, 0.8, 0.2, 1.5, '0'), classic(P2, 100., 6, 0.8, 0.05, 1.0, 'x'), classic(P3, 80., 64, 0., 5., 2.0, '|')],
    ),
    (
        "The Guardian of Order",
        '+',
        &[
            classic(P1, 300., 4, 0., 0.4, 1.5, 'x'),
            classic(Direct, 80., 1, 0., 2., 2.0, 'o'),
            classic(P2, 300., 8, 0.3, 0.1, 1., '|'),
            classic(P3, 50., 64, 0.3, 3.5, 1., '#'),
        ],
    ),
    (
        "The Swarm King",
        '*',
        &[
            classic(P1, 300., 5, 0., 0.3, 1.3, 'x'),
            classic(P2, 100., 64, 0., 2., 1., 'O'),
            classic(P3, 150., 16, 2., 0.5, 1., '%'),
            classic(Direct, 450., 1, 0., 1., 3.0, '.'),
        ],
    ),
    (
        "The Celestial Tyrant",
        'O',
        &[
            classic(P1, 300., 8, 0.0, 0.2, 1.3, '|'),
            classic(P2, 600., 64, 0.0, 5., 1.3, 'o'),
            classic(Direct, 200., 1, 0., 0.4, 2.0, 'x'),
            classic(P3, 250., 5, 0.5, 0.1, 1.5, '*'),
            classic(P4, 250., 5, -0.5, 0.1, 1.5, '*'),
        ],
    ),
    (
        "The Clockwork Leviathan",
        '=',
        &[
            classic(P1, 250., 12, 0.0, 0.1, 1., '='),
            classic(P2, 300., 1, -3.0, 0.05, 1.5, '0'),
            classic(P3, 200., 64, 0., 1., 1., '#'),
            classic(P4, 250., 1, -5.0, 0.05, 2.0, '|'),
            classic(Direct, 500., 1, 0., 2.35, 2.0, 'x'),
        ],
    ),
    (
        "The Chaos Reaper",
        '~',
        &[
            classic(P1, 200., 6, -0.5, 0.1, 1.5, '*'),
            classic(P2, 200., 6, 0.5, 0.1, 1.5, '*'),
            classic(P3, 400., 16, 0.8, 0.2, 1.0, '.'),
            classic(P4, 300., 16, -0.8, 0.3, 1.0, 'o'),
            classic(P5, 100., 64, 0.0, 5.2, 1.0, '&'),
            classic(Direct, 500., 1, 0., 5.3, 2.0, 'x'),
        ],
    ),
];

// ------------------------------------------------------------------- arcade

const STEEL: Rgb = (120, 170, 255);
const ICE: Rgb = (170, 230, 255);
const GOLD: Rgb = (255, 205, 70);
const AMBER: Rgb = (255, 150, 40);
const VIOLET: Rgb = (200, 130, 255);
const LILAC: Rgb = (235, 190, 255);
const FIRE: Rgb = (255, 95, 45);
const EMBER: Rgb = (255, 180, 80);
const GREEN: Rgb = (100, 235, 120);
const LIME: Rgb = (190, 255, 120);
const TEAL: Rgb = (90, 225, 215);
const PALE: Rgb = (210, 250, 245);
const HIVE: Rgb = (215, 235, 60);
const STING: Rgb = (255, 240, 150);
const PINK: Rgb = (255, 120, 200);
const STAR: Rgb = (255, 225, 245);
const BRASS: Rgb = (225, 165, 85);
const COPPER: Rgb = (240, 120, 70);
const BLOOD: Rgb = (235, 45, 65);
const BONE: Rgb = (240, 230, 210);

pub const ARCADE: [BossDef; 10] = [
    BossDef {
        name: "The Sentinel",
        epithet: "keeper of the first gate",
        glyph: '#',
        art: &[" /#\\ ", "[###]", " \\#/ "],
        color: STEEL,
        phases: &[
            Phase {
                name: "Lattice",
                hp: 450.,
                motion: Motion::Still,
                patterns: &[ring(8, 0.8, 100., '.', ICE).big(), aimed(1.0, 110., '|', STEEL).big()],
            },
            Phase {
                name: "Crossfire",
                hp: 550.,
                motion: Motion::Sway { ax: 260., ay: 30., fx: 0.12, fy: 0.24 },
                patterns: &[ring(12, 0.55, 120., '.', ICE).rot(0.6), fan(3, 0.5, 1.3, 170., '|', STEEL).big()],
            },
            Phase {
                name: "Searchlight",
                hp: 650.,
                motion: Motion::Sway { ax: 200., ay: 40., fx: 0.1, fy: 0.2 },
                patterns: &[
                    laser(1, 0., 0.9, 0.7, 0., true, 3.2, ICE).delay(1.2),
                    ring(16, 1.0, 110., 'o', STEEL),
                    aimed(1.6, 160., '|', ICE),
                ],
            },
        ],
    },
    BossDef {
        name: "The Hoarder",
        epithet: "it keeps everything it is given",
        glyph: '$',
        art: &[" .-$-. ", "($$$$$)", " '-$-' "],
        color: GOLD,
        phases: &[
            Phase {
                name: "Counting House",
                hp: 500.,
                motion: Motion::Still,
                patterns: &[ring(64, 2.0, 80., 'x', AMBER), ring(8, 0.3, 100., '$', GOLD).rot(-0.8)],
            },
            Phase {
                name: "Gold Rain",
                hp: 600.,
                motion: Motion::Sway { ax: 300., ay: 20., fx: 0.09, fy: 0.18 },
                patterns: &[rain(2, 0.22, 90., '$', GOLD).accel(70.), ring(24, 1.6, 90., 'o', AMBER)],
            },
            Phase {
                name: "Compound Interest",
                hp: 700.,
                motion: Motion::Still,
                patterns: &[ring(4, 0.12, 110., '$', GOLD).rot(1.2).accel(35.), fan(5, 0.8, 1.6, 180., 'x', AMBER)],
            },
        ],
    },
    BossDef {
        name: "The Watcher",
        epithet: "it has not blinked in a thousand years",
        glyph: '%',
        art: &[" .---. ", "( (%) )", " '---' "],
        color: VIOLET,
        phases: &[
            Phase {
                name: "Unblinking",
                hp: 550.,
                motion: Motion::Still,
                patterns: &[ring(64, 1.0, 100., 'o', VIOLET), aimed(3.0, 300., '|', LILAC).big()],
            },
            Phase {
                name: "Gaze",
                hp: 650.,
                motion: Motion::Sway { ax: 280., ay: 50., fx: 0.11, fy: 0.17 },
                patterns: &[laser(1, 0., 0.9, 0.8, 0., true, 2.4, LILAC).delay(1.0), ring(20, 1.1, 110., 'o', VIOLET).rot(0.4)],
            },
            Phase {
                name: "Thousand Eyes",
                hp: 750.,
                motion: Motion::Hop { every: 3.5 },
                patterns: &[
                    laser(3, 0.55, 1.0, 0.6, 0., true, 3.4, LILAC).delay(1.2),
                    burst(28, 0.9, 90., 'o', VIOLET).accel(30.),
                    fan(5, 0.6, 1.7, 210., '.', LILAC),
                ],
            },
        ],
    },
    BossDef {
        name: "The Infernal Imp",
        epithet: "small, red and furious",
        glyph: '@',
        art: &[" ^ ^ ", "(>@<)", " vVv "],
        color: FIRE,
        phases: &[
            Phase {
                name: "Hellfire Fountain",
                hp: 550.,
                motion: Motion::Still,
                patterns: &[ring(4, 0.05, 260., '~', FIRE).rot(0.3), aimed(2.0, 250., 'O', EMBER).big(), ring(32, 2.0, 100., 'o', FIRE)],
            },
            Phase {
                name: "Ember Storm",
                hp: 650.,
                motion: Motion::Hop { every: 2.8 },
                patterns: &[rain(3, 0.25, 80., '*', EMBER).accel(80.), fan(7, 1.2, 1.2, 190., '~', FIRE).curve(0.35)],
            },
            Phase {
                name: "Inferno",
                hp: 750.,
                motion: Motion::Chase { speed: 70. },
                patterns: &[
                    ring(3, 0.08, 150., '~', FIRE).rot(2.0),
                    ring(24, 1.5, 80., 'o', EMBER).accel(50.),
                    aimed(2.6, 140., 'O', EMBER).homing(1.6).big(),
                ],
            },
        ],
    },
    BossDef {
        name: "The Spiral Beast",
        epithet: "it swallows its own tail",
        glyph: '&',
        art: &[" ,&&, ", "&(&&)&", " '&&' "],
        color: GREEN,
        phases: &[
            Phase {
                name: "Coil",
                hp: 600.,
                motion: Motion::Still,
                patterns: &[ring(12, 0.25, 200., '0', GREEN).rot(0.8), ring(6, 0.08, 100., 'x', LIME).rot(0.8), ring(48, 5.0, 80., '|', GREEN).big()],
            },
            Phase {
                name: "Double Helix",
                hp: 700.,
                motion: Motion::Sway { ax: 220., ay: 30., fx: 0.08, fy: 0.16 },
                patterns: &[ring(2, 0.07, 150., 'x', LIME).rot(2.4), ring(2, 0.07, 150., '0', GREEN).rot(-2.4), ring(16, 1.4, 110., 'o', GREEN).curve(0.8)],
            },
            Phase {
                name: "Ouroboros",
                hp: 800.,
                motion: Motion::Still,
                patterns: &[
                    ring(22, 1.0, 105., 'o', GREEN).curve(1.1),
                    ring(22, 1.0, 105., 'o', LIME).curve(-1.1).delay(1.1),
                    fan(3, 0.4, 1.5, 220., 'x', LIME),
                ],
            },
        ],
    },
    BossDef {
        name: "The Guardian of Order",
        epithet: "everything in its place",
        glyph: '+',
        art: &["+-+-+", "|+++|", "+-+-+"],
        color: TEAL,
        phases: &[
            Phase {
                name: "Decree",
                hp: 600.,
                motion: Motion::Still,
                patterns: &[
                    ring(4, 0.4, 280., 'x', PALE),
                    aimed(2.0, 90., 'o', TEAL).big(),
                    ring(8, 0.15, 250., '|', TEAL).rot(0.3),
                    ring(48, 3.5, 55., '#', PALE).rot(0.3),
                ],
            },
            Phase {
                name: "Grid Law",
                hp: 700.,
                motion: Motion::Sway { ax: 150., ay: 0., fx: 0.1, fy: 0. },
                patterns: &[wall(4, 1.7, 95., '#', TEAL), aimed(1.1, 200., 'o', PALE)],
            },
            Phase {
                name: "Absolute Order",
                hp: 800.,
                motion: Motion::Still,
                patterns: &[laser(4, PI / 2.0, 1.2, 2.6, 0.35, false, 4.6, PALE).delay(0.8), ring(16, 1.1, 100., '+', TEAL)],
            },
        ],
    },
    BossDef {
        name: "The Swarm King",
        epithet: "ten thousand wings, one will",
        glyph: '*',
        art: &["\\ * /", "-(*)-", "/ * \\"],
        color: HIVE,
        phases: &[
            Phase {
                name: "Hive",
                hp: 600.,
                motion: Motion::Still,
                patterns: &[ring(48, 2.2, 90., 'O', HIVE), ring(16, 0.5, 150., '%', STING).rot(2.0), aimed(1.0, 400., '.', STING).big()],
            },
            Phase {
                name: "Swarm",
                hp: 700.,
                motion: Motion::Sway { ax: 260., ay: 40., fx: 0.13, fy: 0.21 },
                patterns: &[burst(6, 1.1, 120., '%', STING).homing(1.8), ring(20, 1.7, 100., 'o', HIVE)],
            },
            Phase {
                name: "Plague",
                hp: 800.,
                motion: Motion::Chase { speed: 60. },
                patterns: &[burst(9, 1.4, 100., '%', STING).homing(1.2), rain(2, 0.3, 120., '.', HIVE), fan(5, 0.7, 1.3, 200., 'x', STING)],
            },
        ],
    },
    BossDef {
        name: "The Celestial Tyrant",
        epithet: "the sky is its throne",
        glyph: 'O',
        art: &[". * .", "*(O)*", "' * '"],
        color: PINK,
        phases: &[
            Phase {
                name: "Firmament",
                hp: 650.,
                motion: Motion::Still,
                patterns: &[
                    ring(8, 0.3, 250., '|', PINK),
                    aimed(0.6, 200., 'x', STAR).big(),
                    ring(5, 0.12, 200., '*', STAR).rot(0.5),
                    ring(5, 0.12, 200., '*', STAR).rot(-0.5),
                ],
            },
            Phase {
                name: "Meteor Shower",
                hp: 750.,
                motion: Motion::Sway { ax: 280., ay: 30., fx: 0.07, fy: 0.14 },
                patterns: &[rain(3, 0.22, 140., '*', STAR).accel(90.), ring(12, 1.5, 160., 'o', PINK).bounce(1)],
            },
            Phase {
                name: "Supernova",
                hp: 850.,
                motion: Motion::Still,
                patterns: &[
                    ring(36, 1.6, 40., '*', STAR).accel(130.),
                    laser(5, 0.5, 1.2, 1.2, 0.3, false, 4.8, PINK).delay(2.0),
                    fan(3, 0.35, 1.1, 220., 'x', PINK),
                ],
            },
        ],
    },
    BossDef {
        name: "The Clockwork Leviathan",
        epithet: "it was wound up before the world began",
        glyph: '=',
        art: &["[=o=]", "|=O=|", "[=o=]"],
        color: BRASS,
        phases: &[
            Phase {
                name: "Escapement",
                hp: 650.,
                motion: Motion::Still,
                patterns: &[
                    ring(12, 0.15, 210., '=', BRASS),
                    ring(1, 0.08, 250., '0', COPPER).rot(-3.0),
                    ring(48, 1.3, 170., '#', BRASS),
                    aimed(2.35, 420., 'x', COPPER).big(),
                ],
            },
            Phase {
                name: "Clock Hands",
                hp: 750.,
                motion: Motion::Still,
                patterns: &[laser(2, PI, 1.0, 3.6, 0.7, false, 5.2, COPPER).delay(0.6), ring(12, 0.65, 140., '=', BRASS)],
            },
            Phase {
                name: "Mainspring",
                hp: 850.,
                motion: Motion::Sway { ax: 180., ay: 40., fx: 0.1, fy: 0.2 },
                patterns: &[
                    ring(4, 0.1, 140., '0', BRASS).rot(1.6).accel(30.),
                    laser(1, 0., 0.8, 1.5, 0.5, true, 3.4, COPPER).delay(1.5),
                    ring(20, 1.2, 120., '#', COPPER).bounce(1),
                ],
            },
        ],
    },
    BossDef {
        name: "The Chaos Reaper",
        epithet: "the end of every boss rush",
        glyph: '~',
        art: &[" _~_ ", "(o~o)", "/|~|\\"],
        color: BLOOD,
        phases: &[
            Phase {
                name: "Harvest",
                hp: 650.,
                motion: Motion::Still,
                patterns: &[
                    ring(6, 0.12, 190., '*', BLOOD).rot(-0.5),
                    ring(6, 0.12, 190., '*', BONE).rot(0.5),
                    ring(16, 0.3, 300., '.', BONE).rot(0.8),
                    ring(48, 5.2, 100., '&', BLOOD),
                ],
            },
            Phase {
                name: "Scythe",
                hp: 750.,
                motion: Motion::Hop { every: 3.2 },
                patterns: &[
                    laser(2, 0.9, 1.0, 1.6, 0.9, true, 4.2, BLOOD).delay(1.0),
                    fan(7, 1.5, 1.2, 190., '~', BONE).curve(0.4),
                    rain(2, 0.35, 120., '.', BONE),
                ],
            },
            Phase {
                name: "Entropy",
                hp: 850.,
                motion: Motion::Sway { ax: 280., ay: 60., fx: 0.13, fy: 0.19 },
                patterns: &[
                    burst(22, 0.8, 95., 'o', BLOOD).accel(40.),
                    burst(3, 1.7, 130., '%', BONE).homing(1.4),
                    ring(10, 1.5, 150., 'x', BLOOD).bounce(2),
                ],
            },
            Phase {
                name: "Last Rites",
                hp: 650.,
                motion: Motion::Chase { speed: 90. },
                patterns: &[
                    ring(4, 0.07, 170., '~', BLOOD).rot(2.6),
                    ring(28, 0.9, 100., 'o', BONE),
                    laser(1, 0., 0.8, 0.8, 0., true, 2.6, BLOOD).delay(1.5),
                ],
            },
        ],
    },
];

pub fn arcade(level: i32) -> &'static BossDef {
    &ARCADE[(level.clamp(1, 10) - 1) as usize]
}

pub fn classic_def(level: i32) -> (&'static str, char, &'static [Pattern]) {
    CLASSIC[(level.clamp(1, 10) - 1) as usize]
}
