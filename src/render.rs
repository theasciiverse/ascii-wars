//! Drawing: the field keeps the original window's shape in whatever room
//! the terminal has, with the HUD on the line above it. Classic keeps the
//! original's look, white glyphs on dark grey; Arcade adds colour, stars,
//! the boss's art, beams, sparks and banners. The original's CRT shader is
//! left out.

use crate::bosses::Rgb;
use crate::game::{FIELD_H, FIELD_W, Game, Grid, State, Vec2};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use std::f32::consts::{PI, TAU};

pub const CLASSIC_BG: Rgb = (26, 26, 26);
pub const BG: Rgb = (10, 10, 22);
pub const FG: Rgb = (255, 255, 255);
pub const PLAYER: Rgb = (120, 230, 255);
pub const MIN_W: u16 = 40;
pub const MIN_H: u16 = 13;

pub fn rgb(c: Rgb) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

/// `c` faded towards `to` by `k` (0 none, 1 all the way).
pub fn mix(c: Rgb, to: Rgb, k: f32) -> Rgb {
    let k = k.clamp(0., 1.);
    let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * k).round() as u8;
    (m(c.0, to.0), m(c.1, to.1), m(c.2, to.2))
}

/// Height over width of a terminal cell, from the window's pixel size
/// where the terminal reports it, else 2.
pub fn cell_aspect() -> f32 {
    match ratatui::crossterm::terminal::window_size() {
        Ok(w) if w.width > 0 && w.height > 0 && w.columns > 0 && w.rows > 0 => {
            let a = (w.height as f32 / w.rows as f32) / (w.width as f32 / w.columns as f32);
            if (1.0..4.0).contains(&a) { a } else { 2.0 }
        }
        _ => 2.0,
    }
}

/// Where the field goes: the biggest rectangle of the original's shape
/// below the HUD line, centred.
pub fn field_rect(area: Rect, aspect: f32) -> Rect {
    let ratio = FIELD_W / FIELD_H;
    let avail_h = area.height.saturating_sub(1).max(1);
    let mut rows = avail_h;
    let mut cols = (ratio * rows as f32 * aspect).round() as u16;
    if cols > area.width {
        cols = area.width;
        rows = ((cols as f32 / (ratio * aspect)).round() as u16).clamp(1, avail_h);
    }
    let x = area.x + (area.width - cols) / 2;
    let y = area.y + 1 + (avail_h - rows) / 2;
    Rect::new(x, y, cols.max(1), rows.max(1))
}

pub fn grid_of(field: Rect) -> Grid {
    Grid { cols: field.width, rows: field.height }
}

/// A box of text and choices over the field: every menu and results page.
pub struct Panel {
    pub title: String,
    pub color: Rgb,
    pub lines: Vec<(String, Rgb)>,
    pub items: Vec<String>,
    pub selected: usize,
    /// Box size as a share of the field, as the original's.
    pub w_pct: u16,
    pub h_pct: u16,
    /// Row of the field the box starts on, else centred.
    pub top: Option<u16>,
    /// The original's tall bordered buttons, else one line each.
    pub buttons: bool,
}

/// What the main loop needs to know about the last frame drawn.
#[derive(Default)]
pub struct Drawn {
    pub field: Option<Rect>,
    pub buttons: Vec<Rect>,
}

pub struct View<'a> {
    pub panel: Option<Panel>,
    pub logo: bool,
    pub aspect: f32,
    pub shake: bool,
    pub hi: u64,
    /// Lines along the bottom of the field.
    pub footer: Vec<&'a str>,
}

