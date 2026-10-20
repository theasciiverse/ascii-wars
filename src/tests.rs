use crate::bosses::{BOSSES, Slot};
use crate::game::{Bullet, FIELD_H, FIELD_W, FRAME, Game, Grid, State, Timer, Vec2};
use crate::render;
use ratatui::layout::Rect;

const GRID: Grid = Grid { cols: 93, rows: 29 };

fn bullets(g: &Game, from_player: bool) -> usize {
    g.bullets.iter().filter(|b| b.from_player == from_player).count()
}

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
    let mut g = Game::new();
    g.new_game(1);
    g.update((0, 0), GRID);
    let shots: Vec<char> = g.bullets.iter().map(|b| b.glyph).collect();
    assert_eq!(shots.iter().filter(|&&c| c == '.').count(), 8);
    assert_eq!(shots.iter().filter(|&&c| c == '|').count(), 1);
    // the aimed shot goes straight down at the player
    let aimed = g.bullets.iter().find(|b| b.glyph == '|').unwrap();
    assert!(aimed.dir.y < -0.999);
}

#[test]
fn the_player_fires_only_while_moving() {
    let mut g = Game::new();
    g.new_game(1);
    g.update((0, 0), GRID);
    assert_eq!(bullets(&g, true), 0);
    g.update((1, 0), GRID);
    g.update((1, 1), GRID);
    assert_eq!(bullets(&g, true), 2);
    let p = g.player.as_ref().unwrap();
    assert!(p.pos.x > FIELD_W / 2.0);
}

#[test]
fn the_player_stays_in_the_window() {
    let mut g = Game::new();
    g.new_game(1);
    for _ in 0..600 {
        g.update((-1, -1), GRID);
        if g.state != State::Playing {
            break;
        }
    }
    if let Some(p) = &g.player {
        assert_eq!((p.pos.x, p.pos.y), (4.0, 4.0));
    }
}

#[test]
fn the_sixth_hit_ends_the_game() {
    let mut g = Game::new();
    g.new_game(1);
    g.boss.as_mut().unwrap().patterns = &[];
    for hit in 1..=6 {
        let p = g.player.as_ref().unwrap().pos;
        g.bullets = vec![Bullet {
            pos: p,
            prev: p,
            dir: Vec2::new(0.0, 1.0),
            speed: 0.0,
            damage: 2.0,
            lifetime: 10.0,
            size: 1.0,
            glyph: 'o',
            from_player: false,
        }];
        g.update((0, 0), GRID);
        if hit < 6 {
            assert_eq!(g.state, State::Playing, "hit {hit}");
            assert!(g.bullets.is_empty());
        }
    }
    assert_eq!(g.state, State::GameOver);
    assert!(g.player.is_none() && g.bullets.is_empty());
}

#[test]
fn the_boss_falls_after_751_hits_and_the_tenth_wins() {
    for level in [1, 10] {
        let mut g = Game::new();
        g.new_game(level);
        let mut frames = 0;
        // stand right under the boss and wiggle, so every shot lands at once
        g.player.as_mut().unwrap().pos = Vec2::new(FIELD_W / 2.0, FIELD_H / 2.0 - 30.0);
        let mut hits = 0;
        while g.state == State::Playing && frames < 5000 {
            let before = g.boss.as_ref().unwrap().pool.health;
            // god mode for the test: no boss bullets
            g.bullets.retain(|b| b.from_player);
            g.update(if frames % 2 == 0 { (1, 0) } else { (-1, 0) }, GRID);
            if let Some(b) = &g.boss {
                hits += ((before - b.pool.health) / 2.0).round() as i32;
            } else {
                hits += 1;
            }
            frames += 1;
        }
        assert_eq!(hits, 751, "level {level}");
        assert_eq!(g.state, if level == 10 { State::WinGame } else { State::FinishedLevel });
    }
}

#[test]
fn every_boss_runs_a_minute_without_trouble() {
    for level in 1..=10 {
        let mut g = Game::new();
        g.new_game(level);
        for f in 0..3600 {
            let dir = [(1, 0), (0, 1), (-1, 0), (0, -1)][(f / 30) % 4];
            g.update(dir, GRID);
            if g.state != State::Playing {
                break;
            }
            assert!(g.bullets.iter().all(|b| b.pos.x.is_finite() && b.pos.y.is_finite()));
        }
        let def = &BOSSES[level as usize - 1];
        assert!(def.patterns.len() >= 2, "{}", def.name);
    }
}

