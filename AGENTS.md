# Moonroom Agent Notes

Moonroom is a Rust workspace for an interactive fiction engine with Lua-authored game worlds. Treat `PLAN.md` as the product roadmap and `README.md` as the current user-facing quickstart.

## Workspace

- `crates/mr-core`: serializable world model, game state, parser/action handling, and core events.
- `crates/mr-lua`: Lua DSL loading, callback registry, controlled `game` API, and JSON save/load of Rust-owned state.
- `crates/mr-cli`: `moonroom play`, `moonroom test`, `moonroom check`, and `moonroom new`.
- `crates/mr-test`: transcript parsing and test runner.
- `examples/house`: the main integration example.
- `docs/lua-dsl.md`: author-facing Lua DSL reference.

## Useful Commands

Run the example interactively:

```bash
cargo run -q -p mr-cli -- play examples/house
```

`moonroom play` uses `rustyline` when stdin/stdout are terminals, giving up/down command history and basic line editing. It falls back to plain stdin for piped smoke tests.

Run transcript tests:

```bash
cargo run -q -p mr-cli -- test examples/house
```

Check static project structure:

```bash
cargo run -q -p mr-cli -- check examples/house
```

Create a template game:

```bash
cargo run -q -p mr-cli -- new my-game
```

Full verification:

```bash
cargo fmt --all --check
cargo test --workspace
cargo run -q -p mr-cli -- check examples/house
cargo run -q -p mr-cli -- test examples/house
```

The root `./test` helper launches the interactive `examples/house` game.

## Design Boundaries

- Rust owns saveable state: current room, visited rooms, inventory, thing locations, flags, counters, timers, random seed/state, and turn count.
- Lua owns author definitions and callbacks, but Lua state itself is not serialized.
- Keep Lua mutation behind the controlled callback `game` API in `mr-lua`.
- Core should emit events for scriptable behavior instead of depending on Lua directly.
- Prefer keeping template/CLI behavior in `mr-cli` until it clearly needs a shared crate.

## Lua DSL

Lua game files intentionally use these globals:

```lua
game { ... }
room "id" { ... }
thing "id" { ... }
verb "id" { ... }
event "id" { ... }
include "rooms.lua"
```

`include` loads a project-local Lua file relative to the including file. Includes must stay inside the game directory, are loaded at most once, and cyclic includes are rejected. Keep `.luarc.json` in sync with DSL globals.

Thing matching ignores one leading article (`the`, `a`, or `an`) and normalizes whitespace/case. Do not add article-prefixed aliases such as `"the key"` unless a future parser feature explicitly needs them.

Room exits support both shorthand and guarded table forms:

```lua
exits = {
  north = "hall",
  east = {
    to = "study",
    requires = "brass_key",
    locked_msg = "The study door is locked tight."
  }
}
```

The top-level `game { ... }` table can define global action hooks:

```lua
before_action = function(game, input) ... end
after_action = function(game, input) ... end
```

If `before_action` says output, it intercepts the command before core parsing. `after_action` runs after normal event/callback processing and appends output.

The top-level `game { ... }` table can also define engine settings. Currently supported:

```lua
settings = {
  exits = {
    show = true,
    label = "Available exits"
  }
}
```

Exit display is off by default. When enabled, rendered room descriptions append a sorted exit list using the configured label.

Things support basic containers and supporters:

```lua
thing "wooden_box" {
  name = "wooden box",
  aliases = { "box" },
  location = "foyer",
  portable = false,
  container = true
}

thing "table" {
  name = "table",
  aliases = { "table" },
  location = "foyer",
  portable = false,
  supporter = true
}
```

Portable things can be marked `wearable = true`. Worn items stay in inventory and are tracked in `GameState.worn`.

Things can define `read` text plus `on_take`, `on_drop`, `on_read`, and `on_use` callbacks. Core emits events for those actions and Lua can replace the default output.

Things can be marked `actor = true` and can define `on_talk = function(game) ... end`. Actors can also define `topics = { key = function(game) ... end }` for `ask actor about key`.

Core parser support includes `look in box`, `look on table`, `put key in box`, `put key on table`, `take key from box`, `use key`, `wear coat`, `remove coat`, `talk to caretaker`, and `ask caretaker about key`.

Timed events are registered with `event "name" { on_trigger = function(game) ... end }` and scheduled through `game.schedule(turns, name)`. Active timers live in `GameState.timers` and must remain serializable.

Lua callbacks can use `game.random(min, max)` for deterministic inclusive integer rolls. RNG seed/state live in `GameState` so transcript tests and save/load are reproducible.

Lua callbacks can use `game.visited(room_id)` for return-visit logic. Visited room ids live in `GameState.visited_rooms` and must remain serializable.

The repo has `.luarc.json` so LuaLS recognizes DSL globals. Update it if new DSL globals are added.

## Transcript Format

Transcript files live under a game project's `tests/` directory and use command blocks:

```text
> look
Room Name

Expected output.
```

Each block compares the output for that command only. Do not include prompt text in expected output.

## Notes

- The repo may not behave as a valid Git repository in this environment because `.git` is currently an empty directory.
- `save.json` is ignored and should be treated as local runtime output.