pub fn draw(f: &mut Frame, game: &Game, view: &View) -> Drawn {
    let area = f.area();
    let buf = f.buffer_mut();
    let mut drawn = Drawn::default();
    if area.width < MIN_W || area.height < MIN_H {
        let msg = format!("ASCII Wars! needs {MIN_W}x{MIN_H} or more");
        put(buf, area, 0, area.height / 2, &msg, Style::new());
        return drawn;
    }
    let field = field_rect(area, view.aspect);
    drawn.field = Some(field);
    let grid = grid_of(field);
    let classic = game.classic() && game.state != State::Title;
    let bg = if classic {
        CLASSIC_BG
    } else if game.flash > 0 {
        mix(BG, (110, 10, 20), game.flash as f32 / 8.)
    } else {
        BG
    };
    buf.set_style(field, Style::new().bg(rgb(bg)).fg(rgb(FG)));

    // the scene draws through a shaken view of the field
    let (sx, sy) = if view.shake && game.shake > 0 && !game.paused {
        let k = (game.shake as i32).min(6) / 3 + 1;
        let f = game.frames as i32;
        (((f * 7) % 3 - 1) * k.min(2), ((f * 5) % 3 - 1) * (k / 2))
    } else {
        (0, 0)
    };
    let mut scene = Scene { buf, field, grid, bg, sx, sy };

    if !classic {
        for s in &game.stars {
            // dots unlike any bullet glyph, so a star never looks like a shot
            let (ch, c) = match s.layer {
                0 => ('·', (40, 40, 64)),
                1 => ('·', (70, 70, 105)),
                _ => ('·', (110, 110, 150)),
            };
            scene.plot(s.pos, ch, Style::new().fg(rgb(c)), false);
        }
        draw_lasers(&mut scene, game);
        if let Some((at, r, _)) = game.wave {
            let (cw, _) = grid.cell_px();
            let n = ((TAU * r / (cw * 0.7)) as usize).max(24);
            let c = mix((170, 240, 255), bg, r / 900.);
            for i in 0..n {
                let a = i as f32 / n as f32 * TAU;
                scene.plot(at.add_scaled(Vec2::from_angle(a), r), if i % 2 == 0 { 'o' } else { '*' }, Style::new().fg(rgb(c)).add_modifier(Modifier::BOLD), false);
            }
        }
    }

    for b in game.bullets.iter().filter(|b| b.from_player) {
        let c = if classic { FG } else { mix(b.color, bg, 0.45) };
        scene.plot(b.pos, b.glyph, Style::new().fg(rgb(c)), false);
    }
    for b in game.bullets.iter().filter(|b| !b.from_player) {
        let mut s = Style::new().fg(rgb(b.color));
        if b.bullet_big() {
            s = s.add_modifier(Modifier::BOLD);
        }
        scene.plot(b.pos, b.glyph, s, false);
    }
    if let Some(boss) = &game.boss {
        let flash = boss.flash > 0 || (boss.invuln > 0. && (game.frames / 4) % 2 == 0);
        let color = if flash { (255, 255, 255) } else { boss.color };
        let s = Style::new().fg(rgb(color)).add_modifier(Modifier::BOLD);
        if boss.art.is_empty() {
            scene.plot(boss.pos, boss.glyph, s, false);
        } else if let Some((x, y, _, _)) = game.boss_rect(grid) {
            for (r, line) in boss.art.iter().enumerate() {
                for (c, ch) in line.chars().enumerate() {
                    if ch != ' ' {
                        scene.cell((x + c as i32, y + r as i32), ch, s, false);
                    }
                }
            }
        }
    }
    for p in &game.particles {
        let k = 1. - p.life / p.max;
        scene.plot(p.pos, p.glyph, Style::new().fg(rgb(mix(p.color, bg, k * 0.9))), false);
    }
    if let Some(p) = &game.player {
        let blink = p.iframes > 0. && (game.frames / 4) % 2 == 0;
        if !blink {
            // the original turns a '>' sprite towards the boss
            let a = p.facing.y.atan2(p.facing.x).to_degrees();
            let ch = match a {
                a if (-45.0..45.0).contains(&a) => '>',
                a if (45.0..135.0).contains(&a) => '^',
                a if (-135.0..-45.0).contains(&a) => 'v',
                _ => '<',
            };
            let mut s = Style::new().add_modifier(Modifier::BOLD);
            s = if classic { s.fg(rgb(FG)) } else { s.fg(rgb(PLAYER)) };
            if p.focus && !classic {
                s = s.bg(rgb((30, 70, 95)));
            }
            scene.plot(p.pos, ch, s, true);
        }
    }
    for p in &game.popups {
        let c = mix(p.color, bg, 1. - p.life / 1.4);
        let (col, row) = grid.cell(p.pos);
        let len = p.text.chars().count() as i32;
        for (i, ch) in p.text.chars().enumerate() {
            scene.cell((col - len / 2 + i as i32, row), ch, Style::new().fg(rgb(c)).add_modifier(Modifier::BOLD), true);
        }
    }

    let buf = scene.buf;
    if !classic && game.state != State::Title {
        draw_boss_bar(buf, field, game);
        if let Some(b) = &game.banner {
            draw_banner(buf, field, b, bg, game.frames);
        }
    }
    if game.state != State::Title {
        if classic {
            draw_classic_hud(buf, field, game);
        } else {
            draw_hud(buf, field, game, view.hi);
        }
    }
    if view.logo {
        draw_logo(buf, field, game.frames);
    }
    for (i, line) in view.footer.iter().rev().enumerate() {
        let y = field.y + field.height - 1 - i as u16;
        centred(buf, field, y, line, Style::new().fg(rgb((150, 150, 175))));
    }
    if let Some(p) = &view.panel {
        drawn.buttons = draw_panel(buf, field, p, classic);
    }
    drawn
}

