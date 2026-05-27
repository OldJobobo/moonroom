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

verb "read" {
  on_action = function(game, input)
    if input ~= "ledger" and input ~= "the ledger" and input ~= "black ledger" and input ~= "book" then
      game.say("Read what?")
      return
    end

    if game.room() ~= "study" then
      game.say("There is no ledger here to read.")
      return
    end

    game.flag("ledger_read")
    game.say("The ledger lists owners in a narrow hand. The final entry has no name, only a water stain shaped like a key.")
  end
}
