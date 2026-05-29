# Moonroom Plan

Moonroom is an interactive fiction engine written in Rust, with game worlds defined in scriptable Lua.

Rust owns the engine, parser, state, saves, testing, packaging, and frontends. Lua owns world definition: rooms, things, verbs, dialogue, events, conditions, and scripted callbacks.

## Naming

Use `Moonroom` for the public project identity and `moonroom` for the user-facing command.

Use `mr-*` for internal Rust crates:

```text
moonroom = product, command, docs, engine name
mr-*     = internal workspace crates
```

Example commands:

```bash
moonroom play examples/house
moonroom test examples/house
moonroom new my-game
```

The short `mr` command can be added later as an alias, but the primary command should stay `moonroom` for discoverability.

## Core Goals

1. Let authors define games in readable Lua.
2. Keep the Rust engine deterministic, testable, and save-friendly.
3. Support classic parser fiction first: `look`, `go north`, `take key`, `use lamp`.
4. Allow Lua scripting without giving Lua uncontrolled access to engine internals.
5. Make games easy to test with transcript files.
6. Keep the authoring model declarative by default, with imperative Lua callbacks only where they add value.

## Stack

Initial stack:

```text
Rust
mlua
serde
serde_json or ron
clap
rustyline or reedline
anyhow
thiserror
```

Potential later additions:

```text
ratatui       -> richer terminal UI
axum          -> browser frontend
wasm          -> web export, if feasible
```

## Workspace Shape

```text
moonroom/
  Cargo.toml
  PLAN.md
  README.md

  crates/
    mr-core/      -> world model, state, parser, actions
    mr-lua/       -> mlua bindings and Lua DSL loading
    mr-cli/       -> command-line frontend
    mr-tui/       -> future terminal UI
    mr-test/      -> transcript test runner
    mr-save/      -> save/load format, if it grows beyond core

  examples/
    house/
      game.lua
      rooms.lua
      things.lua
      verbs.lua
      tests/
        opening.transcript
```

At first, `mr-save` can be folded into `mr-core`. Split it out only if persistence becomes large enough to justify its own crate.

## Crate Responsibilities

### `mr-core`

Owns the engine rules and serializable game state.

Responsibilities:

```text
world model
rooms
things
actors
inventory
flags
counters
parser intent model
action resolution
turn pipeline
output transcript buffer
deterministic RNG hooks
save-state structs
```

### `mr-lua`

Owns the Lua runtime and authoring DSL.

Responsibilities:

```text
load Lua files
register rooms
register things
register verbs
bind callbacks
expose controlled game API to Lua
convert Lua values into mr-core definitions
report useful Lua source errors
```

### `mr-cli`

Owns the playable command-line app.

Responsibilities:

```text
moonroom play
moonroom test
moonroom new
interactive prompt
save/load commands
basic project template generation
```

### `mr-tui`

Future richer terminal frontend.

Responsibilities:

```text
scrollback transcript
status pane
inventory pane
command input
theme support
```

### `mr-test`

Owns transcript testing.

Responsibilities:

```text
load transcript files
run commands against the engine
compare expected output
support deterministic RNG
report concise diffs
```

## Game Project Shape

A Moonroom game should look like this:

```text
my-game/
  game.lua
  rooms.lua
  things.lua
  verbs.lua
  dialogue.lua
  assets/
  tests/
    opening.transcript
```

Small games can keep everything in `game.lua`. Larger games can split definitions across files.

## Lua Authoring Style

Lua should feel like writing story data, not programming the whole engine.

Use `game` as the callback argument name instead of `ctx`. It is friendlier for story authors and reads naturally.

