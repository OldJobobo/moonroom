# Lua DSL

Moonroom game files are Lua scripts that register story data through a small set of global DSL functions.

Small games can keep all definitions in `game.lua`. Larger games can split definitions across project-local Lua files with `include`:

```lua
include "rooms.lua"
include "things.lua"
include "dialogue.lua"
include "verbs.lua"
include "events.lua"
```

Included paths are relative to the file that calls `include`, must stay inside the game directory, and are loaded at most once. Cyclic includes are rejected.

## Game

```lua
game {
  title = "The House Under Glass",
  author = "Example Author",
  start = "foyer",

  settings = {
    exits = {
      show = false,
      label = "Available exits"
    }
  },

  before_action = function(game, input)
    if input == "listen" then
      game.say("Rain ticks against the glass.")
    end
  end,

  after_action = function(game, input)
    if input == "use key" then
      game.say("The house settles around the sound.")
    end
  end
}
```

`start` must be the id of a room defined in the same file.

`settings` is optional. Exit display is off by default. Set `settings.exits.show = true` to append an exit list to rendered room descriptions:

```text
Available exits: east, north.
```

`settings.exits.label` defaults to `"Available exits"` and can be customized.

`before_action` and `after_action` are optional global hooks. Both receive the controlled `game` API and the normalized command text. If `before_action` calls `game.say`, that output is returned and the command is not handled by the core parser. `after_action` runs after the normal command pipeline and appends any output it says.

## Rooms

```lua
room "foyer" {
  name = "Foyer",
  desc = "Rain needles the windows.",
  exits = {
    north = "hall"
  }
}
```

`desc` can also be a function:

```lua
desc = function(game)
  if game.has_flag("lamp_lit") then
    return "The hall opens into a warm pool of lamp light."
  end

  return "The hall is narrow and unlit."
end
```

The parser ignores one leading article when matching thing names and aliases, so authors should write aliases like `"key"` or `"brass key"` rather than `"the key"`.

Exits can be simple room ids:

```lua
exits = {
  north = "hall"
}
```

Or guarded by an inventory item:

```lua
exits = {
  east = {
    to = "study",
    requires = "brass_key",
    locked_msg = "The study door is locked tight."
  }
}
```

When `requires` is set, the player must be carrying that thing id before movement succeeds.

Supported room callbacks:

```lua
on_enter = function(game) ... end
on_look = function(game) ... end
```

## Things

```lua
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
  end,

  on_read = function(game)
    game.flag("read_key")
  end,

  on_use = function(game)
    game.say("The key fits your hand like it remembers you.")
  end
}
```

Things can also be containers or supporters:

```lua
thing "wooden_box" {
  name = "wooden box",
  aliases = { "box", "wooden box" },
  location = "foyer",
  portable = false,
  container = true,
  openable = true,
  open = false,
  desc = "The box is cedar, darkened by years of damp air."
}

thing "table" {
  name = "table",
  aliases = { "table" },
  location = "foyer",
  portable = false,
  supporter = true,
  desc = "The table is narrow enough to belong in a hall."
}
```

Openable things store their open/closed state in Rust-owned save data. Closed openable containers block `look in`, `put ... in`, `take ... from`, and nested object matching until opened.

Things can also be lockable:

```lua
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
  key = "brass_key",

  on_unlock = function(game)
    game.flag("chest_unlocked")
  end
}
```

`key` is optional. If present, it must be the id of a thing the player carries to `lock` or `unlock` the target. Lockable things can be unlocked with `unlock chest` when the player carries the required key, or explicitly with `unlock chest with brass key`.

Things can start hidden:

```lua
thing "folded_note" {
  name = "folded note",
  aliases = { "note" },
  location = "cedar_chest",
  portable = true,
  hidden = true
}
```

Hidden things are omitted from room descriptions, container/supporter contents, inventory listings, and parser matching until Lua reveals them with `game.reveal("folded_note")`. `game.hide("folded_note")` hides them again.

Portable things can be wearable:

```lua
thing "linen_coat" {
  name = "linen coat",
  aliases = { "coat", "linen coat" },
  location = "foyer",
  portable = true,
  wearable = true,
  desc = "The coat is pale linen, too thin for this rain."
}
```

Things can also be actors with talk callbacks:

```lua
thing "caretaker" {
  name = "caretaker",
  aliases = { "caretaker", "old caretaker" },
  location = "hall",
  portable = false,
  actor = true,
  desc = "The caretaker stands as still as a coat on a peg.",

  on_talk = function(game)
    game.say("\"That key remembers more doors than this house has left,\" the caretaker says.")
  end,

  topics = {
    key = function(game)
      game.say("\"It was cut for the study before the study had a name,\" the caretaker says.")
    end,

    house = {
      aliases = { "glass house", "old house" },
      requires = "knows_house",

      ask = function(game, topic)
        local count = game.actor_memory("caretaker", "asked:" .. topic)
        game.say("\"The house has been waiting,\" the caretaker says.")
      end,

      tell = function(game, topic)
        game.say("The caretaker nods at your story.")
      end
    }
  },

  on_show = function(game, item_id)
    game.say("The caretaker studies the " .. item_id .. ".")
  end,

  on_give = function(game, item_id)
    game.move(item_id, "caretaker")
    game.say("The caretaker accepts it.")
  end
}
```

