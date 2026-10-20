use crate::bosses::{ARCADE, CLASSIC, Kind, Slot};
use crate::game::{Bullet, Difficulty, FIELD_H, FIELD_W, FRAME, Game, Grid, Mode, State, Timer, Vec2};
use crate::{bot, render, save::Save};
use ratatui::layout::Rect;

const GRID: Grid = Grid { cols: 93, rows: 29 };

fn bullets(g: &Game, from_player: bool) -> usize {
    g.bullets.iter().filter(|b| b.from_player == from_player).count()
}

fn classic(level: i32) -> Game {
    let mut g = Game::new(7);
    g.new_game(level);
    g
}

fn shot_at(p: Vec2) -> Bullet {
    Bullet {
        pos: p,
        prev: p,
        dir: Vec2::new(0.0, 1.0),
        speed: 0.0,
        accel: 0.,
        curve: 0.,
        homing: 0.,
        home_t: 0.,
        bounce: 0,
        damage: 2.0,
        lifetime: 10.0,
        size: 1.0,
        glyph: 'o',
        color: (255, 255, 255),
        from_player: false,
        grazed: false,
    }
}

// ------------------------------------------------------------- classic

#[test]
fn a_zero_timer_fires_every_frame_and_a_set_one_on_time() {
    let mut t = Timer::default();
    assert!(t.tick(FRAME) && t.tick(FRAME));
    let mut t = Timer::from_seconds(0.1);
    // six 60 Hz frames are a few nanoseconds short of 0.1 s
    let fired: Vec<bool> = (0..8).map(|_| t.tick(FRAME)).collect();
    assert_eq!(fired, [false, false, false, false, false, false, true, false]);
}

#[test]
fn the_sentinel_opens_with_a_ring_of_eight_and_one_aimed_shot() {
    let mut g = classic(1);
    g.update((0, 0), false, GRID);
    let shots: Vec<char> = g.bullets.iter().map(|b| b.glyph).collect();
    assert_eq!(shots.iter().filter(|&&c| c == '.').count(), 8);
    assert_eq!(shots.iter().filter(|&&c| c == '|').count(), 1);
    let aimed = g.bullets.iter().find(|b| b.glyph == '|').unwrap();
    assert!(aimed.dir.y < -0.999);
}

#[test]
fn the_player_fires_only_while_moving() {
    let mut g = classic(1);
    g.update((0, 0), false, GRID);
    assert_eq!(bullets(&g, true), 0);
    g.update((1, 0), false, GRID);
    g.update((1, 1), false, GRID);
    assert_eq!(bullets(&g, true), 2);
    assert!(g.player.as_ref().unwrap().pos.x > FIELD_W / 2.0);
}

#[test]
fn the_sixth_hit_ends_a_classic_game() {
    let mut g = classic(1);
    g.boss.as_mut().unwrap().patterns = &[];
    for hit in 1..=6 {
        let p = g.player.as_ref().unwrap().pos;
        g.bullets = vec![shot_at(p)];
        g.update((0, 0), false, GRID);
        if hit < 6 {
            assert_eq!(g.state, State::Playing, "hit {hit}");
            assert!(g.bullets.is_empty());
        }
    }
    assert_eq!(g.state, State::GameOver);
    assert!(g.player.is_none() && g.bullets.is_empty());
}

#[test]
fn a_classic_boss_falls_after_751_hits_and_the_tenth_wins() {
    for level in [1, 10] {
        let mut g = classic(level);
        g.player.as_mut().unwrap().pos = Vec2::new(FIELD_W / 2.0, FIELD_H / 2.0 - 30.0);
        let (mut hits, mut frames) = (0, 0);
        while g.state == State::Playing && frames < 5000 {
            let before = g.boss.as_ref().unwrap().pool.health;
            g.bullets.retain(|b| b.from_player);
            g.update(if frames % 2 == 0 { (1, 0) } else { (-1, 0) }, false, GRID);
            hits += match &g.boss {
                Some(b) => ((before - b.pool.health) / 2.0).round() as i32,
                None => 1,
            };
            frames += 1;
        }
        assert_eq!(hits, 751, "level {level}");
        assert_eq!(g.state, if level == 10 { State::WinGame } else { State::FinishedLevel });
    }
}

