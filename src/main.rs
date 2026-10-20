mod bosses;
mod game;
mod input;
mod render;
#[cfg(test)]
mod tests;

use game::{Game, State};
use input::{Cmd, Input};
use ratatui::crossterm::event::{
    self, DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture, Event, KeyboardEnhancementFlags, MouseButton,
    MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;
use ratatui::layout::Position;
use render::{Action, Drawn, View};
use std::io;
use std::time::{Duration, Instant};

const SNAPSHOT_EVERY: Duration = Duration::from_millis(500);

const HELP: &str = "ascii-wars [--level N]

ASCII Wars!, a boss rush bullet hell: ten bosses, one after another.

  arrows / WASD   move; you fire at the boss while you move
  Esc / P         pause
  Enter / Space   press the chosen button (the mouse works too)

  gamepad: d-pad or left stick to move, A to press, Start to pause

  --level N       start at boss N (1-10)

Best in a terminal that reports key releases (the kitty keyboard
protocol: kitty, foot, WezTerm, Ghostty, Alacritty, iTerm2 and others),
or with a gamepad.
Session log: RUST_LOG=debug ascii-wars 2>ascii-wars.log";

const NO_RELEASES: &str = "this terminal sends no key releases: moves run on a little";

fn main() -> io::Result<()> {
    env_logger::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return Ok(());
    }
    let start = args
        .iter()
        .position(|a| a == "--level")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<i32>().ok())
        .map(|n| n.clamp(1, game::MAX_LEVEL));

    let mut terminal = ratatui::init();
    let kitty = supports_keyboard_enhancement().unwrap_or(false);
    if kitty {
        let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
            | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;
        execute!(io::stdout(), PushKeyboardEnhancementFlags(flags))?;
    }
    let _ = execute!(io::stdout(), EnableFocusChange, EnableMouseCapture);
    let mut input = Input::new(kitty);
    input.open_pad();
    log::info!("start kitty_keyboard={kitty} pad={:?}", input.pad_name);

    let mut game = Game::new();
    if let Some(n) = start {
        game.new_game(n);
    }
    let result = run(&mut terminal, &mut game, &mut input);

    let _ = execute!(io::stdout(), DisableMouseCapture, DisableFocusChange);
    if kitty {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, game: &mut Game, input: &mut Input) -> io::Result<()> {
    let mut aspect = render::cell_aspect();
    log::info!("cell aspect {aspect:.3}");
    let mut drawn = Drawn::default();
    let mut selected = 0usize;
    let mut shown = game.state;
    let mut clicked: Option<usize> = None;
    let mut next = Instant::now();
    let mut snapshot = Instant::now();
    loop {
        // input until the next frame is due
        loop {
            let wait = next.saturating_duration_since(Instant::now());
            if !event::poll(wait)? {
                break;
            }
            match event::read()? {
                Event::Key(k) => {
                    log::debug!("key {:?} {:?} {:?}", k.code, k.kind, k.modifiers);
                    input.on_key(k, Instant::now());
                }
                Event::Mouse(m) => {
                    let at = Position::new(m.column, m.row);
                    let over = drawn.buttons.iter().position(|r| r.contains(at));
                    match m.kind {
                        MouseEventKind::Moved => {
                            if let Some(i) = over {
                                selected = i;
                            }
                        }
                        MouseEventKind::Down(MouseButton::Left) => {
                            log::debug!("click {},{} on button {over:?}", m.column, m.row);
                            if over.is_some() {
                                clicked = over;
                            }
                        }
                        _ => {}
                    }
                }
                Event::FocusLost => {
                    log::debug!("focus lost: releasing all keys");
                    input.release_all();
                }
                Event::Resize(w, h) => {
                    aspect = render::cell_aspect();
                    log::info!("resize {w}x{h} cell aspect {aspect:.3}");
                }
                _ => {}
            }
        }
        next += game::FRAME;
        let now = Instant::now();
        if next < now {
            // fell behind (suspended, slow terminal): don't race to catch up
            next = now + game::FRAME;
        }

        input.poll_pad();
        let menu = render::menu_for(game.state);
        let count = menu.as_ref().map_or(0, |m| m.items.len());
        let mut press = clicked.take();
        for cmd in std::mem::take(&mut input.cmds) {
            log::debug!("cmd {cmd:?} on {:?}", game.state);
            match cmd {
                Cmd::Exit => return Ok(()),
                Cmd::Pause if game.state == State::Playing => game.pause(),
                // ours: Esc leaves the pause menu as well
                Cmd::Pause if game.state == State::PauseMenu => game.resume(),
                Cmd::Up if count > 0 => selected = (selected + count - 1) % count,
                Cmd::Down if count > 0 => selected = (selected + 1) % count,
                Cmd::Confirm if count > 0 => press = Some(selected),
                _ => {}
            }
        }
        if let (Some(menu), Some(i)) = (&menu, press) {
            let action = menu.items[i.min(count - 1)].1;
            log::info!("{action:?} on {:?}", game.state);
            match action {
                Action::Exit => return Ok(()),
                Action::Play | Action::PlayAgain => game.new_game(1),
                Action::Continue => game.resume(),
                Action::NextLevel => game.next_level(),
            }
        }

        let dir = input.direction(Instant::now());
        let grid = drawn.field.map(render::grid_of).unwrap_or(game::Grid { cols: 93, rows: 29 });
        game.update(dir, grid);
        if game.state != shown {
            log::info!("state {:?} -> {:?}", shown, game.state);
            shown = game.state;
            selected = 0;
        }

        if snapshot.elapsed() >= SNAPSHOT_EVERY {
            snapshot = Instant::now();
            log::debug!(
                "snapshot state={:?} level={} player={:?} boss_health={:?} bullets={} dir={dir:?} grid={grid:?}",
                game.state,
                game.level,
                game.player.as_ref().map(|p| (p.pos.x.round(), p.pos.y.round(), p.pool.health)),
                game.boss.as_ref().map(|b| b.pool.health),
                game.bullets.len(),
            );
        }

        let notice = (!input.exact && input.pad_name.is_none()).then_some(NO_RELEASES);
        let view = View { selected, aspect, notice };
        terminal.draw(|f| drawn = render::draw(f, game, &view))?;
    }
}