struct Scene<'a> {
    buf: &'a mut Buffer,
    field: Rect,
    grid: Grid,
    bg: Rgb,
    sx: i32,
    sy: i32,
}

impl Scene<'_> {
    fn plot(&mut self, p: Vec2, ch: char, style: Style, keep_bg: bool) {
        let c = self.grid.cell(p);
        self.cell(c, ch, style, keep_bg);
    }

    fn cell(&mut self, (c, r): (i32, i32), ch: char, style: Style, keep_bg: bool) {
        let (c, r) = (c + self.sx, r + self.sy);
        if c >= 0 && r >= 0 && c < self.field.width as i32 && r < self.field.height as i32 {
            let cell = &mut self.buf[(self.field.x + c as u16, self.field.y + r as u16)];
            cell.set_char(ch).set_style(style);
            if !keep_bg || style.bg.is_none() {
                cell.set_bg(rgb(self.bg));
            }
        }
    }
}

fn draw_lasers(scene: &mut Scene, game: &Game) {
    let Some(boss) = &game.boss else { return };
    for l in &game.lasers {
        let cells = scene.grid.beam(boss.pos, l.angle);
        if l.burning() {
            // pick the stroke from the beam's direction on screen
            let a = l.angle.rem_euclid(PI);
            let ch = if !(PI / 8. ..PI * 7. / 8.).contains(&a) {
                '-'
            } else if a < PI * 3. / 8. {
                '/'
            } else if a < PI * 5. / 8. {
                '|'
            } else {
                '\\'
            };
            let left = l.warn + l.beam - l.t;
            let k = (left / 0.25).min(1.0);
            let core = mix((255, 255, 255), l.color, 0.3);
            let s = Style::new().fg(rgb(mix(core, scene.bg, 1. - k))).bg(rgb(mix(l.color, scene.bg, 1. - 0.45 * k))).add_modifier(Modifier::BOLD);
            for c in cells {
                scene.cell(c, ch, s, true);
            }
        } else {
            let late = l.warn - l.t < 0.35;
            let on = !late || (game.frames / 3) % 2 == 0;
            if !on {
                continue;
            }
            let c = mix(l.color, scene.bg, if late { 0.1 } else { 0.55 });
            for (i, cell) in cells.into_iter().enumerate() {
                if late || i % 2 == 0 {
                    scene.cell(cell, if late { ':' } else { '.' }, Style::new().fg(rgb(c)), false);
                }
            }
        }
    }
}

/// The boss's health along the field's top row, with a pip per phase.
fn draw_boss_bar(buf: &mut Buffer, field: Rect, game: &Game) {
    let Some(b) = &game.boss else { return };
    let y = field.y;
    let name = b.name;
    let style = Style::new().fg(rgb(b.color)).bg(rgb(BG)).add_modifier(Modifier::BOLD);
    put(buf, field, 1, 0, name, style);
    let pips = b.phases as u16;
    let x0 = field.x + 2 + name.chars().count() as u16;
    let x1 = field.x + field.width - 2 - pips * 2;
    if x1 <= x0 + 4 {
        return;
    }
    let width = x1 - x0;
    let k = (b.pool.health / b.pool.max_health).clamp(0., 1.);
    let full = (k * width as f32).round() as u16;
    for i in 0..width {
        let (ch, c) = if i < full { ('=', b.color) } else { ('-', mix(b.color, BG, 0.7)) };
        buf[(x0 + i, y)].set_char(ch).set_style(Style::new().fg(rgb(c)).bg(rgb(BG)));
    }
    for i in 0..pips {
        let done = (i as usize) < b.phase;
        let c = if done { mix(b.color, BG, 0.7) } else { b.color };
        buf[(x1 + 1 + i * 2, y)].set_char(if done { 'o' } else { '@' }).set_style(Style::new().fg(rgb(c)).bg(rgb(BG)));
    }
}