#[test]
fn each_slot_is_used_at_most_once_per_boss() {
    for def in &BOSSES {
        for s in Slot::ALL {
            assert!(def.patterns.iter().filter(|p| p.slot == s).count() <= 1, "{}", def.name);
        }
    }
}

#[test]
fn a_fast_bullet_cannot_skip_the_players_cell() {
    let mut g = Game::new();
    g.new_game(9);
    let grid = Grid { cols: 300, rows: 90 };
    let p = g.player.as_ref().unwrap().pos;
    let mut b = { g.update((0, 0), grid); g.bullets.iter().find(|b| !b.from_player).unwrap().clone() };
    // 600 px/s and passing straight through the player's spot this frame
    b.prev = Vec2::new(p.x, p.y + 6.0);
    b.pos = b.prev;
    b.dir = Vec2::new(0.0, -1.0);
    b.speed = 600.0;
    g.bullets = vec![b];
    let before = g.player.as_ref().unwrap().pool.health;
    g.boss.as_mut().unwrap().patterns = &[];
    g.update((0, 0), grid);
    assert!(g.player.as_ref().unwrap().pool.health < before);
}

#[test]
fn the_field_keeps_the_window_shape() {
    let f = render::field_rect(Rect::new(0, 0, 100, 30), 2.0);
    assert_eq!((f.width, f.height), (93, 29));
    let f = render::field_rect(Rect::new(0, 0, 80, 40), 2.0);
    assert_eq!(f.width, 80);
    assert!(f.height == 25);
    let g = render::grid_of(f);
    assert_eq!(g.cell(Vec2::new(0.0, FIELD_H)), (0, 0));
    assert_eq!(g.cell(Vec2::new(FIELD_W - 0.01, 0.01)), (79, 24));
}

/// A greedy dodger: each frame, the move whose spot is furthest from the
/// bullets over the next few frames, moving whenever it is safe (moving
/// fires). Prints how far it gets; a check that every boss can be beaten.
#[test]
#[ignore]
fn bot_report() {
    let moves: [(i8, i8); 9] = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
    for level in 1..=10 {
        let mut g = Game::new();
        g.new_game(level);
        let mut frames = 0;
        while g.state == State::Playing && frames < 60 * 180 {
            let p = g.player.as_ref().unwrap().pos;
            let boss = g.boss.as_ref().unwrap().pos;
            let mut best = (f32::MIN, (0, 0));
            for &m in &moves {
                let d = if m == (0, 0) { Vec2::new(0.0, 0.0) } else {
                    let l = ((m.0 as f32).powi(2) + (m.1 as f32).powi(2)).sqrt();
                    Vec2::new(m.0 as f32 / l, m.1 as f32 / l)
                };
                let mut score = 0.0f32;
                for k in [1.0f32, 4.0, 8.0] {
                    let q = Vec2::new((p.x + d.x * 250.0 * k / 60.0).clamp(4.0, FIELD_W - 4.0), (p.y + d.y * 250.0 * k / 60.0).clamp(4.0, FIELD_H - 4.0));
                    let mut near = f32::MAX;
                    for b in g.bullets.iter().filter(|b| !b.from_player) {
                        let bx = b.pos.x + b.dir.x * b.speed * k / 60.0;
                        let by = b.pos.y + b.dir.y * b.speed * k / 60.0;
                        let dx = (bx - q.x) / 13.5;
                        let dy = (by - q.y) / 27.0;
                        near = near.min(dx * dx + dy * dy);
                    }
                    score += near.min(9.0) / k;
                }
                // keep a distance from the boss and from the walls
                let q = Vec2::new(p.x + d.x * 4.0, p.y + d.y * 4.0);
                let db = ((q.x - boss.x).powi(2) + (q.y - boss.y).powi(2)).sqrt();
                score -= ((250.0 - db).max(0.0) / 250.0) * 2.0;
                score -= if q.x < 60.0 || q.x > FIELD_W - 60.0 || q.y < 60.0 || q.y > FIELD_H - 60.0 { 1.0 } else { 0.0 };
                if m != (0, 0) {
                    score += 0.3;
                }
                if score > best.0 {
                    best = (score, m);
                }
            }
            g.update(best.1, GRID);
            frames += 1;
        }
        let hp = g.player.as_ref().map_or(-1.0, |p| p.pool.health);
        let boss = g.boss.as_ref().map_or(0.0, |b| b.pool.health);
        println!("level {level:2} {:?} after {:.1}s, player {hp}, boss {boss}", g.state, frames as f32 / 60.0);
    }
}
