verb "polish" {
  aliases = { "rub" },

  on_action = function(game, input)
    if input == "key" or input == "the key" or input == "tarnished key" or input == "brass key" then
      if not game.has("tarnished_key") then
        game.say("You need to be holding the key first.")
      elseif not game.has("linen_coat") then
        game.say("You need a clean scrap of cloth before the tarnish will lift.")
      else
        game.flag("key_polished")
        game.say("You polish the key with the cuff of the linen coat until brass shows through the dark.")
      end

      return
    end

    game.say("Polish what?")
  end
}

verb "write" {
  aliases = { "sign" },

  on_action = function(game, input)
    if game.room() ~= "glass_room" then
      game.say("There is nothing here that wants your name.")
      return
    end

    if input ~= "name" and input ~= "own name" and input ~= "my name" and input ~= "name in ledger" and input ~= "own name in ledger" and input ~= "my name in ledger" then
      game.say("Write what?")
      return
    end

    if not game.has_flag("glass_ledger_read") then
      game.say("The blank page waits until you understand what it has been keeping.")
      return
    end

    game.flag("ending_written_name")
    game.clear_flag("ending_closed_ledger")
    game.clear_flag("ending_left_open")
    game.clear_flag("house_released")
    game.chapter("after_rain")
    game.start_scene("house_remembers_you")
    game.say("You write your name on the blank page. The ink does not trap you; it opens like a window left unlatched.")
  end
}

verb "leave" {
  on_action = function(game, input)
    if game.room() ~= "glass_room" then
      game.say("Leave what?")
      return
    end

    if input ~= "ledger" and input ~= "open ledger" and input ~= "memory ledger" and input ~= "ledger open" and input ~= "open" then
      game.say("Leave what?")
      return
    end

    if not game.has_flag("glass_ledger_read") then
      game.say("The ledger is already open, but you do not yet know what it is asking.")
      return
    end

    game.flag("ending_left_open")
    game.clear_flag("ending_closed_ledger")
    game.clear_flag("ending_written_name")
    game.clear_flag("house_released")
    game.chapter("after_rain")
    game.start_scene("house_forgetting")
    game.say("You leave the ledger open. The room begins to forget by letting the rain touch each perfect thing.")
  end
}