fn draw_banner(buf: &mut Buffer, field: Rect, b: &crate::game::Banner, bg: Rgb, frames: u64) {
    let k = (b.t / 0.3).min(1.0).min(((b.dur - b.t) / 0.4).max(0.));
    let warning = b.title.starts_with("WARNING");
    if warning && (frames / 8) % 2 == 1 && b.t < 1.6 {
        return;
    }
    let y = field.y + field.height / 3;
    let c = mix(b.color, bg, 1. - k);
    let style = Style::new().fg(rgb(c)).add_modifier(Modifier::BOLD);
    if warning {
        let bar: String = "/".repeat(field.width as usize);
        let s = Style::new().fg(rgb(mix((200, 40, 40), bg, 1. - k * 0.8)));
        put(buf, field, 0, y - field.y - 1, &bar, s);
        put(buf, field, 0, y - field.y + 1, &bar, s);
    }
    centred(buf, field, y, &b.title, style);
    if !b.sub.is_empty() {
        let row = if warning { y + 3 } else { y + 1 };
        centred(buf, field, row, &b.sub, Style::new().fg(rgb(mix((200, 200, 220), bg, 1. - k))));
    }
}

/// The original's HUD: a health bar, a quarter of the width, and the
/// level; the boss's name is ours.
fn draw_classic_hud(buf: &mut Buffer, field: Rect, game: &Game) {
    let y = field.y - 1;
    let style = Style::new().fg(rgb(FG));
    let inner = (field.width / 4).saturating_sub(2).max(4);
    let full = (game.hud_health * inner as f32).round() as u16;
    let mut x = field.x;
    buf[(x, y)].set_char('[').set_style(style);
    for i in 0..inner {
        x += 1;
        buf[(x, y)].set_char(if i < full { '█' } else { ' ' }).set_style(style);
    }
    buf[(x + 1, y)].set_char(']').set_style(style);
    let label = format!("level: {}", game.level);
    let lx = field.x + field.width * 47 / 100;
    put(buf, Rect::new(lx, y, field.x + field.width - lx, 1), 0, 0, &label, style);
    if let Some(b) = &game.boss {
        let nx = (field.x + field.width).saturating_sub(b.name.chars().count() as u16);
        if nx > lx + label.len() as u16 + 1 {
            put(buf, Rect::new(nx, y, b.name.len() as u16, 1), 0, 0, b.name, style);
        }
    }
}

/// Hearts (two health each), bombs, score, best, graze and difficulty.
fn draw_hud(buf: &mut Buffer, field: Rect, game: &Game, hi: u64) {
    let y = field.y - 1;
    let mut x = field.x;
    let mut put_s = |buf: &mut Buffer, s: &str, c: Rgb, bold: bool| {
        let mut st = Style::new().fg(rgb(c));
        if bold {
            st = st.add_modifier(Modifier::BOLD);
        }
        for ch in s.chars() {
            if x >= field.x + field.width {
                return;
            }
            buf[(x, y)].set_char(ch).set_style(st);
            x += 1;
        }
    };
    let health = game.player.as_ref().map_or(0., |p| p.pool.health.max(0.));
    let hearts = (health / 2.).floor() as usize;
    let low = hearts == 0 && game.player.is_some();
    let heart_c = if low && (game.frames / 10) % 2 == 0 { (255, 255, 255) } else { (255, 80, 110) };
    for i in 0..5 {
        if i < hearts {
            put_s(buf, "♥", heart_c, true);
        } else {
            put_s(buf, "♡", (110, 60, 75), false);
        }
    }
    put_s(buf, "  ", FG, false);
    for i in 0..5 {
        if (i as i32) < game.bombs {
            put_s(buf, "◆", (120, 220, 255), true);
        } else if i < 3 {
            put_s(buf, "◇", (60, 90, 110), false);
        }
    }
    put_s(buf, &format!("  SCORE {:09}", game.score), FG, true);
    if field.width >= 70 {
        put_s(buf, &format!("  HI {:09}", hi.max(game.score)), (160, 160, 190), false);
    }
    let right = match game.mode {
        crate::game::Mode::Practice => format!("PRACTICE {}", game.difficulty.name().to_uppercase()),
        _ => format!("GRAZE {}  {}", game.graze, game.difficulty.name().to_uppercase()),
    };
    let rx = (field.x + field.width).saturating_sub(right.chars().count() as u16);
    if rx > x + 1 {
        put(buf, Rect::new(rx, y, right.chars().count() as u16, 1), 0, 0, &right, Style::new().fg(rgb((170, 170, 200))));
    }
}

