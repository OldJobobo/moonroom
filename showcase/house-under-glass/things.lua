thing "tarnished_key" {
  name = "tarnished key",
  aliases = { "key", "tarnished key", "brass key" },
  location = "foyer",
  portable = true,
  desc = "The key is old brass, dark with tarnish. Its teeth are cut in an unfamiliar pattern.",

  on_take = function(game)
    if not game.has_flag("rain_memory_scheduled") then
      game.flag("rain_memory_scheduled")
      game.schedule(3, "rain_memory")
    end

    game.flag("touched_key")
    game.say("The key is colder than the room around it.")
  end,

  on_use = function(game)
    if game.room() == "foyer" and not game.has_flag("key_polished") then
      game.say("The key rasps in the study lock and stops. Tarnish fills the cuts.")
    elseif game.room() == "foyer" then
      game.say("The study lock accepts the polished key with a soft, almost grateful click.")
    elseif game.room() == "study" then
      game.say("The key warms near the open ledger, as if both remember the same hand.")
    else
      game.say("The key gives off a faint brass warmth.")
    end
  end
}

thing "linen_coat" {
  name = "linen coat",
  aliases = { "coat", "linen coat" },
  location = "foyer",
  portable = true,
  wearable = true,
  desc = "The coat is pale linen, too thin for weather and too formal for abandonment."
}

thing "hall_table" {
  name = "hall table",
  aliases = { "table", "hall table", "little table" },
  location = "foyer",
  portable = false,
  supporter = true,
  desc = "The table is narrow and dark, its varnish bubbled by years of damp air."
}

thing "wooden_box" {
  name = "wooden box",
  aliases = { "box", "wooden box" },
  location = "hall_table",
  portable = false,
  container = true,
  openable = true,
  open = false,
  lockable = true,
  locked = true,
  key = "tarnished_key",
  desc = "The box is cedar, its little brass lock kept bright by some more patient hand.",

  on_unlock = function(game)
    game.flag("wooden_box_unlocked")
    game.say("The polished key turns once in the wooden box. The lock gives up with a small domestic click.")
  end,

  on_open = function(game)
    if not game.visible("rain_note") then
      game.reveal("rain_note")
      game.say("You open the wooden box. A folded rain-soft note waits inside.")
    else
      game.say("You open the wooden box.")
    end
  end,

  on_close = function(game)
    game.say("You close the wooden box, and the cedar smell is sealed away again.")
  end
}

thing "rain_note" {
  name = "folded note",
  aliases = { "note", "folded note", "rain note" },
  location = "wooden_box",
  portable = true,
  hidden = true,
  desc = "The note has softened at every fold, as if read by wet hands.",
  read = "The note says: When the glass forgets the garden, look for the pane that remembers rain.",

  on_read = function(game)
    game.flag("rain_note_read")

    if game.chapter() ~= "names" then
      game.chapter("names")
    end

    if game.scene() ~= "rain_memory" then
      game.start_scene("rain_memory")
      game.schedule_scene(2, "rain_memory_scene")
    end
  end
}

thing "kitchen_cabinet" {
  name = "kitchen cabinet",
  aliases = { "cabinet", "kitchen cabinet", "cupboard", "white cupboard" },
  location = "kitchen",
  portable = false,
  container = true,
  openable = true,
  open = false,
  desc = "The cabinet doors are painted white. Their handles are polished from years of careful opening.",

  on_open = function(game)
    game.say("You open the kitchen cabinet. The hinges move without complaint.")
  end,

  on_close = function(game)
    game.say("You close the kitchen cabinet, restoring its careful face.")
  end
}

thing "garden_shears" {
  name = "pair of garden shears",
  aliases = { "shears", "garden shears", "clippers" },
  location = "kitchen_cabinet",
  portable = true,
  desc = "The shears are clean, sharp, and stored where no garden can reach them.",

  on_use = function(game)
    if game.room() == "conservatory" and not game.visible("cracked_pane") then
      game.flag("garden_cut_back")
      game.reveal("cracked_pane")
      game.say("You trim the dead stems away from the glass. Behind them, a cracked pane catches the rain.")
    elseif game.room() == "conservatory" then
      game.say("You cut away a little more dead growth, but the cracked pane is already clear.")
    else
      game.say("The shears open and close with a clean, dry click.")
    end
  end,

  on_use_with = function(game, item_id, target_id)
    if target_id == "cracked_pane" then
      game.say("You work the shears into the crack, but the glass has already shown you all it can.")
    else
      game.say("The shears are too clean and too particular for that.")
    end
  end
}

