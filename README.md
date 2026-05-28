# Moonroom

Moonroom is a Rust and Lua engine for building parser-based interactive fiction.

The Rust engine owns parsing, world state, save/load, transcript tests, and the command-line frontend. Lua owns the authored game world: rooms, things, exits, verbs, dialogue, events, and callbacks.

Moonroom is early, but already playable.

## Features

- Lua DSL for rooms, things, exits, verbs, actors, topics, and timed events.
- Classic parser commands: `look`, `go north`, `take key`, `drop key`, `read note`, `use key`, `inventory`, and more.
- Containers, supporters, wearables, guarded exits, NPC talk, and topic-based dialogue.
- Deterministic Rust-owned state with JSON save/load.
- Static project validation with `moonroom check`.
- Transcript tests for repeatable game behavior.
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

Check a game for missing rooms, invalid thing locations, invalid guarded exits, and duplicate object vocabulary:

```bash
cargo run -q -p mr-cli -- check examples/house
```

The transcript format is plain text:

```text
> look
Foyer

Rain needles the windows. A brass key rests on the table.
```

## Create A Game

```bash
cargo run -q -p mr-cli -- new my-game
cargo run -q -p mr-cli -- play my-game
cargo run -q -p mr-cli -- check my-game
cargo run -q -p mr-cli -- test my-game
```

When installed as a binary, the user-facing command is `moonroom`.

## Lua Example

```lua
game {
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
crates/mr-cli    moonroom play/test/new command-line frontend
crates/mr-test   transcript test parser and runner
examples/house   main integration example
docs/lua-dsl.md  author-facing Lua DSL reference
```

## Documentation

- [Lua DSL reference](docs/lua-dsl.md)
- [Project plan](PLAN.md)
- [Agent notes](AGENTS.md)

## Current Status

Moonroom is pre-release and changing quickly. The core design goal is stable, serializable Rust-owned game state with Lua used as a controlled authoring layer, not as the save format.

Near-term roadmap priorities:

- `again` and `undo`
- richer object state such as open/closed and locked/unlocked
- broader author tooling such as `moonroom inspect` and transcript recording