/// The title in big block letters with a shadow.
const LOGO_FONT: [(char, [&str; 5]); 8] = [
    ('A', [" ### ", "#   #", "#####", "#   #", "#   #"]),
    ('S', [" ####", "#    ", " ### ", "    #", "#### "]),
    ('C', [" ####", "#    ", "#    ", "#    ", " ####"]),
    ('I', ["###", " # ", " # ", " # ", "###"]),
    ('W', ["#   #", "#   #", "# # #", "## ##", "#   #"]),
    ('R', ["#### ", "#   #", "#### ", "#  # ", "#   #"]),
    ('!', ["#", "#", "#", " ", "#"]),
    (' ', ["  ", "  ", "  ", "  ", "  "]),
];

fn logo_rows(text: &str) -> [String; 5] {
    let mut rows: [String; 5] = Default::default();
    for ch in text.chars() {
        let glyph = LOGO_FONT.iter().find(|(c, _)| *c == ch).map(|(_, g)| g).unwrap_or(&LOGO_FONT[7].1);
        for (r, row) in rows.iter_mut().enumerate() {
            row.push_str(glyph[r]);
            row.push(' ');
        }
    }
    rows
}

fn draw_logo(buf: &mut Buffer, field: Rect, frames: u64) {
    let rows = logo_rows("ASCII WARS!");
    let w = rows[0].chars().count() as u16;
    if w + 2 > field.width {
        centred(buf, field, field.y + 2, "A S C I I   W A R S !", Style::new().fg(rgb(PLAYER)).add_modifier(Modifier::BOLD));
        return;
    }
    let x0 = field.x + (field.width - w) / 2;
    let y0 = field.y + 2;
    let colors = [(255, 95, 45), (255, 180, 80), (255, 225, 120), (120, 230, 255), (120, 170, 255)];
    for (r, row) in rows.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            if ch == '#' {
                let shadow = (x0 + c as u16 + 1, y0 + r as u16 + 1);
                if shadow.0 < field.x + field.width && shadow.1 < field.y + field.height {
                    buf[shadow].set_char('░').set_style(Style::new().fg(rgb((60, 50, 90))));
                }
            }
        }
    }
    let sweep = (frames / 2) as i32 % (w as i32 + 40) - 20;
    for (r, row) in rows.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            if ch == '#' {
                let near = (c as i32 - sweep - r as i32).abs() < 3;
                let col = if near { (255, 255, 255) } else { colors[r] };
                buf[(x0 + c as u16, y0 + r as u16)].set_char('█').set_style(Style::new().fg(rgb(col)));
            }
        }
    }
    centred(buf, field, y0 + 6, "a boss rush bullet hell", Style::new().fg(rgb((170, 170, 200))));
}

