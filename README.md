# Moonroom

Moonroom is a Rust and Lua engine for building parser-based interactive fiction.

The Rust engine owns parsing, world state, save/load, transcript tests, and the command-line frontend. Lua owns the authored game world: rooms, things, exits, verbs, dialogue, events, and callbacks.

Moonroom is early, but already playable.

## Features

- Lua DSL for rooms, things, exits, verbs, actors, topics, scenes, chapters, and timed events.
- Classic parser commands: `look`, `look north`, `look at key`, `go north`, `go n`, `take all`, `x it`, `open box`, `unlock chest with key`, `use key on door`, `read note`, `tell caretaker about key`, `show key to caretaker`, `again`, `undo`, and more.
- Containers, supporters, wearables, hidden/revealed things, openable and lockable things, guarded exits, NPC talk, topic aliases, and Rust-owned actor memory.
- Versioned, game-checked JSON save/load, including scenes, chapters, timers, and actor memory.
- Static project validation with `moonroom check`.
- Project inspection with `moonroom inspect`.
- Transcript tests for repeatable game behavior.
- Transcript recording with `moonroom transcript`.
- Single-file `.moon` packages with pack, unpack, play, check, and test support.
- Standalone executable export with an embedded `.moon` package.
- Interactive CLI with shell-style command history.
- Piped input support for smoke tests and scripts.

## Try The Example

```bash
cargo run -q -p mr-cli -- play examples/house
```

Inside interactive play, use the up/down arrow keys to browse command history.

You can also pipe commands:

```bash
printf '%s\n' look 'take the key' north quit \
  | cargo run -q -p mr-cli -- play examples/house
```

## Run Tests

Run the full Rust workspace tests:

```bash
cargo test --workspace
```

Run the example game's transcript tests:

```bash
cargo run -q -p mr-cli -- test examples/house
```

You can narrow or refresh transcript runs:

```bash
cargo run -q -p mr-cli -- test examples/house --filter opening
cargo run -q -p mr-cli -- test examples/house --seed 12345
cargo run -q -p mr-cli -- test examples/house --update
```

Check a game for missing rooms, invalid thing locations, invalid guarded exits, and duplicate object vocabulary:

```bash
cargo run -q -p mr-cli -- check examples/house
```

Inspect a game's rooms, things, exits, verbs, events, and callbacks:

```bash
cargo run -q -p mr-cli -- inspect examples/house
```

Record a play session to a transcript file:

```bash
cargo run -q -p mr-cli -- transcript examples/house -o examples/house/tests/recorded.transcript
```

Package a game for sharing:

```bash
cargo run -q -p mr-cli -- pack examples/house -o dist/house.moon
cargo run -q -p mr-cli -- play dist/house.moon
cargo run -q -p mr-cli -- check dist/house.moon
cargo run -q -p mr-cli -- test dist/house.moon
```

Unpack a `.moon` file for inspection or recovery:

```bash
cargo run -q -p mr-cli -- unpack dist/house.moon -o unpacked-house
```

Build a standalone executable with the `.moon` package embedded:

```bash
cargo run -q -p mr-cli -- build examples/house --standalone -o dist/house
```

The transcript format is plain text:

```text
> look
Foyer

Rain needles the windows. A brass key rests on the table.

> take key
You take the brass key.
!flag touched_key
!contains brass key
!not_contains silver key
```

Assertion lines beginning with `!` check engine state after a command without being compared as output.

## Create A Game

```bash
cargo run -q -p mr-cli -- new my-game
cargo run -q -p mr-cli -- play my-game
cargo run -q -p mr-cli -- check my-game
cargo run -q -p mr-cli -- test my-game
cargo run -q -p mr-cli -- inspect my-game
```

When installed as a binary, the user-facing command is `moonroom`.

## Lua Example

```lua
game {
  id = "house-under-glass",
  version = "0.1.0",
  title = "The House Under Glass",
  author = "Example Author",
  start = "foyer",

  settings = {
    exits = {
      show = true
    }
  }
}

room "foyer" {
  name = "Foyer",
  desc = "Rain needles the windows. A brass key rests on the table.",
  exits = {
    north = "hall",
    east = {
      to = "study",
      requires = "brass_key",
      locked_msg = "The study door is locked tight."
    }
  }
}

thing "brass_key" {
  name = "brass key",
  aliases = { "key", "brass key" },
  location = "foyer",
  portable = true,
  desc = "The brass key is cold and slightly tarnished.",
  read = "The key is stamped STUDY.",

  on_take = function(game)
    game.flag("touched_key")
    game.say("The key is colder than it should be.")
  end
}

thing "cedar_chest" {
  name = "cedar chest",
  aliases = { "chest" },
  location = "foyer",
  portable = false,
  container = true,
  openable = true,
  open = false,
  lockable = true,
  locked = true,
  key = "brass_key"
}

thing "folded_note" {
  name = "folded note",
  aliases = { "note" },
  location = "cedar_chest",
  portable = true,
  hidden = true
}
```

Larger games can split definitions into project-local files:

```lua
include "rooms.lua"
include "things.lua"
include "dialogue.lua"
include "verbs.lua"
include "events.lua"
```

Moonroom matches one leading article in object phrases, so `take key`, `take the key`, and `take a brass key` resolve to the same object when the aliases match.

## Project Layout

```text
crates/mr-core   engine state, parser actions, events, and saveable model
crates/mr-lua    Lua DSL loading, callbacks, and controlled game API
crates/mr-cli    moonroom play/test/check/new command-line frontend
crates/mr-test   transcript test parser and runner
examples/house   main integration example
docs/lua-dsl.md  author-facing Lua DSL reference
```

## Documentation

- [Lua DSL reference](docs/lua-dsl.md)
- [Active release plan and architecture contract](PLAN.md)
- [Long-term roadmap](ROADMAP.md)
- [Agent notes](AGENTS.md)

## Current Status

Moonroom is pre-release and changing quickly. The core design goal is stable, serializable Rust-owned game state with Lua used as a controlled authoring layer, not as the save format.

Near-term roadmap priorities:

- richer object state such as scenery and light/dark rooms
- first-game and packaging documentation
