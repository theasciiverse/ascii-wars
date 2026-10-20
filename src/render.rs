//! Drawing: the field keeps the original window's shape in whatever room
//! the terminal has, with the HUD on the line above it. The original's
//! look is white glyphs on dark grey; its CRT shader is left out.

use crate::bosses;
use crate::game::{FIELD_H, FIELD_W, Game, Grid, State};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

pub const BG: Color = Color::Rgb(26, 26, 26);
pub const FG: Color = Color::Rgb(255, 255, 255);
pub const MIN_W: u16 = 40;
pub const MIN_H: u16 = 13;

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Play,
    Continue,
    NextLevel,
    PlayAgain,
    Exit,
}

pub struct Menu {
    pub title: &'static str,
    pub items: &'static [(&'static str, Action)],
    /// Box size as a share of the field, as the original's.
    pub w_pct: u16,
    pub h_pct: u16,
}

/// The original's five menus.
pub fn menu_for(state: State) -> Option<Menu> {
    use Action::*;
    let m = |title, items, w_pct, h_pct| Some(Menu { title, items, w_pct, h_pct });
    match state {
        State::MainMenu => m("A S C I I   W A R S !", &[("Play", Play), ("Exit", Exit)], 60, 80),
        State::PauseMenu => m("Pause!", &[("Continue", Continue), ("Exit", Exit)], 40, 60),
        State::GameOver => m("Game Over!", &[("Play Again", PlayAgain), ("Exit", Exit)], 40, 60),
        State::FinishedLevel => m("Completed!", &[("Continue to next level", NextLevel)], 40, 40),
        State::WinGame => m("You Won!", &[("Play Again", PlayAgain), ("Exit", Exit)], 40, 60),
        State::Playing => None,
    }
}

/// What the main loop needs to know about the last frame drawn.
#[derive(Default)]
pub struct Drawn {
    pub field: Option<Rect>,
    pub buttons: Vec<Rect>,
}

pub struct View<'a> {
    pub selected: usize,
    pub aspect: f32,
    /// A line under the main menu, e.g. that key releases are not reported.
    pub notice: Option<&'a str>,
}

pub fn draw(f: &mut Frame, game: &Game, view: &View) -> Drawn {
    let area = f.area();
    let buf = f.buffer_mut();
    let mut drawn = Drawn::default();
    if area.width < MIN_W || area.height < MIN_H {
        let msg = format!("ASCII Wars! needs {MIN_W}x{MIN_H} or more");
        put_str(buf, area, 0, area.height / 2, &msg, Style::new());
        return drawn;
    }
    let field = field_rect(area, view.aspect);
    drawn.field = Some(field);
    buf.set_style(field, Style::new().bg(BG).fg(FG));
    let grid = grid_of(field);

    // player bullets under the boss's, the boss and the player on top
    let mut plot = |p, ch: char, style: Style| {
        let (c, r) = grid.cell(p);
        if c >= 0 && r >= 0 && c < field.width as i32 && r < field.height as i32 {
            buf[(field.x + c as u16, field.y + r as u16)].set_char(ch).set_style(style);
        }
    };
    let plain = Style::new().fg(FG).bg(BG);
    let bold = plain.add_modifier(Modifier::BOLD);
    for b in game.bullets.iter().filter(|b| b.from_player) {
        plot(b.pos, b.glyph, plain);
    }
    for b in game.bullets.iter().filter(|b| !b.from_player) {
        plot(b.pos, b.glyph, if b.bullet_big() { bold } else { plain });
    }
    if let Some(boss) = &game.boss {
        plot(boss.pos, boss.glyph, bold);
    }
    if let Some(p) = &game.player {
        // the original turns a '>' sprite towards the boss
        let a = p.facing.y.atan2(p.facing.x).to_degrees();
        let ch = match a {
            a if (-45.0..45.0).contains(&a) => '>',
            a if (45.0..135.0).contains(&a) => '^',
            a if (-135.0..-45.0).contains(&a) => 'v',
            _ => '<',
        };
        plot(p.pos, ch, bold);
    }

    if game.state != State::MainMenu {
        draw_hud(buf, field, game);
    }
    if let Some(menu) = menu_for(game.state) {
        drawn.buttons = draw_menu(buf, field, &menu, view.selected);
        if game.state == State::MainMenu {
            let hint = "move: arrows or WASD (moving fires)   pause: Esc";
            put_centred(buf, field, field.y + field.height - 2, hint, plain);
            if let Some(n) = view.notice {
                put_centred(buf, field, field.y + field.height - 1, n, plain.add_modifier(Modifier::DIM));
            }
        }
    }
    drawn
}

