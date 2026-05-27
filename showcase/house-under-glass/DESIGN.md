# The House Under Glass Design Guide

This document translates a large tarot-inspired creative spread into practical story and design guidance for the showcase game. Treat it as an associative design lens, not fixed lore. The goal is to keep future rooms, puzzles, prose, and engine feature choices pointed in the same direction.

## North Star

The House Under Glass is a game about a place that loved its people so much it forgot how to let them leave.

The house is not evil. It is stuck preserving a false memory of completion, safety, and home. The player should not defeat the house. The player should help it admit what it has been refusing to remember.

## Core Theme

```text
preservation becoming imprisonment
```

The house wants to keep everything safe, named, cataloged, and unchanged. Its tragedy is not a monster or a murder at first. It is maintenance taken too far: caretaking as refusal, memory as ownership, glass as protection that became a cage.

Every major puzzle should ask:

```text
What is being kept?
Who is keeping it?
What would happen if it were allowed to change?
```

## Tarot Spread

```text
1. The House Itself: Three of Wands reversed
2. The Protagonist: Nine of Pentacles reversed
3. The Caretaker: Ace of Cups upright
4. The Key: The Magician reversed
5. The Ledger: Ten of Cups upright
6. The Glass Room: Six of Cups upright
7. Central Puzzle Pressure: Four of Pentacles upright
8. Emotional Wound: Knight of Pentacles upright
9. Hidden Truth: The Hermit upright
10. False Solution: Two of Wands upright
11. Midgame Expansion: Two of Cups reversed
12. Ending Shape: Two of Swords upright
13. Design Constraint: Five of Wands reversed
14. Player Experience: Four of Wands upright
15. Engine Feature To Prioritize: The Devil upright
```

## Story Guidelines

Make the house emotionally defensive, not aggressive.

- Locked doors should feel like withheld trust.
- Glass should show memory, not just reflection.
- Dust, ledgers, keys, coats, and rain should imply care, repetition, and preservation.
- The house should respond to attention: examining, reading, asking, and revisiting.

The player should slowly learn humility. They cannot solve the house by forcing doors. They have to understand what each object is preserving.

## The Caretaker

The caretaker should be warmer than he first appears.

- He is not the villain.
- He is the emotional channel of the house.
- His early silence is restraint, not hostility.
- His dialogue should become more humane as the player reads, repairs, or remembers correctly.

The caretaker should rarely volunteer the full truth. He should confirm, redirect, and mourn.

## The Key

The key should be suspicious.

- It is not just a key.
- It may mislead when tarnished.
- It may open the wrong thing if used too early.
- Polishing it should not only unlock a door. It should reveal that tools can lie when neglected.

The key is a test of care. It responds to being cleaned, carried, used in the right place, and understood.

## The Ledger

The ledger should be emotionally loaded, not bureaucratic.

- It records belonging, family, completion, and domestic dreams.
- Reading it should feel intimate.
- It should contain evidence of people trying to make the house a home.
- The horror is that the house preserved the dream after the people were gone.

The ledger should unlock caretaker topics and change room descriptions. It should make the player understand that the house remembers people as entries, objects, and arrangements.

## The Glass Room

The Glass Room should be healing, familiar, and dangerous because of that familiarity.

- It should be beautiful, not scary.
- It should feel like childhood memory, nostalgia, and old comfort.
- It should tempt the player to leave things as they are.
- The danger is sentimental stasis: keeping memory perfect by preventing change.

The final room should not feel like a boss room. It should feel like a preserved afternoon that cannot end.

## Puzzle Structure

Build puzzles around releasing what has been over-preserved.

```text
Key puzzle
  Clean the key so it stops lying.

Ledger puzzle
  Read the names so the house stops reducing people to objects.

Caretaker puzzle
  Ask the right topics after gaining context.

Conservatory or bedroom puzzle
  Reveal a hidden route by disturbing a preserved arrangement.

Glass Room puzzle
  Choose between keeping the house sealed and letting memory change.
```

Puzzle solutions should feel like attention and consent, not conquest.

## False Solution

Avoid making the solution simply:

```text
find exit
escape house
win
```

That is too obvious and too external. The better ending is not escape. It is permission.

The player should learn that leaving is not the opposite of remembering.

## Ending Shape

The ending should preserve ambiguity. Possible ending choices:

```text
Close the ledger
  The house keeps its memories, but no longer traps new ones.

Leave the ledger open
  The house begins forgetting, and the glass room becomes ordinary.

Write your own name
  The player accepts becoming part of the house's memory, but on their own terms.
```

Do not make one ending purely good and the others bad. Make them tonal choices.

## Design Constraints

Avoid noisy conflict.

- No combat.
- No loud antagonist reveal.
- No domination-based puzzle solutions.
- No chase sequence unless it is restrained and thematically necessary.
- Keep tension quiet: locked doors, changed descriptions, repeated phrases, and rooms remembering too much.

Use random output only for atmosphere. Never hide puzzle-critical information behind randomness.

## Player Experience Target

The player should finish with the feeling of having restored a home, not conquered a dungeon.

Aim for:

```text
quiet satisfaction
recognition
a little sadness
the sense that the house can breathe again
```

The final scene should feel like opening windows after rain.

## Mechanical Priorities

The showcase wants mechanics that let the house bind things, conceal things, preserve things, and tempt the player with one more locked room.

Prioritize:

```text
read
open / close
lock / unlock
hidden / revealed
object state
scenery
```

`read` matters for the ledger. `open`, `close`, `lock`, and `unlock` matter for making preservation physical. Hidden/revealed and scenery objects matter for making the house feel detailed without cluttering every room listing.

## Concrete Next Additions

1. Add `read` as a core verb instead of only a showcase custom verb.
2. Add openable and lockable object state.
3. Add the Conservatory as the next room.
4. Make the ledger unlock additional caretaker topics.
5. Add a recurring phrase in room descriptions: `kept safe`.
6. Let the player discover that `safe` and `trapped` are the same word to the house.
7. Design the Glass Room as a beautiful memory, not a boss room.
