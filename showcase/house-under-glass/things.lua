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

thing "study_ledger" {
  name = "black ledger",
  aliases = { "ledger", "black ledger", "book" },
  location = "study",
  portable = false,
  desc = "The ledger is bound in black cloth. A pale ribbon marks a page near the end."
}