thing "study_ledger" {
  name = "black ledger",
  aliases = { "ledger", "black ledger", "book" },
  location = "study",
  portable = false,
  desc = "The ledger is bound in black cloth. A pale ribbon marks a page near the end.",
  read = "The ledger lists owners in a narrow hand. The final entry has no name, only a water stain shaped like a key.",

  on_read = function(game)
    game.flag("ledger_read")

    if game.chapter() ~= "names" then
      game.chapter("names")
    end
  end
}

thing "cracked_pane" {
  name = "cracked pane",
  aliases = { "pane", "glass pane", "cracked pane", "greenhouse pane" },
  location = "conservatory",
  portable = false,
  hidden = true,
  desc = "One pane has cracked in a line like handwriting. The rain finds it no matter how the roof leans.",
  read = "The crack is not lettering, but your eye keeps making a word from it: safe.",

  on_read = function(game)
    game.flag("safe_means_trapped")
  end,

  on_use = function(game)
    if game.has_flag("safe_means_trapped") then
      game.flag("upstairs_route_found")
      game.chapter("ascent")
      game.start_scene("upper_house")
      game.set_counter("landing_visits", 1)
      game["goto"]("upstairs_landing")
      game.say("You ease the cracked pane inward. Behind it, a narrow stair climbs through the wall of glass. You climb into the upper house.")
    else
      game.say("You press the cracked pane. It flexes inward, then settles back into its old refusal.")
    end
  end
}

thing "bedroom_door" {
  name = "bedroom door",
  aliases = { "door", "bedroom door", "glass knob" },
  location = "upstairs_landing",
  portable = false,
  openable = true,
  open = false,
  lockable = true,
  locked = true,
  key = "tarnished_key",
  desc = "The bedroom door is painted white. Its glass knob holds the shape of your hand after you let go.",

  on_unlock = function(game)
    game.flag("bedroom_door_unlocked")
    game.say("The polished key turns in the bedroom door with the reluctance of an old apology.")
  end,

  on_open = function(game)
    game.flag("bedroom_door_open")
    game.say("You open the bedroom door. Air slips out smelling of dust, linen, and rain that never reached the floor.")
  end,

  on_close = function(game)
    game.clear_flag("bedroom_door_open")
    game.say("You close the bedroom door. The glass knob cools at once.")
  end
}

thing "bedroom_mirror" {
  name = "covered mirror",
  aliases = { "mirror", "covered mirror", "bedroom mirror", "washstand mirror" },
  location = "locked_bedroom",
  portable = false,
  desc = "A linen sheet covers the mirror. The cloth is clean except where fingertips have worried the hem.",
  read = "There is no writing on the mirror, but the dust along the sheet has settled into a line: let the room change.",

  on_read = function(game)
    game.flag("mirror_message_read")
  end,

  on_use = function(game)
    if game.has_flag("mirror_message_read") and game.has_flag("ledger_read") then
      game.flag("glass_room_found")
      game.flag("entered_glass_room")
      game.chapter("release")
      game.start_scene("glass_room")
      game["goto"]("glass_room")
      game.say("You touch the mirror where the Glass Room waits in reflection. The surface gives way like cold water, and the room receives you without a sound.")
    else
      game.flag("bedroom_mirror_woken")
      game.start_scene("mirror_memory")
      game.say("You draw the sheet from the mirror. For a breath, it reflects the glass room instead of the bedroom.")
    end
  end
}

thing "memory_ledger" {
  name = "memory ledger",
  aliases = { "open ledger", "memory ledger", "blank page" },
  location = "glass_room",
  portable = false,
  openable = true,
  open = true,
  desc = "The ledger is open to a page without ink. The paper dimples as if rain has fallen from inside the room.",
  read = "The page lists no owner. It only repeats one phrase in a narrowing hand: kept safe, kept safe, kept safe.",

  on_read = function(game)
    game.flag("glass_ledger_read")
  end,

  on_close = function(game)
    game.flag("ending_closed_ledger")
    game.clear_flag("ending_left_open")
    game.clear_flag("ending_written_name")
    game.flag("house_released")
    game.chapter("after_rain")
    game.start_scene("house_released")
    game.say("You close the ledger. The preserved afternoon exhales, and rain begins to sound like weather instead of memory.")
  end,

  on_open = function(game)
    game.clear_flag("ending_closed_ledger")
    game.clear_flag("house_released")
    game.chapter("release")
    game.start_scene("glass_room")
    game.say("You open the ledger again. The room holds its breath, waiting to see whether you will keep it.")
  end
}
