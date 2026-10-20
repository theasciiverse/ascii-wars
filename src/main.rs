mod audio;
mod bosses;
mod bot;
mod game;
mod input;
mod music;
mod render;
mod save;
#[cfg(test)]
mod tests;

use audio::{Audio, Ui};
use game::{Difficulty, Game, Mode, State};
use input::{Cmd, Input};
use ratatui::crossterm::event::{
    self, DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture, Event, KeyboardEnhancementFlags, MouseButton,
    MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;
use ratatui::layout::Position;
use render::{Drawn, Panel, View};
use save::Save;
use std::io;
use std::time::{Duration, Instant};

const SNAPSHOT_EVERY: Duration = Duration::from_millis(500);
/// Seconds on the title before the demo starts.
const ATTRACT_AFTER: f32 = 20.0;

const HELP: &str = "ascii-wars [--level N] [--classic] [--demo] [--mute] [--seed N]

ASCII Wars!, a boss rush bullet hell: ten bosses, one after another.

  arrows / WASD    move; you fire at the boss while you move
  Shift, Z or J    hold to focus: slow, careful moves
  X, K or Space    bomb: clears the shots round you
  Esc / P          pause          M   sound on and off
  Enter            choose (the mouse works in menus too)

  gamepad: d-pad or stick to move, A or a shoulder to focus,
  B or Y to bomb, Start to pause

  --level N        start Arcade at boss N (1-10)
  --classic        start the original game (with --level, at that boss)
  --demo           watch the computer play
  --mute           no sound or music
  --seed N         fix the random numbers

Best in a terminal that reports key releases (the kitty keyboard
protocol: kitty, foot, WezTerm, Ghostty, Alacritty, iTerm2 and others),
or with a gamepad.
Session log: RUST_LOG=debug ascii-wars 2>ascii-wars.log";

const NO_RELEASES: &str = "this terminal sends no key releases: moves run on a little (see --help)";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Screen {
    Title,
    Options,
    Practice,
    HowTo,
    Game,
}

struct App {
    screen: Screen,
    sel: usize,
    save: Save,
    /// The computer is playing (the attract demo or `--demo`).
    demo: bool,
    /// `--demo`: keys don't end it.
    demo_only: bool,
    idle: f32,
    shown: State,
    wait: f32,
    exact_keys: bool,
}

fn main() -> io::Result<()> {
    env_logger::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return Ok(());
    }
    let has = |f: &str| args.iter().any(|a| a == f);
    let value = |f: &str| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1));
    let level = value("--level").and_then(|v| v.parse::<i32>().ok()).map(|n| n.clamp(1, game::MAX_LEVEL));
    let seed = value("--seed").and_then(|v| v.parse::<u64>().ok()).unwrap_or_else(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1)
    });

    let save = Save::load();
    // open audio before the alternate screen, so any ALSA chatter lands above it
    let mut audio = Audio::new(has("--mute"), save.sound, save.music);
    log::info!("audio available {} seed {seed}", audio.available());

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

    let mut game = Game::new(seed);
    let mut app = App { screen: Screen::Title, sel: 0, save, demo: false, demo_only: false, idle: 0., shown: State::Title, wait: 0., exact_keys: kitty };
    if has("--demo") {
        app.demo = true;
        app.demo_only = true;
        app.screen = Screen::Game;
        game.start(Mode::Arcade, Difficulty::Normal, level.unwrap_or(1));
    } else if has("--classic") {
        app.screen = Screen::Game;
        game.new_game(level.unwrap_or(1));
    } else if let Some(n) = level {
        app.screen = Screen::Game;
        game.start(Mode::Arcade, app.save.difficulty, n);
    }
    let result = run(&mut terminal, &mut game, &mut input, &mut audio, &mut app);

    let _ = execute!(io::stdout(), DisableMouseCapture, DisableFocusChange);
    if kitty {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    result
}

/// What a choice on a panel does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Act {
    Arcade,
    Practice,
    Classic,
    Options,
    HowTo,
    Exit,
    Back,
    Difficulty,
    Sound,
    Music,
    Shake,
    Fight(i32),
    Locked,
    Continue,
    Retry,
    ToTitle,
    NextBoss,
    PlayAgain,
    ClassicNext,
}

