game {
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
