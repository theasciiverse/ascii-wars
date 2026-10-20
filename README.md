# ASCII Wars!

A boss rush bullet hell in ASCII: ten bosses, one after another. You fire
at the boss whenever you move, so standing still is never safe.

Built on Carlos Alvarez's [ASCII Wars!](https://github.com/Alvarz/ascii-wars)
(2025, Rust and Bevy), brought to the terminal for the Asciiverse catalog in
Rust and Ratatui and grown into a fuller game:

- **Arcade**: all ten bosses in a row, each fought in phases that move,
  aim, rain, wall you in, sweep beams and send homing swarms; one score.
- **Practice**: any boss you have reached, with your best time.
- **Classic**: the original game, rule for rule.
- Focus for careful moves, bombs, grazing for points and extra bombs,
  bonuses for clean phases, Easy, Normal and Hard.
- Colour, sparks, explosions, banners, synthesised sound and a chiptune
  for every boss; a demo plays itself when the title is left alone.

## Play

```
cargo run --release
```

| | |
|---|---|
| arrows / WASD | move (and fire) |
| Shift, Z or J | hold to focus: slow, careful moves |
| X, K or Space | bomb |
| Esc / P | pause |
| M | sound on and off |
| Enter | choose; the mouse works in menus |
| gamepad | stick or d-pad to move, A or a shoulder to focus, B or Y to bomb, Start to pause |

`--level N` starts Arcade at boss N, `--classic` the original game,
`--demo` lets the computer play. Best in a terminal that reports key
releases (kitty keyboard protocol: kitty, foot, WezTerm, Ghostty,
Alacritty, iTerm2 and others) or with a gamepad; elsewhere a move runs on a
little after the key is let go.

Settings and records: `$XDG_DATA_HOME/ascii-wars/save.txt`
(`%APPDATA%` on Windows). Session log: `RUST_LOG=debug ascii-wars 2>ascii-wars.log`.

## What this edition changes

Arcade and Practice are new: phases, moving bosses and their new attacks,
focus, bombs, grazing, the blink after a hit, the score, colour, sound and
music. Arcade also fires every fourth frame for four times the damage
(the original fires every frame), so the screen is not crossed by a solid
line of shots.

Classic is the original game, rule for rule (speeds, fire rates, health,
its timers and quirks), at a fixed 60 frames a second in the original's
window coordinates, with these differences:

- The player is hit by a shot drawn in, or passing through, the player's
  cell: terminal cells are taller than the original's hit circle, so its
  own test could hit from the next cell or miss a shot drawn on top.
- The HUD sits on a line above the field and names the boss.
- Menus work with keys and gamepad as well as the mouse; Esc also leaves
  the pause menu; music plays (the original is silent).
- The ship's '>' turns in four steps; no CRT shader.

## Licence

MIT, as the original; see `LICENSE`.
