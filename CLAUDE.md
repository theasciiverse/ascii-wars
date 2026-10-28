# ASCII Wars!

A Rust + Ratatui port of ASCII Wars!, Carlos Alvarez's (VikingosTeam) boss
rush bullet hell (2025), for the Asciiverse catalog. The original draws
CP437 glyphs as sprites in a Bevy window with a CRT shader; the port keeps
the game and puts the glyphs in the terminal, the way the Armoured
Commander ports keep theirs.

Status: started 2026-10-20 as a straight port (the whole original: ten
bosses, the player, the HUD, the five menus). The original is a thin game
(static bosses, rings and aimed shots, white dots), so the same day it
grew into a fuller one around it, the original kept as Classic mode:

- Arcade (ten bosses in a row, one score), Practice (any boss reached,
  best times) and Classic (the original, rule for rule).
- Every boss redesigned in three or four phases (`bosses.rs` `ARCADE`),
  each opening with its classic attack, then moving (sway, hop, chase) and
  adding fans, bursts, rain, walls with gaps, curving, accelerating,
  bouncing and homing shots, and lasers (dotted warning, then a burning
  beam, aimed or sweeping). Phase names shown as banners.
- Player: blink of safety after a hit (by difficulty), focus (slow moves,
  hit cell shown), bombs (shock wave clears shots, 3 per game, carried
  over), grazing (points and a bomb every 80), health carried between
  Arcade bosses with 4 back.
- Score: hits, grazes, cancelled shots, phase bonuses (clean phase +3000),
  speed, no-hit and no-bomb bonuses; Easy x0.5, Normal x1, Hard x1.5.
- Look: colour per boss, multi-cell boss art, stars, sparks, explosions,
  shake, red flash on a hit, boss health bar with phase pips, title logo.
- Sound (`audio.rs`, synthesised effects) and music (`music.rs`, a
  chiptune per boss and the title, intensity rising through the fight).
- Options (difficulty, sound, music, shake) and records in a save file;
  attract demo after 20 s on the title, `--demo` for the preview.
- `bot.rs`: a greedy dodger; 2026-10-20 it beat all ten bosses on every
  difficulty, so the fights can be won (humans will find them harder).

Keyboard, mouse in the menus and gamepad (an Xbox 360 pad is picked up).
On the website mockup since 2026-10-20: `hub/catalog/ascii-wars.toml` and
`.md`, preview `asciiverse/ascii-wars.cast` (100x30, `ascii-wars --demo
--level 8 --seed 4` through hub's `castrec.py`, best of eight takes; still
at 36 s, the Supernova ring), local Linux build in the website's
`dev-releases/ascii-wars` (1.0.0, links libasound and libudev).
Private at github.com/theasciiverse/ascii-wars (pushed 2026-10-20), Alvarz/ascii-wars as `upstream`. Release workflow `.github/workflows/release.yml` (2026-10-20), on the
pattern of Armoured Commander II's: glibc Linux on Ubuntu 22.04 (x86_64,
aarch64; libasound and libudev linked dynamically, declared as
system_deps), macOS and Windows (MSVC), x86_64 and ARM64 each; archives
`ascii-wars-<target>` with the binary, README.md and LICENSE. Tags
`v<version>`; `v1.0.0` pushed (2026-10-28), its run waiting on the
organisation's Actions billing. macOS and Windows are not built locally
(no rustup here), so the first run is their first build.

## Source and licence

- Original: github.com/Alvarz/ascii-wars, last commit `0c4121f`
  (2025-04-29, Bevy 0.16), cloned at `../../references/ASCII Wars`. About
  2,450 lines; all real work was 2025-01-22 to 2025-01-29, later commits
  are dependency bumps. No tags or releases.
- MIT, `LICENSE.md` in the original; our `LICENSE` keeps its notice and
  adds ours.
- Its assets (`terminal8x8.png`, a CP437 sheet, and `crt.wgsl`) are not
  used: the terminal draws the glyphs itself.

## How the port is built

One `Game::update` is one frame at a fixed 60 Hz (the original ran at the
display's refresh with vsync, so its fire rates and the player's one shot
a frame assumed about that). Positions stay in the original's pixels: a
1260x787.5 window (1680x1050 at scale 0.75), y up, so every speed, rate
and hit size carries over unchanged.

- `src/bosses.rs`: `CLASSIC`, the original's ten from `enemies.rs`, and
  `ARCADE`, ours.
- `src/game.rs`: the rules; `update_classic` is the original's `game.rs`,
  `player.rs`, `shoot.rs` and pattern systems, `update_arcade` ours.
- `src/render.rs`: field, HUD, panels, logo. `src/main.rs`: screens and
  menus. `src/save.rs`, `src/audio.rs`, `src/music.rs`, `src/bot.rs`.
- `src/input.rs`: held keys and the gamepad, taken over from Ceres.
- `src/tests.rs`: classic rules checked against the original's numbers,
  arcade rules; `cargo test --release -- --ignored --nocapture
  bot_report` runs the bot against every boss on every difficulty.

Kept from the original in Classic, quirks included:

- The player fires one bullet a frame at the boss while moving, never
  while standing still. Player: 10 health, 2 per hit, dies when health goes
  below 0, so on the sixth hit. Boss: 1500 health, so 751 hits.
- Bosses never move (their up and down drift is commented out in the
  original).
- Bullets hit as an 8 pixel circle whatever their size; size only changes
  the look (bold here at 2x and up). Bullets live 10 seconds, on or off
  screen.
- One timer per pattern system (`Slot`), shared by every boss and kept
  from level to level; a timer is reset to the pattern's fire rate when it
  fires, so the remainder is lost. A fire rate of 0 fires every frame.
- Rings turn with Bevy's virtual time since start, which runs in the menus.
- The HUD stays up on the menus after the first level starts.
- Exit on the pause, game over and won menus quits the program.

Changed, and why:

- How the player is hit: by a bullet drawn in the player's cell, or that
  passed through it since the last frame. Glyphs land on whole cells, and
  the original's test (8 px circle against a 2.6 px box) is shorter than a
  cell is tall at 100x30, so it could hit from the next cell or miss a
  bullet drawn on the player. By area the two match at 100x30; on bigger
  terminals cells are smaller, so it gets a little easier there. The
  boss's hit test is the original's.
- The HUD is a line above the field instead of on top of it, and shows the
  boss's name (the names are the original's, from its source comments).
- Esc also leaves the pause menu; menus work with keys and the gamepad,
  not only the mouse. Classic has music (the original has no sound).
- The player's '>' turns in four steps (the sprite turned freely).
- No CRT shader or chromatic aberration.

## Layout

The field keeps the original window's 1.6:1 shape in cells, using the
terminal's real cell shape where it reports pixel sizes (else 1:2), centred
below the HUD line. At 100x30 that is 93x29. Smallest: 40x13.

## Commands

```bash
cargo run --release              # play
cargo run --release -- --level 7 # Arcade from boss 7
cargo run --release -- --classic # the original
cargo run --release -- --demo    # the computer plays
cargo test
```