/// A panel: its box, title, lines and choices; returns the choices'
/// rectangles for the mouse.
fn draw_panel(buf: &mut Buffer, field: Rect, p: &Panel, classic: bool) -> Vec<Rect> {
    let n = p.items.len() as u16;
    let item_h = if p.buttons { 4 } else { 1 };
    let need_h = 4 + p.lines.len() as u16 + n * item_h + if p.lines.is_empty() { 0 } else { 1 };
    let longest = p.items.iter().map(|l| l.chars().count()).chain(p.lines.iter().map(|(l, _)| l.chars().count())).max().unwrap_or(0) as u16;
    let need_w = (longest + 6).max(p.title.chars().count() as u16 + 4);
    let w = (field.width * p.w_pct / 100).max(need_w).min(field.width);
    let h = (field.height * p.h_pct / 100).max(need_h).min(field.height);
    let top = match p.top {
        Some(t) => (field.y + t).min(field.y + field.height - h),
        None => field.y + (field.height - h) / 2,
    };
    let bx = Rect::new(field.x + (field.width - w) / 2, top, w, h);
    let bgc = if classic { CLASSIC_BG } else { (16, 16, 32) };
    let edge = if classic { FG } else { mix(p.color, bgc, 0.35) };
    let style = Style::new().fg(rgb(FG)).bg(rgb(bgc));
    buf.set_style(bx, style);
    frame_box(buf, bx, style.fg(rgb(edge)));

    let mut y = bx.y + 1 + if p.buttons { (h - 2) / 10 } else { 0 };
    centred(buf, bx, y, &p.title, style.fg(rgb(p.color)).add_modifier(Modifier::BOLD));
    y += 2;
    for (line, c) in &p.lines {
        centred(buf, bx, y, line, style.fg(rgb(*c)));
        y += 1;
    }
    if !p.lines.is_empty() {
        y += 1;
    }
    let mut rects = Vec::new();
    if p.buttons {
        let bw = (w * 60 / 100).max(longest + 4).min(w - 2);
        let room = (bx.y + h - 1).saturating_sub(y);
        let gap = room.saturating_sub(n * 3) / (n + 1);
        y += gap;
        for (i, label) in p.items.iter().enumerate() {
            let r = Rect::new(bx.x + (w - bw) / 2, y, bw, 3);
            let hot = i == p.selected;
            let s = if hot { Style::new().fg(rgb(bgc)).bg(rgb(FG)) } else { style };
            buf.set_style(r, s);
            frame_box(buf, r, s);
            centred(buf, r, r.y + 1, label, s);
            rects.push(r);
            y += 3 + gap;
        }
    } else {
        let room = (bx.y + h - 1).saturating_sub(y);
        y += room.saturating_sub(n) / 2;
        let iw = (longest + 6).min(w - 2);
        for (i, label) in p.items.iter().enumerate() {
            let r = Rect::new(bx.x + (w - iw) / 2, y, iw, 1);
            let hot = i == p.selected;
            let s = if hot { Style::new().fg(rgb(bgc)).bg(rgb(p.color)).add_modifier(Modifier::BOLD) } else { style };
            buf.set_style(r, s);
            let text = if hot { format!("» {label} «") } else { label.clone() };
            centred(buf, r, y, &text, s);
            rects.push(r);
            y += 1;
        }
    }
    rects
}

fn frame_box(buf: &mut Buffer, r: Rect, style: Style) {
    for y in r.y..r.y + r.height {
        for x in r.x..r.x + r.width {
            let top = y == r.y;
            let bottom = y == r.y + r.height - 1;
            let left = x == r.x;
            let right = x == r.x + r.width - 1;
            let ch = match (top, bottom, left, right) {
                (true, _, true, _) => '┌',
                (true, _, _, true) => '┐',
                (_, true, true, _) => '└',
                (_, true, _, true) => '┘',
                (true, _, _, _) | (_, true, _, _) => '─',
                (_, _, true, _) | (_, _, _, true) => '│',
                _ => ' ',
            };
            buf[(x, y)].set_char(ch).set_style(style);
        }
    }
}

fn centred(buf: &mut Buffer, within: Rect, y: u16, s: &str, style: Style) {
    if y < within.y || y >= within.y + within.height {
        return;
    }
    let len = s.chars().count() as u16;
    let x = within.width.saturating_sub(len) / 2;
    put(buf, within, x, y - within.y, s, style);
}

fn put(buf: &mut Buffer, within: Rect, dx: u16, dy: u16, s: &str, style: Style) {
    let y = within.y + dy;
    for (i, ch) in s.chars().enumerate() {
        let x = within.x + dx + i as u16;
        if x >= within.x + within.width {
            break;
        }
        buf[(x, y)].set_char(ch).set_style(style);
    }
}
