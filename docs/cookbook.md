# Moonroom Cookbook

## Locked exit

Use a guarded exit for a simple inventory gate:

```lua
exits = {
  east = { to = "study", requires = "brass_key", locked_msg = "The study door is locked." }
}
```

## Container and hidden clue

```lua
thing "box" { name = "wooden box", location = "foyer", container = true, openable = true }
thing "note" { name = "hidden note", location = "box", hidden = true, portable = true }
```

Reveal only from a callback: `game.reveal("note")`. Hidden state, open state, and locations are saved and undoable.

## Actor topic

```lua
thing "caretaker" {
  name = "caretaker", location = "hall", actor = true,
  topics = { key = { aliases = { "brass key" }, ask = function(game) game.say("It opens the study.") end } }
}
```

## Timed scene beat

Define `event "bell" { on_trigger = function(game) ... end }`, then call `game.schedule(2, "bell")` from a callback. For scene-local beats, start a scene and call `game.schedule_scene(2, "bell")`.

## Deterministic chance

Use `game.random(1, 6)` inside a callback. The random state is Rust-owned, so saves and transcript `--seed` runs reproduce it.

For complete signatures and ownership rules, use [the DSL reference](lua-dsl.md).
