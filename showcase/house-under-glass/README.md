# The House Under Glass

The House Under Glass is planned as Moonroom's polished showcase game: a small parser-fiction mystery about a rain-lashed house, a tarnished key, and rooms that remember who has passed through them.

This directory is currently a first playable slice of the planned showcase. See [PLAN.md](PLAN.md) for the intended full story shape, room list, puzzle chain, and engine features the showcase should exercise. See [DESIGN.md](DESIGN.md) for the story and design guidelines.

## Status

Playable 3-room slice. `examples/house` remains the compact integration testbed; this directory is the more polished showcase path.

Implemented in this slice:

- Foyer, Hall, and Study.
- Split Lua files loaded from `game.lua`.
- Tarnished key and linen coat puzzle.
- State-dependent room descriptions.
- Caretaker dialogue and topics.
- Custom `polish` and `read` verbs.
- Timed atmospheric event.
- Transcript coverage for opening, golden path, and caretaker topics.

## Commands

```bash
cargo run -- play showcase/house-under-glass
cargo run -- test showcase/house-under-glass
```
