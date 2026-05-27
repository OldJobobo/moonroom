room "foyer" {
  name = "Foyer",
  desc = function(game)
    if game.visited("study") then
      return "Rain needles the glass around the front door. Study dust has found the cracks in the tile."
    end

    if game.has_flag("touched_key") then
      return "Rain needles the glass around the front door. The little table stands bare."
    end

    return "Rain needles the glass around the front door. A tarnished key waits on the little table."
  end,
  exits = {
    north = "hall",
    east = {
      to = "study",
      requires = "tarnished_key",
      locked_msg = "The study door has no handle, only a narrow brass keyhole."
    }
  }
}

room "hall" {
  name = "Hall",
  desc = function(game)
    if game.has_flag("ledger_read") then
      return "The hall is long and glass-framed. Rain sketches the names from the ledger against the panes."
    end

    if game.has_flag("key_polished") then
      return "The hall is long and glass-framed. The polished key throws a small gold mark ahead of you."
    end

    return "The hall is long and glass-framed, with a caretaker standing where the shadows gather."
  end,
  exits = {
    south = "foyer"
  },

  on_enter = function(game)
    local visits = game.inc_counter("hall_visits", 1)

    if visits == 1 then
      game.say("The floorboards answer your first step with a tired creak.")
    end
  end
}

room "study" {
  name = "Study",
  desc = function(game)
    if game.has_flag("ledger_read") then
      return "Dusty shelves lean over a desk scarred by old candle wax. The ledger lies open to the page of missing owners."
    end

    return "Dusty shelves lean over a desk scarred by old candle wax. A black ledger waits beneath the lamp."
  end,
  exits = {
    west = "foyer"
  }
}