```lua
game {
  title = "The House Under Glass",
  author = "Example Author",
  start = "foyer"
}

room "foyer" {
  name = "Foyer",
  desc = "Rain needles the windows. A brass key rests on the table.",
  exits = {
    north = "hall"
  }
}

room "hall" {
  name = "Hall",
  desc = function(game)
    if game.has_flag("lamp_lit") then
      return "The hall opens into a warm pool of lamp light."
    end

    return "The hall is narrow and unlit."
  end
}

thing "brass_key" {
  name = "brass key",
  aliases = { "key", "brass key" },
  location = "foyer",
  portable = true,

  on_take = function(game)
    game.say("The key is colder than it should be.")
    game.flag("touched_key")
  end
}
```

## Lua API

Lua scripts should mutate the world only through a controlled engine API.

Initial API:

```lua
game.say(text)
game.flag(name)
game.clear_flag(name)
game.has_flag(name)
game.counter(name)
game.set_counter(name, value)
game.inc_counter(name, amount)
game.move(thing_id, location_id)
game.goto(room_id)
game.has(thing_id)
game.room()
game.turn()
game.random(min, max)
```

Possible later API:

```lua
game.scene(name)
game.schedule(turns, event_name)
game.cancel(event_name)
game.actor(actor_id)
game.visible(thing_id)
game.choice(options)
```

## Rust-Owned State

The save file should serialize engine-owned state, not raw Lua state or the Lua stack.

Save data should include:

```text
current room
inventory
object locations
flags
counters
visited rooms
turn count
random seed
active timers/events
```

This keeps saves stable across Lua reloads and makes transcript tests deterministic.

## Turn Pipeline

```text
1. Read player input.
2. Parse command into an intent.
3. Resolve verb and targets.
4. Run before-action hooks.
5. Execute built-in or Lua action.
6. Mutate world state.
7. Run after-action hooks.
8. Advance time and scheduled events.
9. Print response.
```

## Parser Strategy

Start simple. Do not build a natural-language parser too early.

Phase 1 grammar:

```text
verb
verb noun
verb noun with noun
direction aliases
object aliases
```

Examples:

```text
look
inventory
take key
x brass key
go north
n
unlock door with brass key
```

Internal shape:

```rust
Intent {
    verb: "unlock",
    direct_object: Some("door"),
    preposition: Some("with"),
    indirect_object: Some("brass_key"),
}
```

## MVP Features

1. Load `game.lua`.
2. Define rooms, exits, things, and inventory.
3. Support built-in commands:

```text
look
inventory / i
go north / n
take item
drop item
examine item / x item
quit
save
load
```

4. Support Lua callbacks:

```text
on_enter
on_look
on_take
on_drop
on_use
before_action
after_action
```

5. Support JSON or RON save/load.
6. Provide CLI runner:

```bash
moonroom play path/to/game
```

7. Provide transcript tests:

```bash
moonroom test path/to/game
```

## Milestones

### Milestone 1: Core Prototype

```text
Rust CLI
load Lua file
register rooms and things
move between rooms
look, take, drop, inventory
basic output buffer
```

Target:

```bash
moonroom play examples/house
```

Playable loop:

```text
Foyer

Rain needles the windows. A brass key rests on the table.

> take key
The key is colder than it should be.

> north
Hall

The hall is narrow and unlit.
```

### Milestone 2: Scripted World

```text
Lua callbacks
flags and counters
custom verbs
conditional descriptions
useful errors with Lua source locations
```

### Milestone 3: Persistence and Tests

```text
save/load
transcript testing
deterministic RNG
golden output tests
```

### Milestone 4: Author Experience

```text
better error messages
project template
example game
documentation
hot reload during development
```

### Milestone 5: Richer IF Features

```text
containers
supporters
locked doors
wearables
actors and NPCs
timed events
dialogue trees
scenes and chapters
```

## First Concrete Build Target

Build the smallest real slice:

```bash
moonroom play examples/house
```

That should prove:

```text
Rust can load Lua-authored game data.
The player can move through rooms.
The player can inspect and manipulate things.
Lua callbacks can produce output and mutate engine state.
The engine state remains serializable.
```

