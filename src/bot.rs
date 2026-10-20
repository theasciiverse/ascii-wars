//! A greedy dodger: each frame it tries every move, fast and focused,
//! looks a few frames ahead at the bullets and beams, and takes the safest,
//! moving whenever it can (moving fires). It plays the demo behind the
//! title and with `--demo`, and in the tests it checks every boss can be
//! beaten.

use crate::game::{FIELD_H, FIELD_W, Game, Grid, State, Vec2};

const MOVES: [(i8, i8); 9] = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];

pub struct Choice {
    pub mv: (i8, i8),
    pub focus: bool,
    pub bomb: bool,
}

pub fn choose(g: &Game, grid: Grid) -> Choice {
    let none = Choice { mv: (0, 0), focus: false, bomb: false };
    let Some(p) = &g.player else { return none };
    if !matches!(g.state, State::Playing | State::Intro | State::BossDying) {
        return none;
    }
    let (cw, ch) = grid.cell_px();
    let boss = g.boss.as_ref().map(|b| b.pos);
    let rect = g.boss_rect(grid);
    let beams: Vec<Vec<(i32, i32)>> = match boss {
        Some(bp) => g.lasers.iter().map(|l| grid.beam(bp, l.angle + l.sweep * 0.25)).collect(),
        None => Vec::new(),
    };
    let mut best = (f32::MIN, (0, 0), false);
    for &focus in &[false, true] {
        for &m in &MOVES {
            let speed = if focus { 110. } else { 250. };
            let d = if m == (0, 0) {
                Vec2::default()
            } else {
                let l = ((m.0 as f32).powi(2) + (m.1 as f32).powi(2)).sqrt();
                Vec2::new(m.0 as f32 / l, m.1 as f32 / l)
            };
            let mut score = 0.0f32;
            for k in [1.0f32, 3.0, 6.0, 12.0] {
                let q = Vec2::new((p.pos.x + d.x * speed * k / 60.).clamp(4., FIELD_W - 4.), (p.pos.y + d.y * speed * k / 60.).clamp(4., FIELD_H - 4.));
                let mut near = 9.0f32;
                for b in g.bullets.iter().filter(|b| !b.from_player) {
                    let bx = b.pos.x + b.dir.x * b.speed * k / 60.;
                    let by = b.pos.y + b.dir.y * b.speed * k / 60.;
                    let dx = (bx - q.x) / cw;
                    let dy = (by - q.y) / ch;
                    near = near.min(dx * dx + dy * dy);
                }
                let qc = grid.cell(q);
                for beam in &beams {
                    if beam.iter().any(|c| (c.0 - qc.0).abs() <= 1 && (c.1 - qc.1).abs() <= 1) {
                        near = near.min(if beam.contains(&qc) { 0. } else { 0.8 });
                    }
                }
                if let Some((x, y, w, h)) = rect {
                    if qc.0 >= x - 1 && qc.0 <= x + w && qc.1 >= y - 1 && qc.1 <= y + h {
                        near = 0.;
                    }
                }
                score += near.min(9.) / k.sqrt();
            }
            // stay low and away from the walls, keep moving to fire
            let q = p.pos.add_scaled(d, 6.);
            if let Some(bp) = boss {
                let db = q.sub(bp).len();
                score -= ((260. - db).max(0.) / 260.) * 3.;
            }
            if q.x < 80. || q.x > FIELD_W - 80. || q.y < 50. || q.y > FIELD_H * 0.6 {
                score -= 1.5;
            }
            score -= (q.y - FIELD_H * 0.2).abs() / FIELD_H;
            if m != (0, 0) {
                score += 0.4;
            }
            if focus {
                score -= 0.1;
            }
            if score > best.0 {
                best = (score, m, focus);
            }
        }
    }
    // nothing safe: bomb
    let bomb = best.0 < 0.6 && g.bombs > 0 && p.iframes <= 0.;
    Choice { mv: best.1, focus: best.2, bomb }
}
