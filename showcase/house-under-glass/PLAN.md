# The House Under Glass Showcase Plan

This showcase should become the polished Moonroom demo: a small, complete parser-fiction game with a consistent mood, a clear puzzle arc, and enough authored detail to show what the engine can do.

Keep `examples/house` as the compact integration testbed. It should continue to exercise engine features in miniature. This showcase can be slower, more atmospheric, and more story-shaped.

Use [DESIGN.md](DESIGN.md) as the creative guide for story tone, symbolic motifs, ending shape, and puzzle design constraints.

## Intent

```text
examples/house
  Small regression fixture.
  Dense coverage of engine features.
  Easy to update when parser behavior changes.

showcase/house-under-glass
  Polished playable demo.
  Richer prose and pacing.
  Multiple transcripts for meaningful paths.
  Built to reveal what authors can make with Moonroom.
```

## Current Scaffold

```text
showcase/house-under-glass/
  PLAN.md
  README.md
  game.lua
  rooms.lua
  things.lua
  dialogue.lua
  verbs.lua
  events.lua
  tests/
    opening.transcript
    golden-path.transcript
    caretaker-topics.transcript
```

Moonroom supports project-local `include`, so `game.lua` should act as the entrypoint and load the neighboring split files.

## Story Premise

The player enters a rain-lashed house whose glass rooms preserve memories like pressed flowers. A tarnished key, a silent caretaker, and a ledger of vanished owners point toward a sealed glass room at the heart of the house.

The house should feel watchful but not hostile. It remembers. It repeats. It tests whether the player notices what changes after they act.

## Target Scope

Aim for a 20-30 minute parser game.

Rooms:

```text
Foyer
Hall
Study
Conservatory
Kitchen
Upstairs Landing
Locked Bedroom
Glass Room
```

Core objects:

```text
tarnished key
linen coat
wooden box
hall table
study ledger
garden shears
greenhouse pane
bedroom mirror
glass room door
```

Primary actor:

```text
caretaker
```

Optional later actors:

```text
reflection
voice behind the glass
```

## Golden Path

```text
look
take key
wear coat
polish key
east / unlock study
read ledger
ask caretaker about ledger
ask caretaker about glass room
find conservatory clue
open/reveal route upstairs
unlock bedroom
use mirror or key
enter glass room
resolve the house memory
```

The initial version can use existing engine mechanics:

```text
guarded exits
custom verbs
on_take / on_use callbacks
actors and topics
timed events
game.visited
game.random for atmosphere
exit display settings
transcript tests
```

## Engine Features The Showcase Wants

These should be implemented before the showcase becomes too large:

```text
open / close
lock / unlock
read
again / g
undo
hidden / revealed things
scenery objects
moonroom check
```

Do not fake all of these in Lua if they belong in core state. The showcase should drive engine design, not work around it forever.

## Puzzle Design

### Puzzle 1: The Tarnished Key

Purpose:

```text
teach take, examine, use, polish/custom verb, and state-dependent descriptions
```

Shape:

```text
key starts in foyer
key is cold and tarnished
polishing it changes responses and unlocks study-related information
caretaker gives different topic responses before/after polish
```

### Puzzle 2: The Study Ledger

Purpose:

```text
introduce read once the engine supports it
connect house memory to concrete clues
unlock caretaker topics
```

Shape:

```text
ledger names former owners
some names appear in room descriptions after reading
caretaker confirms one name if asked
```

### Puzzle 3: The Conservatory Route

Purpose:

```text
use open/close, hidden/revealed objects, and room revisits
```

Shape:

```text
player finds a cracked pane or latch
opening/revealing it changes available exits or object visibility
rain/weather timer adds pressure but not failure
```

### Puzzle 4: The Glass Room

Purpose:

```text
final payoff using dialogue, visited-state, and a small inventory/state check
```

Shape:

```text
requires key, ledger knowledge, and a response from the caretaker
entry changes the house's descriptions
ending should be quiet, not explosive
```

## Writing Rules

- Keep prose concrete and inspectable.
- Prefer short room descriptions that change after important actions.
- Avoid random output for puzzle-critical clues.
- Use `game.random` only for atmosphere.
- Do not make every object portable.
- Avoid making transcript output brittle with too many decorative timed lines.
- Every puzzle-critical clue should be reachable through at least two signals: object description, dialogue, room text, or transcript-visible command result.

## Transcript Plan

```text
opening.transcript
  First room, first object interactions, basic movement.

golden-path.transcript
  Complete intended solution path.

caretaker-topics.transcript
  Dialogue before and after key/ledger state changes.

study-ledger.transcript
  Reading and study-specific puzzle checks once read exists.
```

Keep transcripts shorter than a full transcript novel. They are regression tools, not walkthrough prose.

## README Goals

The showcase README should eventually include:

```text
one-paragraph premise
how to run
how to test
estimated play time
implemented engine features demonstrated
known missing engine features if the showcase is still partial
```

## First Build Slice

Build a playable 3-room slice before expanding:

```text
Foyer
Hall
Study
```

Minimum actions:

```text
look
take key
polish key
east
talk to caretaker
ask caretaker about key
read ledger, once read exists
```

Acceptance:

```text
cargo run -q -p mr-cli -- play showcase/house-under-glass
cargo run -q -p mr-cli -- test showcase/house-under-glass
```

The first slice should pass transcript tests before adding more rooms.

Status: implemented.

Active transcripts:

```text
opening.transcript
golden-path.transcript
caretaker-topics.transcript
```
