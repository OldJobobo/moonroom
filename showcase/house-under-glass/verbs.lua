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