fn run(terminal: &mut ratatui::DefaultTerminal, game: &mut Game, input: &mut Input, audio: &mut Audio, app: &mut App) -> io::Result<()> {
    let mut aspect = render::cell_aspect();
    log::info!("cell aspect {aspect:.3}");
    let mut drawn = Drawn::default();
    let mut clicked: Option<usize> = None;
    let mut next = Instant::now();
    let mut snapshot = Instant::now();
    loop {
        // input until the next frame is due
        let mut touched = false;
        loop {
            let wait = next.saturating_duration_since(Instant::now());
            if !event::poll(wait)? {
                break;
            }
            match event::read()? {
                Event::Key(k) => {
                    log::debug!("key {:?} {:?} {:?}", k.code, k.kind, k.modifiers);
                    input.on_key(k, Instant::now());
                    touched = true;
                }
                Event::Mouse(m) => {
                    let at = Position::new(m.column, m.row);
                    let over = drawn.buttons.iter().position(|r| r.contains(at));
                    match m.kind {
                        MouseEventKind::Moved => {
                            if let Some(i) = over {
                                if app.sel != i {
                                    app.sel = i;
                                    audio.ui(Ui::Move);
                                }
                            }
                        }
                        MouseEventKind::Down(MouseButton::Left) => {
                            log::debug!("click {},{} on choice {over:?}", m.column, m.row);
                            touched = true;
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
        let cmds = std::mem::take(&mut input.cmds);
        let grid = drawn.field.map(render::grid_of).unwrap_or(game::Grid { cols: 93, rows: 29 });

        // a key ends the demo
        if app.demo && !app.demo_only && (touched || !cmds.is_empty()) {
            log::info!("demo ends");
            app.demo = false;
            game.to_title();
            app.screen = Screen::Title;
            app.sel = 0;
            app.idle = 0.;
            continue;
        }

        let items = panel(game, app).map_or(0, |p| p.items.len());
        let mut press = clicked.take();
        for cmd in cmds {
            log::debug!("cmd {cmd:?} on {:?} {:?}", app.screen, game.state);
            match cmd {
                Cmd::Exit => return Ok(()),
                Cmd::Mute => {
                    app.save.sound = !app.save.sound;
                    app.save.music = app.save.sound;
                    audio.sound = app.save.sound;
                    audio.set_music(app.save.music);
                    app.save.store();
                }
                Cmd::Bomb if app.screen == Screen::Game => game.bomb(),
                Cmd::Pause if app.screen == Screen::Game && in_fight(game) => {
                    game.paused = !game.paused;
                    app.sel = 0;
                }
                Cmd::Pause | Cmd::Back if matches!(app.screen, Screen::Options | Screen::Practice | Screen::HowTo) => {
                    app.screen = Screen::Title;
                    app.sel = 0;
                }
                Cmd::Up if items > 0 => {
                    app.sel = (app.sel + items - 1) % items;
                    audio.ui(Ui::Move);
                }
                Cmd::Down if items > 0 => {
                    app.sel = (app.sel + 1) % items;
                    audio.ui(Ui::Move);
                }
                Cmd::Left | Cmd::Right if app.screen == Screen::Options => {
                    let back = cmd == Cmd::Left;
                    if let Some(a) = panel_acts(game, app).get(app.sel).copied() {
                        change(app, audio, a, back);
                    }
                }
                Cmd::Confirm if items > 0 => press = Some(app.sel),
                _ => {}
            }
        }
        if let Some(i) = press {
            let acts = panel_acts(game, app);
            if let Some(&a) = acts.get(i.min(acts.len().saturating_sub(1))) {
                log::info!("{a:?} on {:?} {:?}", app.screen, game.state);
                if act(game, app, audio, a) {
                    return Ok(());
                }
            }
        }

        let ((mut dir, mut focus), mut bomb) = (input.direction(Instant::now()), false);
        if app.demo {
            let c = bot::choose(game, grid);
            (dir, focus, bomb) = (c.mv, c.focus, c.bomb);
        }
        if bomb {
            game.bomb();
        }
        game.update(dir, focus, grid);
        for s in std::mem::take(&mut game.events) {
            audio.play(s);
        }
        follow_state(game, app, audio);

        // music: the title's tune off the field, the boss's on it
        let tune = match (app.screen, game.state) {
            (Screen::Game, State::GameOver) => -1,
            (Screen::Game, State::WinGame) => 0,
            (Screen::Game, _) => game.level,
            _ => 0,
        };
        audio.tune(tune);
        let k = game.boss.as_ref().map_or(0.3, |b| {
            let hp = 1. - (b.pool.health / b.pool.max_health).clamp(0., 1.);
            ((b.phase as f32 + hp) / b.phases as f32).clamp(0., 1.)
        });
        audio.intensity(if game.paused { 0. } else { k });

        if app.screen == Screen::Title && !app.demo {
            app.idle += game::DT;
            if touched {
                app.idle = 0.;
            }
            if app.idle >= ATTRACT_AFTER {
                let n = (game.rng.next() % 10) as i32 + 1;
                log::info!("demo starts at boss {n}");
                app.demo = true;
                app.screen = Screen::Game;
                game.start(Mode::Arcade, Difficulty::Normal, n);
            }
        }

        if snapshot.elapsed() >= SNAPSHOT_EVERY {
            snapshot = Instant::now();
            log::debug!(
                "snapshot screen={:?} state={:?} paused={} mode={:?} level={} player={:?} boss={:?} bullets={} lasers={} score={} bombs={} dir={dir:?} focus={focus} grid={grid:?}",
                app.screen,
                game.state,
                game.paused,
                game.mode,
                game.level,
                game.player.as_ref().map(|p| (p.pos.x.round(), p.pos.y.round(), p.pool.health)),
                game.boss.as_ref().map(|b| (b.phase, b.pool.health)),
                game.bullets.len(),
                game.lasers.len(),
                game.score,
                game.bombs,
            );
        }

        let mut footer: Vec<&str> = Vec::new();
        if app.demo {
            footer.push(if app.demo_only { "DEMO" } else { "DEMO   press any key" });
        } else if app.screen == Screen::Title {
            footer.push("move: arrows/WASD (moving fires)   focus: Shift/Z   bomb: X   pause: Esc   sound: M");
            if !app.exact_keys && input.pad_name.is_none() {
                footer.push(NO_RELEASES);
            }
        }
        let view = View {
            panel: if app.demo { None } else { panel(game, app) },
            logo: matches!(app.screen, Screen::Title | Screen::Options),
            aspect,
            shake: app.save.shake,
            hi: app.save.best[game.difficulty.index()],
            footer,
        };
        terminal.draw(|f| drawn = render::draw(f, game, &view))?;
    }
}

fn in_fight(game: &Game) -> bool {
    matches!(game.state, State::Intro | State::Playing | State::BossDying | State::PlayerDying)
}

/// Records and the demo's next step, as the game moves between states.
fn follow_state(game: &mut Game, app: &mut App, audio: &mut Audio) {
    if game.state != app.shown {
        log::info!("state {:?} -> {:?}", app.shown, game.state);
        app.shown = game.state;
        app.sel = 0;
        app.wait = 0.;
        if !app.demo && game.mode != Mode::Classic {
            let s = &mut app.save;
            match game.state {
                State::Results => {
                    let r = &game.results;
                    let i = (r.level - 1) as usize;
                    if s.best_time[i].is_none_or(|t| r.time < t) {
                        s.best_time[i] = Some(r.time);
                    }
                    if game.mode == Mode::Arcade {
                        s.reached = s.reached.max((r.level + 1).min(game::MAX_LEVEL));
                    }
                    s.store();
                }
                State::GameOver | State::WinGame if game.mode == Mode::Arcade => {
                    let d = game.difficulty.index();
                    s.best[d] = s.best[d].max(game.score);
                    if game.state == State::WinGame {
                        s.cleared = true;
                    }
                    s.store();
                }
                _ => {}
            }
        }
        if game.state == State::WinGame {
            audio.play(game::Sound::Win);
        }
    }
    if app.demo {
        app.wait += game::DT;
        match game.state {
            State::Results if app.wait > 2.5 => {
                if game.level >= game::MAX_LEVEL {
                    game.start(Mode::Arcade, Difficulty::Normal, 1);
                } else {
                    game.next_level();
                }
            }
            State::GameOver | State::WinGame if app.wait > 2.5 => {
                let n = (game.rng.next() % 10) as i32 + 1;
                game.start(Mode::Arcade, Difficulty::Normal, n);
            }
            _ => {}
        }
    }
}

/// The panel on screen now, if any.
fn panel(game: &Game, app: &App) -> Option<Panel> {
    let (title, color, lines, items, w, h) = panel_content(game, app)?;
    let classic_menu = game.classic() && app.screen == Screen::Game;
    let top = (app.screen != Screen::Game).then_some(10);
    let items: Vec<String> = items.into_iter().map(|(l, _)| l).collect();
    Some(Panel {
        title,
        color,
        lines,
        selected: app.sel.min(items.len().saturating_sub(1)),
        items,
        w_pct: w,
        h_pct: h,
        top,
        buttons: classic_menu,
    })
}

fn panel_acts(game: &Game, app: &App) -> Vec<Act> {
    panel_content(game, app).map(|c| c.3.into_iter().map(|(_, a)| a).collect()).unwrap_or_default()
}

type Content = (String, bosses::Rgb, Vec<(String, bosses::Rgb)>, Vec<(String, Act)>, u16, u16);

const TEXT: bosses::Rgb = (215, 215, 230);
const DIM: bosses::Rgb = (140, 140, 165);
const GOLD: bosses::Rgb = (255, 215, 90);

fn time_text(t: f32) -> String {
    format!("{}:{:04.1}", (t / 60.) as u32, t % 60.)
}

fn panel_content(game: &Game, app: &App) -> Option<Content> {
    let s = &app.save;
    let it = |l: &str, a: Act| (l.to_string(), a);
    let line = |l: String, c| (l, c);
    match app.screen {
        Screen::Title => {
            let best = s.best[s.difficulty.index()];
            let title = if best > 0 { format!("best {} on {}", best, s.difficulty.name()) } else { "welcome".into() };
            Some((
                title,
                GOLD,
                vec![],
                vec![
                    it("Arcade: all ten bosses", Act::Arcade),
                    it("Practice a boss", Act::Practice),
                    it("Classic: the original game", Act::Classic),
                    it("Options", Act::Options),
                    it("How to play", Act::HowTo),
                    it("Exit", Act::Exit),
                ],
                50,
                0,
            ))
        }
        Screen::Options => {
            let on = |b: bool| if b { "on" } else { "off" };
            Some((
                "Options".into(),
                render::PLAYER,
                vec![line("left and right change a setting".into(), DIM)],
                vec![
                    (format!("Difficulty     < {} >", s.difficulty.name()), Act::Difficulty),
                    (format!("Sound          {}", on(s.sound)), Act::Sound),
                    (format!("Music          {}", on(s.music)), Act::Music),
                    (format!("Screen shake   {}", on(s.shake)), Act::Shake),
                    it("Back", Act::Back),
                ],
                50,
                0,
            ))
        }
        Screen::Practice => {
            let mut items: Vec<(String, Act)> = (1..=game::MAX_LEVEL)
                .map(|n| {
                    if n <= s.reached {
                        let b = bosses::arcade(n);
                        let t = s.best_time[(n - 1) as usize].map_or("  --  ".into(), time_text);
                        (format!("{n:2}  {:<24} {t}", b.name), Act::Fight(n))
                    } else {
                        (format!("{n:2}  {:<24} {}", "? ? ?", "      "), Act::Locked)
                    }
                })
                .collect();
            items.push(it("Back", Act::Back));
            Some((
                format!("Practice ({})", s.difficulty.name()),
                render::PLAYER,
                vec![line("reach a boss in Arcade to practise it".into(), DIM)],
                items,
                60,
                0,
            ))
        }
        Screen::HowTo => Some((
            "How to play".into(),
            render::PLAYER,
            vec![
                line("You fire at the boss whenever you move.".into(), TEXT),
                line("Standing still is safe from nothing.".into(), TEXT),
                line(String::new(), TEXT),
                line("Hold Shift or Z to focus: slow and careful.".into(), TEXT),
                line("X bombs: it clears the shots round you.".into(), TEXT),
                line("Brush past shots to graze: points, and".into(), TEXT),
                line(format!("a new bomb every {} grazes.", game::GRAZES_PER_BOMB), TEXT),
                line("Dotted lines turn into beams. Move!".into(), TEXT),
                line(String::new(), TEXT),
                line("Each boss fights in phases; break one".into(), TEXT),
                line("without a scratch for a bonus.".into(), TEXT),
            ],
            vec![it("Back", Act::Back)],
            60,
            0,
        )),
        Screen::Game => game_panel(game, app),
    }
}

fn game_panel(game: &Game, app: &App) -> Option<Content> {
    let it = |l: &str, a: Act| (l.to_string(), a);
    let classic = game.classic();
    let white = (255, 255, 255);
    if game.paused {
        return Some(if classic {
            ("Pause!".into(), white, vec![], vec![it("Continue", Act::Continue), it("Exit", Act::Exit)], 40, 60)
        } else {
            (
                "Paused".into(),
                render::PLAYER,
                vec![],
                vec![it("Continue", Act::Continue), it("Retry this boss", Act::Retry), it("Quit to title", Act::ToTitle)],
                40,
                40,
            )
        });
    }
    match game.state {
        State::FinishedLevel => Some(("Completed!".into(), white, vec![], vec![it("Continue to next level", Act::ClassicNext)], 40, 40)),
        State::GameOver if classic => Some(("Game Over!".into(), white, vec![], vec![it("Play Again", Act::PlayAgain), it("Exit", Act::Exit)], 40, 60)),
        State::WinGame if classic => Some(("You Won!".into(), white, vec![], vec![it("Play Again", Act::PlayAgain), it("Exit", Act::Exit)], 40, 60)),
        State::Results => {
            let r = &game.results;
            let def = bosses::arcade(r.level);
            let bonus = |n: u64| if n > 0 { format!("+{n}") } else { "-".into() };
            let best = app.save.best_time[(r.level - 1) as usize];
            let mut lines = vec![
                (format!("time            {}", time_text(r.time)), TEXT),
                (format!("hits taken      {}", r.hits), TEXT),
                (format!("bombs used      {}", r.bombs_used), TEXT),
                (format!("speed bonus     {}", bonus(r.time_bonus)), TEXT),
                (format!("no-hit bonus    {}", bonus(r.no_hit_bonus)), TEXT),
                (format!("no-bomb bonus   {}", bonus(r.bomb_bonus)), TEXT),
                (format!("score           {}", r.score), GOLD),
            ];
            if best.is_some_and(|b| (b - r.time).abs() < 0.01) {
                lines.push(("best time yet!".into(), GOLD));
            }
            let items = if game.mode == Mode::Practice {
                vec![it("Fight again", Act::Retry), it("Boss select", Act::Practice)]
            } else if r.level >= game::MAX_LEVEL {
                vec![it("Finish", Act::NextBoss)]
            } else {
                vec![it("Next boss", Act::NextBoss), it("Quit to title", Act::ToTitle)]
            };
            Some((format!("{} defeated!", def.name), def.color, lines, items, 50, 0))
        }
        State::GameOver => {
            let def = bosses::arcade(game.level);
            let lines = vec![(format!("score {}", game.score), GOLD), (format!("fell to {}", def.name), TEXT)];
            let items = if game.mode == Mode::Practice {
                vec![it("Fight again", Act::Retry), it("Boss select", Act::Practice)]
            } else {
                vec![it("Continue (score back to 0)", Act::Retry), it("Quit to title", Act::ToTitle)]
            };
            Some(("Game Over".into(), (255, 90, 90), lines, items, 50, 0))
        }
        State::WinGame => {
            let best = app.save.best[game.difficulty.index()];
            let mut lines = vec![
                ("All ten bosses are down.".into(), TEXT),
                (format!("final score {}", game.score), GOLD),
                (format!("continues {}", game.continues), TEXT),
            ];
            if game.score > 0 && game.score >= best && game.continues == 0 {
                lines.push(("a new best!".into(), GOLD));
            }
            Some(("You Won!".into(), GOLD, lines, vec![it("Title", Act::ToTitle)], 50, 0))
        }
        _ => None,
    }
}

/// Left or right on an option.
fn change(app: &mut App, audio: &mut Audio, a: Act, back: bool) {
    let s = &mut app.save;
    match a {
        Act::Difficulty => {
            let i = s.difficulty.index() as i32 + if back { -1 } else { 1 };
            s.difficulty = Difficulty::ALL[i.rem_euclid(3) as usize];
        }
        Act::Sound => {
            s.sound = !s.sound;
            audio.sound = s.sound;
        }
        Act::Music => {
            s.music = !s.music;
            audio.set_music(s.music);
        }
        Act::Shake => s.shake = !s.shake,
        _ => return,
    }
    audio.ui(Ui::Move);
    s.store();
}

/// Do a choice; true to quit.
fn act(game: &mut Game, app: &mut App, audio: &mut Audio, a: Act) -> bool {
    if a != Act::Locked {
        audio.ui(Ui::Select);
    }
    let diff = app.save.difficulty;
    match a {
        Act::Exit => return true,
        Act::Arcade => {
            app.screen = Screen::Game;
            game.start(Mode::Arcade, diff, 1);
        }
        Act::Classic => {
            app.screen = Screen::Game;
            game.new_game(1);
        }
        Act::Practice => {
            game.to_title();
            app.screen = Screen::Practice;
        }
        Act::Options => app.screen = Screen::Options,
        Act::HowTo => app.screen = Screen::HowTo,
        Act::Back | Act::ToTitle => {
            game.to_title();
            app.screen = Screen::Title;
        }
        Act::Difficulty | Act::Sound | Act::Music | Act::Shake => change(app, audio, a, false),
        Act::Fight(n) => {
            app.screen = Screen::Game;
            game.start(Mode::Practice, diff, n);
        }
        Act::Locked => {}
        Act::Continue => game.paused = false,
        Act::Retry => game.retry(),
        Act::NextBoss | Act::ClassicNext => game.next_level(),
        Act::PlayAgain => game.new_game(1),
    }
    if !matches!(a, Act::Difficulty | Act::Sound | Act::Music | Act::Shake | Act::Locked) {
        app.sel = 0;
    }
    false
}
