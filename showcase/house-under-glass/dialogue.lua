thing "caretaker" {
  name = "caretaker",
  aliases = { "caretaker", "old caretaker" },
  location = "hall",
  portable = false,
  actor = true,
  desc = "The caretaker stands with one gloved hand resting against the glass.",

  on_talk = function(game)
    if game.has_flag("rain_note_read") then
      game.say("\"Then you have found what the rain kept,\" the caretaker says.")
    elseif game.has_flag("ledger_read") then
      game.say("\"Then you have seen how the house keeps its names,\" the caretaker says.")
    elseif game.has_flag("key_polished") then
      game.say("\"Bright enough to remember the lock now,\" the caretaker says, nodding toward the study.")
    else
      game.say("The caretaker looks from you to the tarnished key and waits.")
    end
  end,

  topics = {
    key = {
      aliases = { "brass key", "tarnished key", "study key" },

      ask = function(game, topic)
        if game.has_flag("key_polished") then
          local count = game.actor_memory("caretaker", "asked:" .. topic)

          if count <= 2 then
            game.say("\"It was cut for the study before the study had a door,\" the caretaker says.")
          else
            game.say("\"The key remembers enough now,\" the caretaker says. \"Ask what the door was protecting.\"")
          end
        else
          game.say("\"Brass forgets under tarnish,\" the caretaker says. \"Polish it, then ask again.\"")
        end
      end,

      tell = function(game)
        game.say("The caretaker listens to your theory about the key and folds his gloved hands tighter.")
      end
    },

    ledger = {
      aliases = { "black ledger", "book", "names" },

      ask = function(game)
        if game.has_flag("ledger_read") then
          game.say("\"Every owner signed it,\" the caretaker says. \"The house kept the rest of the handwriting.\"")
        else
          game.say("\"The ledger is not hidden,\" the caretaker says. \"Only waiting.\"")
        end
      end,

      tell = function(game)
        if game.has_flag("ledger_read") then
          game.say("\"Then you know why the empty line matters,\" the caretaker says.")
        else
          game.say("\"Tell me after you have read it,\" the caretaker says.")
        end
      end
    },

    house = {
      aliases = { "glass house", "old house" },

      ask = function(game)
        game.say("\"The house remembers best through glass,\" the caretaker says.")
      end,

      tell = function(game)
        game.say("\"It hears you,\" the caretaker says, so quietly the rain nearly takes the words.")
      end
    },

    glass_room = {
      aliases = { "glass room", "sealed room", "room", "door" },
      requires = "ledger_read",

      ask = function(game)
        if game.has_flag("ending_written_name") then
          game.say("\"Then the house will remember you as a guest, not a possession,\" the caretaker says.")
        elseif game.has_flag("ending_left_open") then
          game.say("\"Then it will forget slowly,\" the caretaker says. \"That may be a mercy too.\"")
        elseif game.has_flag("house_released") then
          game.say("\"Then it has remembered how to be a room,\" the caretaker says.")
        elseif game.has_flag("glass_room_found") then
          game.say("\"You have seen it now,\" the caretaker says. \"Do not let beauty make the choice for you.\"")
        elseif game.has_flag("rain_note_read") then
          game.say("\"The glass room is not locked against you,\" the caretaker says. \"It is locked against change.\"")
        else
          game.say("\"The ledger names the room by refusing to name it,\" the caretaker says.")
        end
      end,

      tell = function(game)
        game.say("The caretaker closes his eyes when you say the glass room aloud.")
      end
    },

    rain = {
      aliases = { "note", "folded note", "rain note", "conservatory" },
      requires = "rain_note_read",

      ask = function(game)
        game.say("\"Rain finds every failure in glass,\" the caretaker says. \"That is why the house fears it.\"")
      end,

      tell = function(game)
        game.say("\"Then follow the pane,\" the caretaker says. \"But do not mistake safe for kind.\"")
      end
    },

    bedroom = {
      aliases = { "bedroom", "locked bedroom", "bedroom door", "upstairs" },
      requires = "upstairs_route_found",

      ask = function(game)
        if game.has_flag("bedroom_door_open") then
          game.say("\"The room is open now,\" the caretaker says. \"Be gentle with what it shows you.\"")
        else
          game.say("\"The bedroom kept the house's first promise,\" the caretaker says. \"That is why it locked itself.\"")
        end
      end,

      tell = function(game)
        game.say("\"Yes,\" the caretaker says. \"Upstairs is where safe became a habit.\"")
      end
    },

    mirror = {
      aliases = { "bedroom mirror", "covered mirror", "reflection" },
      requires = "bedroom_mirror_woken",

      ask = function(game)
        game.say("\"Mirrors are glass that learned to answer back,\" the caretaker says.")
      end,

      tell = function(game)
        game.say("\"Then the room has started telling the truth,\" the caretaker says.")
      end
    }
  },

  on_show = function(game, item_id)
    if item_id == "tarnished_key" and game.has_flag("key_polished") then
      game.say("The caretaker turns the polished key toward the light and nods once.")
    elseif item_id == "tarnished_key" then
      game.say("\"Not yet,\" the caretaker says. \"It is still wearing the dark.\"")
    elseif item_id == "rain_note" then
      game.say("\"I wondered where the house had put that,\" the caretaker says.")
    elseif item_id == "bedroom_mirror" then
      game.say("\"Do not carry its answer back to the house too quickly,\" the caretaker says.")
    end
  end,

  on_give = function(game, item_id)
    if item_id == "tarnished_key" then
      game.say("\"Keep it,\" the caretaker says. \"It has chosen your pocket.\"")
    elseif item_id == "rain_note" then
      game.say("The caretaker does not take the note. \"You found it. Let it keep speaking to you.\"")
    end
  end
}
