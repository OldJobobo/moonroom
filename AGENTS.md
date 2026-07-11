# Moonroom Agent Notes

Moonroom is a Rust workspace for an interactive fiction engine with Lua-authored game worlds. Treat `PLAN.md` as the active 0.1 release plan and architecture contract, `ROADMAP.md` as deferred product direction, and `README.md` as the current user-facing quickstart.

## Workspace

- `crates/mr-core`: serializable world model, game state, parser/action handling, and core events.
- `crates/mr-lua`: Lua DSL loading, callback registry, controlled `game` API, and JSON save/load of Rust-owned state.
- `crates/mr-cli`: command-line play, author tooling, packaging, and standalone builds.
- `crates/mr-test`: transcript parsing and test runner.
- `examples/house`: the main integration example.
- `showcase/house-under-glass`: the release-driving proof game.
- `docs/lua-dsl.md`: author-facing Lua DSL reference.

## Release Focus

The next target is Moonroom 0.1: a dependable authoring release proving this complete workflow:

```text
create a game -> author it -> check it -> test it -> package it -> play it
```

Work through the active phases in `PLAN.md` in order unless a later task directly unblocks an earlier one:

1. Author feedback and actionable diagnostics.
2. Parser correctness and deterministic ambiguity handling.
3. Save and package safety.
4. Author documentation.
5. The 0.1 release workflow and polished showcase.

Prefer correctness, diagnostics, compatibility fixtures, and the House Under Glass release path over expanding the DSL. Add scenery before 0.1 only if the showcase materially needs it. Keep darkness/light, edible or drinkable things, plural pronouns, declarative dialogue, hot reload, `game.choice()`, TUI, browser, and WASM work in `ROADMAP.md` until their prerequisites and concrete user need are proven.

Do not silently promote a `ROADMAP.md` idea into active work. Move it into `PLAN.md` only with a release target, bounded acceptance criteria, defined Rust/Lua ownership, save and undo behavior, validation, tests, and documentation.

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

Inspect the loaded world and callbacks:

```bash
cargo run -q -p mr-cli -- inspect examples/house
```

Record a transcript:

```bash
cargo run -q -p mr-cli -- transcript examples/house -o examples/house/tests/recorded.transcript
```

Package and inspect release bundles:

```bash
cargo run -q -p mr-cli -- pack examples/house -o dist/house.moon
cargo run -q -p mr-cli -- play dist/house.moon
cargo run -q -p mr-cli -- check dist/house.moon
cargo run -q -p mr-cli -- test dist/house.moon
cargo run -q -p mr-cli -- unpack dist/house.moon -o unpacked-house
cargo run -q -p mr-cli -- build examples/house --standalone -o dist/house
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
cargo run -q -p mr-cli -- inspect examples/house
cargo run -q -p mr-cli -- check showcase/house-under-glass
cargo run -q -p mr-cli -- test showcase/house-under-glass
```

The root `./test` helper launches the interactive `examples/house` game.

## Design Boundaries

- Rust owns saveable state: current room, visited rooms, inventory, thing locations, open/locked thing state, flags, counters, timers, random seed/state, and turn count.
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

`include` loads a project-local Lua file relative to the including file. Includes must stay inside the game directory or packaged `.moon` root, are loaded at most once, and cyclic includes are rejected. Keep `.luarc.json` in sync with DSL globals.

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

The top-level `game { ... }` table can define optional save identity metadata:

```lua
id = "stable-game-id"
version = "0.1.0"
```

Versioned save files include the game id/title/version and reject loading when the save's game id does not match the currently loaded game. If `id` is omitted, Moonroom uses the title as the save compatibility id. Legacy raw `GameState` JSON saves still load.

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
  container = true,
  openable = true,
  open = false
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

Things can define `read` text plus `on_take`, `on_drop`, `on_read`, `on_open`, `on_close`, `on_lock`, `on_unlock`, and `on_use` callbacks. Core emits events for those actions and Lua can replace the default output.

