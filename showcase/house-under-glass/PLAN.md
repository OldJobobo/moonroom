# The House Under Glass Showcase Plan

This showcase should become the polished Moonroom demo: a small, complete parser-fiction game with a consistent mood, a clear puzzle arc, and enough authored detail to show what the engine can do through the current roadmap.

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

Delivery shape:

```text
playable source folder for authors
.moon package for players and reviewers
standalone build for distribution smoke tests
transcripts that verify the main solution, dialogue changes, state changes, and package behavior
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

The full showcase should use existing engine mechanics:

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
read text and on_read callbacks
open / close / lock / unlock state
containers and supporters
hidden / revealed things
wearables and worn inventory state
show / give / tell dialogue actions
topic aliases and gated topics
actor memory
scenes, chapters, and scene-scoped timers
before_action / after_action hooks
again / g
undo
save/load compatibility metadata
moonroom check / inspect / transcript
moonroom pack / unpack / build --standalone
```

## Milestone Coverage Goals

The engine now has enough features through Milestone 14 that the showcase should stop waiting on the roadmap and start proving it. `examples/house` can keep dense regression coverage; this game should demonstrate the same capabilities in a player-facing form.

### Milestone 6: Project Structure

```text
Keep game.lua as the entrypoint.
Keep rooms.lua, things.lua, dialogue.lua, verbs.lua, and events.lua split by author concern.
Add enough content to make project-local include feel necessary rather than ornamental.
Keep checkable source paths clear when a validation error points into a split file.
```

### Milestone 7: Parser Quality

```text
Use read for the ledger and at least one optional clue.
Use open / close for the conservatory pane or bedroom cabinet.
Use lock / unlock for the study door, bedroom door, or glass room door.
Make the golden path include again/g and undo in a short transcript branch.
Keep parser failures authored enough that a stuck player gets useful direction from visible state.
```

### Milestone 8: Object State

```text
Use the wooden box as an openable container with a hidden or protected clue.
Use the hall table as a supporter for visible staging.
Use the linen coat as a wearable object with at least one response difference.
Use hidden/revealed state for the conservatory clue and one late-game reveal.
Use lockable object state for a door or cabinet whose state survives save/load.
```

Scenery is still a high-value next engine feature. Until it exists, avoid overloading the room listings with nonportable detail objects.

### Milestone 9: Dialogue System

```text
Give the caretaker topic aliases for key, ledger, house, glass room, owner names, and door.
Gate at least two topics behind reading the ledger or polishing the key.
Use tell caretaker about ledger to test a different emotional beat than ask.
Use show key to caretaker before and after polishing.
Use give key to caretaker as a refusal or trust-building moment.
Store a small caretaker memory counter so repeated questions change tone without becoming noisy.
```

Optional later actors, such as the reflection or the voice behind the glass, should only be added after the caretaker has enough depth to justify the dialogue system.

### Milestone 10: Scenes and Chapters

```text
Start in chapter "arrival".
Move to chapter "names" after the ledger is read.
Start scene "rain_memory" after the key is taken or the ledger is read.
Use a scene-scoped timer for a room description change or caretaker aside.
End with scene "glass_room" and assert it in the final transcript.
```

Scenes should structure the story; they should not become required ceremony for every small interaction.

### Milestone 11: Author Tooling

```text
Keep moonroom check passing as part of showcase acceptance.
Use moonroom inspect output as a manual review aid when adding rooms, exits, topics, and callbacks.
Use moonroom transcript to record new branches, then edit them down into focused regression transcripts.
Treat check/inspect failures as design feedback, not just engine feedback.
```

### Milestone 12: Save Format Hardening

```text
Add stable game id and version metadata to game.lua.
Keep at least one manual save/load smoke path in the release checklist.
Exercise compact save output once the golden path has enough state to matter.
Ensure saves reject mismatched games when the showcase package is loaded under a different source.
```

The game should avoid storing puzzle state in Lua-only globals. If the state affects the ending, transcripts, saves, or undo, it belongs in Rust-owned state through the controlled game API.

### Milestone 13: Testing Improvements

```text
Use !contains / !not_contains for atmospheric branches where exact prose is too brittle.
Use !room, !flag, !counter, !scene, and !chapter assertions for puzzle-critical state.
Use --filter while developing individual transcripts.
Use --update only after reviewing the changed output as prose.
Use --seed for deterministic atmosphere checks that rely on game.random.
```