/// The health bar, a quarter of the width, and the level; the boss's name
/// is ours.
fn draw_hud(buf: &mut Buffer, field: Rect, game: &Game) {
    let y = field.y - 1;
    let style = Style::new().fg(FG);
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
    put_str(buf, Rect::new(lx, y, field.x + field.width - lx, 1), 0, 0, &label, style);
    if game.level > 0 {
        let name = bosses::boss(game.level).name;
        let nx = (field.x + field.width).saturating_sub(name.chars().count() as u16);
        if nx > lx + label.len() as u16 + 1 {
            put_str(buf, Rect::new(nx, y, name.len() as u16, 1), 0, 0, name, style);
        }
    }
}

/// The box, its title and its buttons; returns the buttons' rectangles.
fn draw_menu(buf: &mut Buffer, field: Rect, menu: &Menu, selected: usize) -> Vec<Rect> {
    let n = menu.items.len() as u16;
    let need_h = 2 + 3 + n * 4;
    let longest = menu.items.iter().map(|(l, _)| l.len()).max().unwrap_or(0) as u16;
    let need_w = (longest + 6).max(menu.title.len() as u16 + 4);
    let w = (field.width * menu.w_pct / 100).max(need_w).min(field.width);
    let h = (field.height * menu.h_pct / 100).max(need_h).min(field.height);
    let bx = Rect::new(field.x + (field.width - w) / 2, field.y + (field.height - h) / 2, w, h);
    let style = Style::new().fg(FG).bg(BG);
    frame_box(buf, bx, style);

    // title a tenth of the way down, buttons spread over the rest
    let ty = bx.y + 1 + (h - 2) / 10;
    put_centred(buf, bx, ty, menu.title, style.add_modifier(Modifier::BOLD));
    let bw = (w * 60 / 100).max(longest + 4).min(w - 2);
    let room = bx.y + h - 1 - (ty + 2);
    let gap = room.saturating_sub(n * 3) / (n + 1);
    let mut y = ty + 2 + gap;
    let mut rects = Vec::new();
    for (i, (label, _)) in menu.items.iter().enumerate() {
        let r = Rect::new(bx.x + (w - bw) / 2, y, bw, 3);
        let hot = i == selected;
        let s = if hot { Style::new().fg(BG).bg(FG) } else { style };
        buf.set_style(r, s);
        frame_box(buf, r, s);
        put_centred(buf, r, r.y + 1, label, s);
        rects.push(r);
        y += 3 + gap;
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

fn put_centred(buf: &mut Buffer, within: Rect, y: u16, s: &str, style: Style) {
    let len = s.chars().count() as u16;
    let x = within.width.saturating_sub(len) / 2;
    put_str(buf, within, x, y - within.y, s, style);
}

fn put_str(buf: &mut Buffer, within: Rect, dx: u16, dy: u16, s: &str, style: Style) {
    let y = within.y + dy;
    for (i, ch) in s.chars().enumerate() {
        let x = within.x + dx + i as u16;
        if x >= within.x + within.width {
            break;
        }
        buf[(x, y)].set_char(ch).set_style(style);
    }
}
