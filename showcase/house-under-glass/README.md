# The House Under Glass

The House Under Glass is planned as Moonroom's polished showcase game: a small parser-fiction mystery about a rain-lashed house, a tarnished key, and rooms that remember who has passed through them.

This directory is Moonroom 0.1's release-driving showcase. See [PLAN.md](PLAN.md) for the story shape, room list, puzzle chain, and engine features it exercises. See [DESIGN.md](DESIGN.md) for the story and design guidelines.

## Status

Polished 8-room release showcase. `examples/house` remains the compact integration testbed.

Implemented in this slice:

- Foyer, Hall, Study, Kitchen, Conservatory, Upstairs Landing, Locked Bedroom, and Glass Room.
- Split Lua files loaded from `game.lua`.
- Tarnished key and linen coat puzzle.
- Locked/openable wooden box with a hidden note.
- Optional Kitchen branch with cabinet and garden shears.
- Conservatory reveal path and cracked pane route upstairs.
- Lockable bedroom door and mirror memory beat.
- Glass Room ending beats: close the ledger, leave it open, or write your name.
- State-dependent room descriptions.
- Caretaker dialogue with topic aliases, gated topics, ask/tell, show, and give.
- Custom `polish`, `write`, and `leave` verbs, plus built-in `read` support for the ledgers.
- Timed atmospheric event plus scene-scoped timer.
- Saved chapter and scene state for the note, upstairs route, mirror, and Glass Room.
- Transcript coverage for opening, golden path, caretaker topics, object state, Kitchen branch, bedroom route, Glass Room route, and ending choices.

## Commands

```bash
cargo run -- play showcase/house-under-glass
cargo run -- check showcase/house-under-glass
cargo run -- test showcase/house-under-glass
cargo run -- inspect showcase/house-under-glass
cargo run -- pack showcase/house-under-glass -o dist/house-under-glass.moon
```
