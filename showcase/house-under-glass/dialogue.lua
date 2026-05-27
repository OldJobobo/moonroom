thing "caretaker" {
  name = "caretaker",
  aliases = { "caretaker", "old caretaker" },
  location = "hall",
  portable = false,
  actor = true,
  desc = "The caretaker stands with one gloved hand resting against the glass.",

  on_talk = function(game)
    if game.has_flag("ledger_read") then
      game.say("\"Then you have seen how the house keeps its names,\" the caretaker says.")
    elseif game.has_flag("key_polished") then
      game.say("\"Bright enough to remember the lock now,\" the caretaker says, nodding toward the study.")
    else
      game.say("The caretaker looks from you to the tarnished key and waits.")
    end
  end,

  topics = {
    key = function(game)
      if game.has_flag("key_polished") then
        game.say("\"It was cut for the study before the study had a door,\" the caretaker says.")
      else
        game.say("\"Brass forgets under tarnish,\" the caretaker says. \"Polish it, then ask again.\"")
      end
    end,

    ledger = function(game)
      if game.has_flag("ledger_read") then
        game.say("\"Every owner signed it,\" the caretaker says. \"The house kept the rest of the handwriting.\"")
      else
        game.say("\"The ledger is not hidden,\" the caretaker says. \"Only waiting.\"")
      end
    end,

    house = function(game)
      game.say("\"The house remembers best through glass,\" the caretaker says.")
    end
  }
}
