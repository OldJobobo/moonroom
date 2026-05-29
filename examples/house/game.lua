game {
  id = "house-under-glass",
  version = "0.1.0",
  title = "The House Under Glass",
  author = "Example Author",
  start = "foyer",

  settings = {
    exits = {
      show = true
    }
  },

  before_action = function(game, input)
    if input == "listen" then
      game.say("Rain ticks against the glass with patient fingers.")
    end
  end,

  after_action = function(game, input)
    if input == "use key" and game.has_flag("key_polished") then
      game.say("For a moment, the house seems to listen.")
    end
  end
}

room "foyer" {
  name = "Foyer",
  desc = function(game)
    if game.visited("study") then
      return "Rain needles the windows. The study dust follows you. The table is bare now."
    end

    if game.has_flag("touched_key") then
      return "Rain needles the windows. The table is bare now."
    end

    return "Rain needles the windows. A brass key rests on the table."
  end,
  exits = {
    north = "hall",
    east = {
      to = "study",
      requires = "brass_key",
      locked_msg = "The study door is locked tight."
    }
  }
}

room "hall" {
  name = "Hall",
  desc = function(game)
    if game.has_flag("key_polished") then
      return "The hall is narrow, but the polished key throws a small gleam ahead."
    end

    return "The hall is narrow and unlit."
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
  desc = "Dusty shelves lean over a desk scarred by old candle wax.",
  exits = {
    west = "foyer"
  }
}

thing "brass_key" {
  name = "brass key",
  aliases = { "key", "brass key" },
  location = "foyer",
  portable = true,
  desc = "The brass key is cold and slightly tarnished.",

  on_take = function(game)
    if not game.has_flag("house_settling_scheduled") then
      game.flag("house_settling_scheduled")
      game.schedule(2, "house_settles")
    end

    game.flag("touched_key")
    game.say("The key is colder than it should be.")
  end,

  on_drop = function(game)
    game.say("The brass key lands without a sound.")
  end,

  on_use = function(game)
    if game.room() == "study" then
      game.say("You turn the brass key in the desk lock, but the drawer has already given up its secrets.")
    elseif game.has_flag("key_polished") then
      game.say("The polished key warms in your hand, eager for the study door.")
    else
      game.say("The tarnished key resists your thumb. It wants cleaning first.")
    end
  end
}

thing "linen_coat" {
  name = "linen coat",
  aliases = { "coat", "linen coat" },
  location = "foyer",
  portable = true,
  wearable = true,
  desc = "The coat is pale linen, too thin for this rain."
}

thing "wooden_box" {
  name = "wooden box",
  aliases = { "box", "wooden box" },
  location = "foyer",
  portable = false,
  container = true,
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

thing "caretaker" {
  name = "caretaker",
  aliases = { "caretaker", "old caretaker" },
  location = "hall",
  portable = false,
  actor = true,
  desc = "The caretaker stands as still as a coat on a peg.",

  on_talk = function(game)
    if game.has_flag("key_polished") then
      game.say("\"That key remembers more doors than this house has left,\" the caretaker says.")
    else
      game.say("The caretaker studies you, then looks away.")
    end
  end,

  topics = {
    key = {
      aliases = { "brass key", "study key" },

      ask = function(game, topic)
        if game.has_flag("key_polished") then
          local count = game.actor_memory("caretaker", "asked:" .. topic)
          if count == 1 then
            game.say("\"It was cut for the study before the study had a name,\" the caretaker says.")
          else
            game.say("\"I have told you what I know of the key,\" the caretaker says.")
          end
        else
          game.say("\"Polish it first,\" the caretaker says. \"Then ask me again.\"")
        end
      end,

      tell = function(game)
        game.say("The caretaker listens closely to your theory about the key.")
      end
    },

    house = {
      aliases = { "glass house", "old house" },
      requires = "key_polished",

      ask = function(game)
        game.say("\"The house remembers every hand that closes a door,\" the caretaker says.")
      end,

      tell = function(game)
        game.say("\"Then it remembers you now,\" the caretaker says.")
      end
    }
  },

  on_show = function(game, item_id)
    if item_id == "brass_key" and game.has_flag("key_polished") then
      game.say("The caretaker turns the polished key toward the light and nods once.")
    end
  end,

  on_give = function(game, item_id)
    if item_id == "brass_key" then
      game.say("\"Keep it,\" the caretaker says. \"It has chosen your pocket.\"")
    end
  end
}

event "house_settles" {
  on_trigger = function(game)
    game.say("Somewhere above you, the house settles with a soft wooden sigh.")
  end
}

verb "polish" {
  aliases = { "rub" },

  on_action = function(game, input)
    if input == "key" or input == "brass key" then
      if game.has("brass_key") then
        game.chapter("study")
        game.start_scene("polished_key")
        game.flag("key_polished")
        game.say("You polish the key with your sleeve until it catches the light.")
      else
        game.say("You need to be holding the key first.")
      end

      return
    end

    game.say("Polish what?")
  end
}

verb "flip" {
  on_action = function(game, input)
    if input ~= "key" and input ~= "brass key" then
      game.say("Flip what?")
      return
    end

    if not game.has("brass_key") then
      game.say("You need to be holding the key first.")
      return
    end

    if game.random(1, 2) == 1 then
      game.say("The key lands teeth-up in your palm.")
    else
      game.say("The key lands bow-up in your palm.")
    end
  end
}