#[test]
fn each_classic_slot_is_used_at_most_once_per_boss() {
    for (name, _, patterns) in CLASSIC {
        for s in Slot::ALL {
            assert!(patterns.iter().filter(|p| p.slot == Some(s)).count() <= 1, "{name}");
        }
    }
}

// -------------------------------------------------------------- arcade

fn arcade(level: i32) -> Game {
    let mut g = Game::new(11);
    g.start(Mode::Arcade, Difficulty::Normal, level);
    g
}

fn skip_intro(g: &mut Game) {
    while g.state == State::Intro {
        g.update((0, 0), false, GRID);
    }
}

#[test]
fn a_fight_opens_with_the_boss_arriving_and_holding_fire() {
    let mut g = arcade(3);
    assert_eq!(g.state, State::Intro);
    for _ in 0..60 {
        g.update((1, 0), false, GRID);
    }
    assert!(g.bullets.is_empty(), "no shots during the intro");
    skip_intro(&mut g);
    assert_eq!(g.state, State::Playing);
    for _ in 0..90 {
        g.update((0, 0), false, GRID);
    }
    assert!(bullets(&g, false) > 0);
}

#[test]
fn a_hit_gives_a_moment_of_safety() {
    let mut g = arcade(1);
    skip_intro(&mut g);
    g.boss.as_mut().unwrap().patterns = &[];
    let p = g.player.as_ref().unwrap().pos;
    g.bullets = vec![shot_at(p)];
    g.update((0, 0), false, GRID);
    assert_eq!(g.player.as_ref().unwrap().pool.health, 8.0);
    g.bullets = vec![shot_at(p)];
    g.update((0, 0), false, GRID);
    assert_eq!(g.player.as_ref().unwrap().pool.health, 8.0, "still blinking");
}

#[test]
fn a_bomb_clears_the_shots_and_spends_one() {
    let mut g = arcade(2);
    skip_intro(&mut g);
    for _ in 0..120 {
        g.update((0, 0), false, GRID);
    }
    let before = bullets(&g, false);
    assert!(before > 10);
    g.bomb();
    assert_eq!(g.bombs, 2);
    for _ in 0..60 {
        g.update((0, 0), false, GRID);
    }
    assert!(bullets(&g, false) < before / 2);
}

#[test]
fn breaking_a_phase_moves_on_and_clears_the_screen() {
    let mut g = arcade(1);
    skip_intro(&mut g);
    for _ in 0..60 {
        g.update((0, 0), false, GRID);
    }
    g.boss.as_mut().unwrap().pool.health = 2.0;
    let pos = g.player.as_ref().unwrap().pos;
    g.player.as_mut().unwrap().pos = Vec2::new(pos.x, FIELD_H * 0.6);
    let before = g.score;
    for _ in 0..30 {
        g.update((1, 0), false, GRID);
        if g.boss.as_ref().unwrap().phase == 1 {
            break;
        }
    }
    let b = g.boss.as_ref().unwrap();
    assert_eq!(b.phase, 1);
    assert!(b.invuln > 0.);
    assert_eq!(bullets(&g, false), 0);
    assert!(g.score >= before + 1000);
}

#[test]
fn every_arcade_boss_has_phases_and_runs_a_minute() {
    for level in 1..=10 {
        let def = &ARCADE[level as usize - 1];
        assert!(def.phases.len() >= 3, "{}", def.name);
        assert!(def.phases[0].patterns.len() >= 2, "{}", def.name);
        let mut g = arcade(level);
        for f in 0..3600 {
            let dir = [(1, 0), (0, 1), (-1, 0), (0, -1)][(f / 30) % 4];
            g.update(dir, f % 120 < 60, GRID);
            if !matches!(g.state, State::Intro | State::Playing) {
                break;
            }
            assert!(g.bullets.iter().all(|b| b.pos.x.is_finite() && b.pos.y.is_finite()));
            assert!(g.bullets.len() < 3000, "{} has {} bullets", def.name, g.bullets.len());
        }
    }
}

#[test]
fn every_attack_kind_is_used() {
    let all: Vec<Kind> = ARCADE.iter().flat_map(|b| b.phases.iter().flat_map(|p| p.patterns.iter().map(|x| x.kind))).collect();
    assert!(all.iter().any(|k| matches!(k, Kind::Laser { .. })));
    assert!(all.iter().any(|k| matches!(k, Kind::Wall { .. })));
    assert!(all.iter().any(|k| matches!(k, Kind::Rain)));
    assert!(all.iter().any(|k| matches!(k, Kind::Fan { .. })));
    assert!(all.iter().any(|k| matches!(k, Kind::Burst)));
}

