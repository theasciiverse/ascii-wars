# ASCII Wars!

A boss rush bullet hell in ASCII: ten bosses, one after another, each with
its own rings and aimed shots. You fire at the boss whenever you move, so
standing still is never safe.

This is a terminal port of Carlos Alvarez's [ASCII Wars!](https://github.com/Alvarz/ascii-wars)
(2025, Rust and Bevy) for the Asciiverse catalog, in Rust and Ratatui. The
bosses, their patterns, speeds and fire rates are the original's.

## Play

```
cargo run --release
```

| | |
|---|---|
| arrows / WASD | move (and fire) |
| Esc / P | pause |
| Enter / Space | press the chosen button; the mouse works too |
| gamepad | d-pad or left stick to move, A to press, Start to pause |

`--level N` starts at boss N. Best in a terminal that reports key releases
(kitty keyboard protocol: kitty, foot, WezTerm, Ghostty, Alacritty, iTerm2
and others) or with a gamepad; elsewhere a move runs on a little after the
key is let go.

Session log: `RUST_LOG=debug ascii-wars 2>ascii-wars.log`.

## Licence

MIT, as the original; see `LICENSE`.