Topic shorthand keeps working:

```lua
topics = {
  key = function(game) ... end
}
```

Table-form topics can define `aliases`, a flag-gated `requires` condition, `ask`/`on_ask`, and `tell`/`on_tell`. Topic aliases are normalized like thing aliases, so `ask caretaker about the old house` can resolve to the canonical `house` topic. If `requires = "flag_name"` is set, the topic is unavailable until that Rust-owned flag exists.

Actor memory is Rust-owned and saved. The engine automatically increments these memory counters when the matching command succeeds:

```text
asked:topic_id
told:topic_id
shown:thing_id
given:thing_id
```

Callbacks can also manage their own actor memory keys with `game.set_actor_memory` and `game.inc_actor_memory`.

Multi-step dialogue can branch on actor memory:

```lua
topics = {
  key = {
    ask = function(game, topic)
      if game.actor_memory("caretaker", "asked:" .. topic) == 1 then
        game.say("\"First time asking? The key is old,\" the caretaker says.")
      else
        game.say("\"I already told you about the key,\" the caretaker says.")
      end
    end
  }
}
```

Supported interactions include:

```text
look in box
look on table
put key in box
put key on table
take key from box
open box
close box
unlock chest with key
lock chest
read note
use key
wear coat
remove coat
talk to caretaker
ask caretaker about key
tell caretaker about house
show key to caretaker
give key to caretaker
again
undo
```

`again` or `g` repeats the last advancing command. `undo` restores the Rust-owned game state from before the last advancing command, including state changed by Lua callbacks. The undo history is bounded and is not written into save files.

Supported thing callbacks:

```lua
on_take = function(game) ... end
on_drop = function(game) ... end
on_read = function(game) ... end
on_open = function(game) ... end
on_close = function(game) ... end
on_lock = function(game) ... end
on_unlock = function(game) ... end
on_use = function(game) ... end
on_talk = function(game) ... end
on_show = function(game, item_id) ... end
on_give = function(game, item_id) ... end
topics = {
  key = function(game) ... end,
  house = {
    aliases = { "glass house" },
    requires = "knows_house",
    ask = function(game, topic) ... end,
    tell = function(game, topic) ... end
  }
}
```

## Custom Verbs

```lua
verb "polish" {
  aliases = { "rub" },

  on_action = function(game, input)
    if input == "key" and game.has("brass_key") then
      game.flag("key_polished")
      game.say("You polish the key with your sleeve.")
    else
      game.say("Polish what?")
    end
  end
}
```

The `input` argument is the text after the verb.

## Timed Events

Events are named Lua callbacks that can be scheduled from other callbacks:

```lua
event "house_settles" {
  on_trigger = function(game)
    game.say("Somewhere above you, the house settles.")
  end
}
```

Schedule or cancel them with the game API:

```lua
game.schedule(2, "house_settles")
game.cancel("house_settles")
```

The first argument to `schedule` is the number of advancing turns before the event fires. Timers are saved as Rust-owned state.

## Game API

Lua callbacks receive a controlled `game` object.

```lua
game.say(text)
game.flag(name)
game.clear_flag(name)
game.has_flag(name)
game.counter(name)
game.set_counter(name, value)
game.inc_counter(name, amount)
game.actor_memory(actor_id, key)
game.set_actor_memory(actor_id, key, value)
game.inc_actor_memory(actor_id, key, amount)
game.move(thing_id, location_id)
game.goto(room_id)
game.has(thing_id)
game.visible(thing_id)
game.hide(thing_id)
game.reveal(thing_id)
game.room()
game.visited(room_id)
game.turn()
game.random(min, max)
game.schedule(turns, event_name)
game.cancel(event_name)
```

`game.visited(room_id)` returns whether the player has occupied that room. Visited rooms are tracked by the Rust engine and saved.

`game.visible(thing_id)` returns whether a thing is currently revealed. Hidden/revealed state is tracked by the Rust engine and saved.

`game.actor_memory(actor_id, key)` returns a saved actor-specific counter. Missing keys return `0`.

`game.random(min, max)` returns a deterministic integer in the inclusive range. The random seed/state is saved with the rest of the engine state, so transcript tests and save/load flows remain reproducible.

Save files serialize Rust-owned state only: current room, visited rooms, inventory, worn items, thing locations, open/locked thing state, hidden/revealed thing state, flags, counters, actor memory, active timers, random seed/state, and turn count. Lua state and Lua globals are not saved.

## Transcript Tests

Transcript files live under `tests/` and use command blocks:

```text
> look
Foyer

Rain needles the windows.

> take key
The key is colder than it should be.
!flag touched_key
!counter keys_taken 1

> north
Hall

The hall is narrow and unlit.
!room hall
```

Assertion directives start with `!` and are checked after the command output:

```text
!room room_id
!flag flag_name
!counter counter_name integer_value
```

Directive lines are not included in output comparison.

Run them with:

```bash
moonroom test path/to/game
```