### Milestone 14: Packaging and Distribution

```text
Add package smoke tests to the release checklist:
  moonroom pack showcase/house-under-glass -o dist/house-under-glass.moon
  moonroom check dist/house-under-glass.moon
  moonroom test dist/house-under-glass.moon
  moonroom inspect dist/house-under-glass.moon
  moonroom unpack dist/house-under-glass.moon -o dist/unpacked-house-under-glass
  moonroom build showcase/house-under-glass --standalone -o dist/house-under-glass
```

The source folder remains the authoring form. The `.moon` package is the normal player/reviewer form. The standalone binary is the "send this to someone" form once the story is complete enough to represent the engine.

## Future Milestone Hooks

### Milestone 15: Frontends

Keep the CLI transcript as canonical, but design output so it can survive richer frontends later.

```text
Room names should be concise enough for a status pane.
Inventory and worn state should be meaningful enough for a side pane.
Events should be structured through engine output rather than frontend-specific prose hacks.
Avoid relying on terminal-only formatting for essential clues.
```

When an event-stream or TUI/browser frontend exists, this showcase should become the first acceptance game for it.

### Milestone 16: Documentation as Product

Treat the finished showcase as documentation material.

```text
Keep code examples clean enough to quote in tutorials.
Keep comments sparse but useful around non-obvious Lua callback patterns.
Add README sections that map implemented game moments to DSL features.
Use the showcase as the source for cookbook examples: locked door, NPC topic, hidden item, timer, scene transition, save-compatible state, and package release.
```

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
teach read as a first-class clue action
connect house memory to concrete clues
unlock caretaker topics
advance the story chapter from arrival to names
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
exercise saved object state and scene-scoped timers
```

Shape:

```text
player finds a cracked pane or latch
opening/revealing it changes available exits or object visibility
rain/weather timer adds pressure but not failure
undo can roll back a discovery branch without corrupting callback-driven state
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
scene/chapter state marks the ending sequence
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
  Reading and study-specific puzzle checks.

object-state.transcript
  Container/supporter, open/close, lock/unlock, hidden/revealed, wear/remove, and undo checks.

scene-ending.transcript
  Scene/chapter progression and the final Glass Room state.

package-smoke.transcript
  Short path that remains stable when run from source and from a .moon package.
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
current completeness status
package and standalone release commands once the game is ready to distribute
```

## Build Slices

### Slice 1: Implemented 3-room foundation

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
read ledger
```

Acceptance:

```text
cargo run -q -p mr-cli -- play showcase/house-under-glass
cargo run -q -p mr-cli -- check showcase/house-under-glass
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

### Slice 2: Milestone 8-10 expansion

Add the next playable arc:

```text
Conservatory
Kitchen or Upstairs Landing
wooden box as a container
hall table as a supporter
lockable route toward the bedroom
hidden/revealed conservatory clue
chapter transition after ledger reading
rain_memory scene with a scene-scoped timer
caretaker topic aliases and gated ask/tell/show/give responses
```

Acceptance:

```text
cargo run -q -p mr-cli -- check showcase/house-under-glass
cargo run -q -p mr-cli -- test showcase/house-under-glass --filter object-state
cargo run -q -p mr-cli -- test showcase/house-under-glass --filter caretaker
cargo run -q -p mr-cli -- inspect showcase/house-under-glass
```

### Slice 3: Complete game and release form

Finish the target room list and prove distribution:

```text
Locked Bedroom
Glass Room
ending choice or ending variation
save/load smoke path
package smoke path
standalone build smoke path
README update with implemented features and play time
```

Acceptance:

```text
cargo run -q -p mr-cli -- test showcase/house-under-glass
cargo run -q -p mr-cli -- pack showcase/house-under-glass -o dist/house-under-glass.moon
cargo run -q -p mr-cli -- check dist/house-under-glass.moon
cargo run -q -p mr-cli -- test dist/house-under-glass.moon
cargo run -q -p mr-cli -- inspect dist/house-under-glass.moon
cargo run -q -p mr-cli -- build showcase/house-under-glass --standalone -o dist/house-under-glass
```
