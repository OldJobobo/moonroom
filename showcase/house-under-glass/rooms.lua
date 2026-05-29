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
    south = "foyer",
    east = "conservatory",
    west = "kitchen"
  },

  on_enter = function(game)
    local visits = game.inc_counter("hall_visits", 1)

    if visits == 1 then
      game.say("The floorboards answer your first step with a tired creak.")
    end
  end
}

room "kitchen" {
  name = "Kitchen",
  desc = function(game)
    if game.has_flag("house_released") then
      return "The kitchen smells faintly of rain and clean stone. The old cupboards stand open to ordinary air."
    end

    if game.has_flag("garden_cut_back") then
      return "The kitchen is narrow and white-tiled. A clean rectangle on the counter shows where the shears used to wait."
    end

    return "The kitchen is narrow and white-tiled. Its cupboards are too orderly for a room that should have fed people."
  end,
  exits = {
    east = "hall"
  }
}

room "conservatory" {
  name = "Conservatory",
  desc = function(game)
    if game.has_flag("rain_note_read") then
      return "Glass ribs hold the rain above ranks of empty planters. The cracked pane stands out now, a narrow wound in the careful room."
    end

    if game.has_flag("conservatory_seen") then
      return "Glass ribs hold the rain above ranks of empty planters. Something keeps tapping from inside the weather."
    end

    return "Glass ribs hold the rain above ranks of empty planters. The room smells of damp earth kept too long indoors."
  end,
  exits = {
    west = "hall"
  },

  on_enter = function(game)
    local visits = game.inc_counter("conservatory_visits", 1)
    game.flag("conservatory_seen")

    if visits == 1 then
      game.say("The conservatory air is warmer than the hall and much less alive.")
    end
  end,

  on_look = function(game)
    if game.has_flag("rain_note_read") and not game.visible("cracked_pane") then
      game.reveal("cracked_pane")
      game.say("With the rain note in mind, you spot the cracked pane behind the empty planters.")
    end
  end
}

room "upstairs_landing" {
  name = "Upstairs Landing",
  desc = function(game)
    if game.has_flag("bedroom_mirror_woken") then
      return "The upstairs landing narrows between glass balustrades. The bedroom mirror has left a pale shape in the air."
    end

    if game.has_flag("bedroom_door_open") then
      return "The upstairs landing narrows between glass balustrades. The bedroom door stands open, and the rain sounds farther away."
    end

    return "The upstairs landing narrows between glass balustrades. A closed bedroom door waits at the far end."
  end,
  exits = {
    down = "conservatory",
    north = "locked_bedroom"
  },

  on_enter = function(game)
    local visits = game.inc_counter("landing_visits", 1)

    if visits == 1 then
      game.say("You climb through the conservatory light into the upper house.")
    end
  end
}

room "locked_bedroom" {
  name = "Locked Bedroom",
  desc = function(game)
    if game.has_flag("bedroom_mirror_woken") then
      return "A narrow bed sits made and untouched. The mirror over the washstand no longer reflects the room correctly."
    end

    return "A narrow bed sits made and untouched. A covered mirror hangs over the washstand."
  end,
  exits = {
    south = "upstairs_landing"
  }
}

room "glass_room" {
  name = "Glass Room",
  desc = function(game)
    if game.has_flag("ending_written_name") then
      return "The Glass Room is quiet around your name. The old afternoon remains, but the windows show rain moving forward."
    end

    if game.has_flag("ending_left_open") then
      return "The Glass Room is losing its perfect afternoon by inches. The open ledger lets light and rain trade places on the table."
    end

    if game.has_flag("house_released") then
      return "The Glass Room is ordinary now: wet windows, a plain table, and air moving freely through the house."
    end

    if game.has_flag("glass_ledger_read") then
      return "The Glass Room holds a preserved afternoon in impossible light. The open ledger waits on the table like a held breath."
    end

    return "The Glass Room holds a preserved afternoon in impossible light. A ledger lies open on a plain table."
  end,
  exits = {
    west = "locked_bedroom"
  },

  on_enter = function(game)
    if not game.has_flag("entered_glass_room") then
      game.flag("entered_glass_room")
      game.say("The mirror gives way like cold water, and the Glass Room receives you without a sound.")
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