#[test]
fn a_burning_beam_hurts_and_a_dotted_one_does_not() {
    let mut g = arcade(3);
    skip_intro(&mut g);
    // phase two of the Watcher fires aimed beams
    g.boss.as_mut().unwrap().pool.health = 0.5;
    g.player.as_mut().unwrap().pos = Vec2::new(FIELD_W / 2., FIELD_H * 0.62);
    for f in 0..60 {
        g.update(if f % 2 == 0 { (1, 0) } else { (-1, 0) }, false, GRID);
        if g.boss.as_ref().unwrap().phase == 1 {
            break;
        }
    }
    assert_eq!(g.boss.as_ref().unwrap().phase, 1);
    g.player.as_mut().unwrap().pos = Vec2::new(200., 120.);
    let mut seen_warn = false;
    for _ in 0..400 {
        g.update((0, 0), false, GRID);
        g.bullets.retain(|b| b.from_player);
        if g.lasers.iter().any(|l| !l.burning()) {
            seen_warn = true;
            assert_eq!(g.player.as_ref().unwrap().pool.health, 10.0);
        }
        if g.player.as_ref().unwrap().pool.health < 10.0 {
            break;
        }
    }
    assert!(seen_warn);
    assert!(g.player.as_ref().unwrap().pool.health < 10.0, "an aimed beam at a still player should burn");
}

#[test]
fn a_fast_bullet_cannot_skip_the_players_cell() {
    let grid = Grid { cols: 300, rows: 90 };
    let p = Vec2::new(600., 200.);
    let mut b = shot_at(Vec2::new(p.x, p.y + 6.));
    b.pos = Vec2::new(p.x, p.y - 4.);
    assert!(grid.sweep(b.prev, b.pos, |c| c == grid.cell(p)));
}

#[test]
fn the_field_keeps_the_window_shape() {
    let f = render::field_rect(Rect::new(0, 0, 100, 30), 2.0);
    assert_eq!((f.width, f.height), (93, 29));
    let f = render::field_rect(Rect::new(0, 0, 80, 40), 2.0);
    assert_eq!((f.width, f.height), (80, 25));
    let g = render::grid_of(f);
    assert_eq!(g.cell(Vec2::new(0.0, FIELD_H)), (0, 0));
    assert_eq!(g.cell(Vec2::new(FIELD_W - 0.01, 0.01)), (79, 24));
}

#[test]
fn the_save_reads_back_what_it_wrote() {
    let mut s = Save::default();
    s.difficulty = Difficulty::Hard;
    s.music = false;
    s.reached = 7;
    s.best = [1, 2, 3];
    s.best_time[4] = Some(42.5);
    let t = Save::parse(&s.text());
    assert_eq!(t.text(), s.text());
    assert_eq!(Save::parse("garbage\nreached 99\n").reached, 10);
}

/// How the bot fares against every boss, on every difficulty: a check
/// that each can be beaten. `cargo test --release -- --ignored
/// --nocapture bot_report`
#[test]
#[ignore]
fn bot_report() {
    for d in Difficulty::ALL {
        let mut wins = 0;
        for level in 1..=10 {
            let mut g = Game::new(level as u64 * 31 + d.index() as u64);
            g.start(Mode::Practice, d, level);
            let mut frames = 0;
            while matches!(g.state, State::Intro | State::Playing | State::BossDying | State::PlayerDying) && frames < 60 * 300 {
                let c = bot::choose(&g, GRID);
                if c.bomb {
                    g.bomb();
                }
                g.update(c.mv, c.focus, GRID);
                frames += 1;
            }
            let won = g.state == State::Results;
            wins += won as i32;
            let boss = g.boss.as_ref().map(|b| (b.phase, b.pool.health.round()));
            println!(
                "{:6} {level:2} {:24} {:?} after {:5.1}s, bombs left {}, hits {}, boss {boss:?}",
                d.name(),
                ARCADE[level as usize - 1].name,
                g.state,
                frames as f32 / 60.0,
                g.bombs,
                g.results.hits
            );
        }
        println!("{}: {wins} of 10", d.name());
    }
}
