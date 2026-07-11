# Your First Moonroom Game

Create a project, then run its checks and transcript before editing it:

```bash
moonroom new lantern-room
cd lantern-room
moonroom check .
moonroom test .
```

Replace `game.lua` with this small game:

```lua
game {
  id = "lantern-room",
  version = "0.1.0",
  title = "The Lantern Room",
  start = "cellar"
}

room "cellar" {
  name = "Cellar",
  desc = "A dry cellar beneath a narrow stair.",
  exits = { up = "yard" }
}

room "yard" {
  name = "Yard",
  desc = "Rain clears above the open gate.",
  exits = { down = "cellar" }
}

thing "lantern" {
  name = "brass lantern",
  aliases = { "lantern" },
  location = "cellar",
  portable = true,
  read = "The lantern is engraved: RETURN BEFORE DAWN."
}
```

Create `tests/opening.transcript`:

```text
> look
Cellar

A dry cellar beneath a narrow stair.

You can see a brass lantern.

> take lantern
You take the brass lantern.

> up
Yard

Rain clears above the open gate.
!room yard
```

Now run the complete author loop:

```bash
moonroom check .
moonroom test .
moonroom play .
moonroom pack . -o dist/lantern-room.moon
moonroom test dist/lantern-room.moon
```

Use `moonroom inspect .` when you want a compact listing of loaded rooms, things, exits, callbacks, verbs, and events. See [the Lua DSL reference](lua-dsl.md) for every available definition field.