Openable things use `openable = true` and optional initial `open = true`. Lockable things use `lockable = true`, optional initial `locked = true`, and optional `key = "thing_id"`. Open/locked state lives in `GameState.open_things` and `GameState.locked_things`.

Things can start hidden with `hidden = true`. Hidden things are omitted from room descriptions, container/supporter contents, inventory listings, and parser matching until Lua calls `game.reveal("thing_id")`. Lua can call `game.hide("thing_id")` to hide a thing again and `game.visible("thing_id")` to query reveal state. Hidden/revealed state lives in `GameState.hidden_things`.

Things can be marked `actor = true` and can define `on_talk = function(game) ... end`. Actors can also define `topics = { key = function(game) ... end }` for `ask actor about key`, or table-form topics with aliases, `requires`, `ask`, and `tell` callbacks. Actors can define `on_show` and `on_give` callbacks. Actor memory lives in `GameState.actor_memory`.

Core parser support includes `look in box`, `look on table`, `put key in box`, `put key on table`, `take key from box`, `open box`, `close box`, `unlock chest with key`, `lock chest`, `use key`, `wear coat`, `remove coat`, `talk to caretaker`, `ask caretaker about key`, `tell caretaker about key`, `show key to caretaker`, `give key to caretaker`, `again`/`g`, and `undo`.

`again` repeats the last advancing command. `undo` restores Rust-owned state from before the last advancing command, including Lua callback mutations when commands run through `LuaGame`. Undo history is bounded and is not serialized into save files.

Timed events are registered with `event "name" { on_trigger = function(game) ... end }` and scheduled through `game.schedule(turns, name)`. Scene-scoped timers use `game.schedule_scene(turns, name)` and only fire if the same scene is active when due. Active timers live in `GameState.timers` and must remain serializable.

Scenes and chapters are optional Rust-owned story structure. Lua can use `game.scene()`, `game.start_scene(name)`, `game.end_scene(name)`, and `game.chapter(name)`. Current scene/chapter live in `GameState.current_scene` and `GameState.current_chapter`. Top-level `game { ... }` can define `on_scene_start`, `on_scene_end`, and `on_chapter` hooks.

The in-game `save` command writes pretty versioned JSON by default. `save --compact path` or `save -c path` writes compact JSON.

Lua callbacks can use `game.random(min, max)` for deterministic inclusive integer rolls. RNG seed/state live in `GameState` so transcript tests and save/load are reproducible.

Lua callbacks can use `game.visited(room_id)` for return-visit logic. Visited room ids live in `GameState.visited_rooms` and must remain serializable.

The repo has `.luarc.json` so LuaLS recognizes DSL globals. Update it if new DSL globals are added.

## Transcript Format

Transcript files live under a game project's `tests/` directory and use command blocks:

```text
> look
Room Name

Expected output.

> take key
You take the key.
!flag touched_key
```

Each block compares the output for that command only. Do not include prompt text in expected output. Assertion directives beginning with `!` are checked after the command and are excluded from output comparison. Supported directives are `!contains text`, `!not_contains text`, `!room room_id`, `!scene scene_name`, `!scene none`, `!chapter chapter_name`, `!chapter none`, `!flag flag_name`, and `!counter counter_name integer_value`.

`moonroom test` supports `--filter text` to run matching transcript paths, `--seed integer` to override deterministic random state per transcript, and `--update` to refresh expected output while preserving directive lines.

`.moon` files are single-file release packages. `moonroom play`, `check`, and `test` accept either a source folder or a `.moon` package. `moonroom pack` writes the package, `moonroom unpack` restores the virtual files to a folder, and `moonroom build --standalone` copies the current Moonroom executable with an embedded `.moon` payload.

## Notes

- `save.json` is ignored and should be treated as local runtime output.
- Lua projects and `.moon` packages are trusted executable content. Robust input limits do not constitute a sandbox.