Once that works, add transcript tests before making the parser more ambitious.

## Expanded Roadmap

The original milestone plan gets Moonroom to a playable, scriptable parser-fiction engine. The next phase should turn it into a stronger authoring platform: scalable project structure, better parser ergonomics, richer world state, safer save files, and tooling that helps authors understand their games before players do.

### Milestone 6: Project Structure

```text
multi-file game loading from game.lua as the entrypoint
project-local include helper for rooms.lua, things.lua, verbs.lua, dialogue.lua, and events.lua
safe path handling so included files cannot escape the game directory
clear Lua source errors that preserve the included file path
cycle/duplicate include behavior defined explicitly
moonroom new can keep generating one-file games, with split-file templates later
```

Target:

```lua
-- game.lua
game {
  title = "The House Under Glass",
  start = "foyer"
}

include "rooms.lua"
include "things.lua"
include "dialogue.lua"
include "verbs.lua"
include "events.lua"
```

Small games should still be able to keep everything in `game.lua`. Larger games should be able to split definitions across files without changing the runtime model.

This milestone should happen before the showcase grows beyond its scaffold.

### Milestone 7: Parser Quality

```text
again / g to repeat the last advancing command
undo with bounded Rust-owned state history
pronouns such as it / them after object references
disambiguation when multiple visible objects match the same name
common parser verbs: open, close, unlock, lock, read, give, show
clearer parser failure messages tied to visible world state
```

Target:

```text
> take key
You take the brass key.

> x it
The brass key is cold and slightly tarnished.

> g
You see nothing new about the brass key.

> undo
Undone.
```

### Milestone 8: Object State

```text
openable and closable containers
lockable objects and exits
hidden and revealed things
light and dark room support
edible and drinkable things
fixed scenery objects that do not clutter room listings
```

Core state should remain serializable. Prefer explicit Rust-owned object state over ad hoc Lua globals for anything that must survive save/load.

### Milestone 9: Dialogue System

```text
topic aliases: key, brass key, door key
topic availability conditions
tell actor about topic
give item to actor
show item to actor
multi-step dialogue trees
actor memory stored in Rust-owned state
```

The current `talk` and `ask actor about topic` model is enough for simple NPCs. This milestone should make conversation useful for puzzles and longer scenes without requiring each game to invent its own dialogue framework.

### Milestone 10: Scenes and Chapters

```text
current scene/chapter in Rust-owned state
game.scene()
game.start_scene(name)
game.end_scene(name)
game.chapter(name)
scene-scoped timers and hooks
scene/chapter assertions in transcript tests
```

Scenes should be optional structure for authors, not a requirement for small games.

### Milestone 11: Author Tooling

```text
moonroom check path/to/game
static validation for missing rooms, exits, thing locations, and callbacks
duplicate alias warnings
invalid guarded-exit target checks
moonroom inspect path/to/game for rooms, things, exits, verbs, events, and callbacks
moonroom transcript path/to/game to record a play session into a transcript file
better Lua load errors with DSL-specific context
```

This is likely the highest-leverage author experience milestone. As the DSL grows, mistakes should be caught before a player transcript stumbles into them.

### Milestone 12: Save Format Hardening

```text
save format version
game id and game version in save files
reject saves from different games
save migrations for older formats
compact and pretty save output modes
backward compatibility tests for representative old saves
```

The save format should be treated as a public contract once games start depending on it.

### Milestone 13: Testing Improvements

```text
transcript directives: !contains, !not_contains, !room, !flag, !counter
test filtering by transcript name
golden transcript update mode
random seed override for tests
better diffs with command context
fixture games for parser and state regressions
```

Transcript tests should remain readable story artifacts, but they need enough structure to verify state without forcing authors to expose everything through prose.

### Milestone 14: Packaging and Distribution

