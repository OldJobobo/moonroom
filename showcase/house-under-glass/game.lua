game {
  id = "house-under-glass",
  version = "0.2.0",
  title = "The House Under Glass",
  author = "Example Author",
  start = "foyer",

  settings = {
    exits = {
      show = true
    }
  },

  before_action = function(game, input)
    if (input == "east" or input == "go east") and game.room() == "foyer" and game.has("tarnished_key") and not game.has_flag("key_polished") then
      game.say("The tarnished key enters the study lock, but the wards in the brass rasp and refuse it.")
    end

    if (input == "unlock box" or input == "unlock wooden box" or input == "unlock box with key" or input == "unlock wooden box with key") and game.has("tarnished_key") and not game.has_flag("key_polished") then
      game.say("The tarnished key worries at the little lock, but the cuts are too dark with old brass.")
    end

    if (input == "north" or input == "go north") and game.room() == "upstairs_landing" and not game.has_flag("bedroom_door_open") then
      game.say("The bedroom door stays shut, its glass knob cold under your hand.")
    end
  end,

  after_action = function(game, input)
    if input == "read ledger" and game.has_flag("ledger_read") then
      game.say("Somewhere in the hall, the caretaker turns a page you cannot see.")
    end
  end
}

include "rooms.lua"
include "things.lua"
include "dialogue.lua"
include "verbs.lua"
include "events.lua"