```text
installable moonroom binary
moonroom pack to package a game directory into a .moon file
moonroom play/check/test support for both game directories and .moon packages
single-file .moon game bundle format
moonroom unpack for inspection, recovery, and tooling
moonroom build --standalone to produce one executable with an embedded .moon package
release profile checks
example games as distribution fixtures
documented game project versioning
```

This milestone should make it realistic to hand a Moonroom game to someone who is not working inside the repository.

Moonroom should support three first-class game distribution forms:

```text
source folder      editable author format with Lua files on disk
.moon package      portable single-file release bundle
standalone binary  executable runner with an embedded .moon package
```

Target usage:

```bash
moonroom play my-game/
moonroom pack my-game/ -o dist/my-game.moon
moonroom play dist/my-game.moon
moonroom check dist/my-game.moon
moonroom test dist/my-game.moon
moonroom unpack dist/my-game.moon -o unpacked-my-game
moonroom build my-game/ --standalone -o dist/my-game
```

The folder format should remain the normal authoring experience. The `.moon` package should be the normal sharing format for players and reviewers. Standalone builds should be the most convenient player-facing export when the author wants to distribute one executable instead of requiring a separate Moonroom install.

The implementation should introduce a small game-source abstraction so the Lua loader does not care whether `game.lua` and included files come from a directory, a `.moon` package, or embedded bytes:

```rust
enum GameSource {
    Directory(PathBuf),
    Package(PathBuf),
    Embedded(&'static [u8]),
}
```

The package should preserve Moonroom's existing project model by storing a project-local virtual filesystem. Includes should continue to use paths relative to the including Lua file and must not escape the packaged game root.

Initial `.moon` package shape:

```text
my-game.moon
  moon.json
  game.lua
  rooms.lua
  things.lua
  dialogue.lua
  verbs.lua
  assets/
  tests/
```

Initial `moon.json` shape:

```json
{
  "format": "moonroom.moon",
  "version": 1,
  "entry": "game.lua",
  "title": "The House Under Glass",
  "author": "Example Author"
}
```

Start with a simple archive-backed `.moon` format. Later release modes can strip/minify Lua or optionally store Lua bytecode to make distributed game code less casually readable. These modes should be treated as convenience and polish, not as strong DRM; any game that runs locally can be reverse engineered by a determined user.

### Milestone 15: Frontends

```text
keep CLI as the canonical frontend
mr-tui with scrollback, status pane, inventory pane, and command input
JSON event stream mode for external interfaces
browser frontend using the same engine/state model
possible wasm export if the Lua/runtime constraints are acceptable
```

New frontends should consume the same engine outputs rather than forking parser or world behavior.

### Milestone 16: Documentation as Product

```text
build your first Moonroom game tutorial
DSL reference split by topic
parser command reference
save/load model explanation
testing guide
authoring cookbook
```

Cookbook examples should include:

```text
locked door
NPC with topics
timed event
hidden item
openable container
deterministic random event
scene transition
save-compatible puzzle state
```

## Recommended Next Work

Completed from this list:

```text
multi-file loading, because the planned project shape and showcase scaffold already need it.
moonroom check, with static validation for world graph errors and duplicate object vocabulary.
read, with thing-authored read text and on_read callbacks for inspectable clues.
open, close, lock, and unlock for thing-owned object state, including saved open/locked state and Lua callbacks.
again and undo, with bounded Rust-owned command history and Lua callback state rollback.
hidden/revealed things, with saved visibility state and controlled Lua hide/reveal helpers.
transcript directives for !room, !flag, and !counter state assertions.
Milestone 9 dialogue basics: topic aliases, flag-gated topic availability, tell/show/give commands, table-form topic callbacks, and Rust-owned actor memory counters.
```

The next implementation pass should prioritize:

```text
1. broader moonroom check source context and optional warnings as the DSL grows.
2. scenery things, because they let authors add inspectable detail without cluttering room listings.
3. light and dark room support, because it unlocks a classic parser-fiction constraint while exercising saved world state.
```
