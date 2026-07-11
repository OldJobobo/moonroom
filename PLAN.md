# Moonroom Plan

Moonroom is a Rust engine for parser-based interactive fiction with Lua-authored game worlds.

Rust owns parsing, saveable state, action rules, testing, packaging, and frontends. Lua owns author definitions and scripted callbacks through a controlled `game` API. Lua runtime state is never the save format.

This document is the active release plan and architecture contract. `ROADMAP.md` holds later product directions, `README.md` is the current quickstart, and `docs/lua-dsl.md` is the author-facing DSL reference.

## Status

Moonroom is pre-release but already usable from the command line.

| Area | Status |
| --- | --- |
| Core parser-fiction loop | Shipped |
| Lua DSL and callbacks | Shipped |
| Rust-owned save/load | Shipped, compatibility work remains |
| Transcript testing | Shipped |
| Author inspection and validation | Shipped, diagnostics can grow |
| `.moon` packages and standalone builds | Shipped, release hardening remains |
| Advanced object state | In progress |
| Parser disambiguation | Planned |
| Frontend-neutral session protocol | Planned |
| TUI and browser frontends | Later |

Status terms in this plan mean:

- **Shipped**: implemented, documented, and covered by tests.
- **In progress**: a useful subset is shipped, but listed acceptance criteria remain.
- **Planned**: intended next work with an agreed boundary.
- **Later**: directional work that should not constrain near-term implementation yet.

## Product Principles

1. Authors should describe games in readable Lua rather than reimplementing the engine.
2. Saveable gameplay state belongs to Rust: rooms, inventory, object state, flags, counters, timers, scenes, actor memory, RNG state, and turn count.
3. Lua mutation must go through the controlled callback `game` API.
4. Engine behavior should be deterministic under a known random seed.
5. Transcript tests should remain readable story artifacts.
6. The CLI is the canonical frontend until a stable frontend-neutral session protocol exists.
7. Small games may use one `game.lua`; larger games may split definitions with project-local `include` files.
8. Every new DSL feature must define validation, save behavior, undo behavior, tests, and documentation as part of its definition of done.

## Current Stack and Workspace

The settled stack is:

```text
Rust 2024
mlua with vendored Lua 5.4
serde and serde_json
clap
rustyline
anyhow and thiserror
```

Current crates:

```text
crates/mr-core   serializable world/state, parser actions, rules, and core events
crates/mr-lua    Lua DSL loading, callback registry, game API, saves, and packages
crates/mr-cli    play/test/check/inspect/transcript/pack/unpack/build/new commands
crates/mr-test   transcript parser, assertions, runner, filtering, and update mode
```

Possible future crates should be created only when their boundary is proven:

```text
crates/mr-tui    richer terminal frontend
crates/mr-save   save compatibility and migration logic, if mr-lua becomes crowded
```

## Current Capabilities

Moonroom currently supports:

- Rooms, exits, things, inventory, containers, supporters, wearables, hidden things, guarded exits, openable and lockable objects.
- Movement, examination, manipulation, use-with actions, reading, wearing, dialogue actions, `again`, and bounded `undo`.
- Actors, topic aliases and requirements, ask/tell/show/give callbacks, and Rust-owned actor memory.
- Flags, counters, deterministic random numbers, visited rooms, timers, scenes, and chapters.
- Global, room, thing, verb, topic, scene, chapter, and timer callbacks.
- Project-local Lua includes with duplicate suppression, cycle rejection, and root containment.
- Versioned JSON saves with game identity checks plus legacy raw-state loading.
- Transcript assertions, filtering, deterministic seed overrides, and golden updates.
- Static checks, project inspection, transcript recording, `.moon` packages, unpacking, and standalone executable output.

The canonical commands are:

```bash
moonroom play GAME
moonroom test GAME [--filter TEXT] [--seed INTEGER] [--update]
moonroom check GAME
moonroom inspect GAME
moonroom transcript GAME [-o FILE]
moonroom pack DIRECTORY -o FILE.moon
moonroom unpack FILE.moon -o DIRECTORY
moonroom build DIRECTORY --standalone -o EXECUTABLE
moonroom new DIRECTORY
```

## Architecture Contracts

### Ownership

`mr-core` must not depend on Lua. It owns the serializable model, parser actions, rules, turn state, and events that scripted behavior can observe.

`mr-lua` owns Lua-specific definitions and callback execution. It may translate Lua tables into core definitions and apply queued `game` API commands, but it must not make Lua tables or closures part of `GameState`.

Frontend crates must call an engine/session API. They must not duplicate parser rules or mutate state directly.

### Game projects and includes

A typical source project is:

```text
my-game/
  game.lua
  rooms.lua
  things.lua
  dialogue.lua
  verbs.lua
  events.lua
  assets/
  tests/
```

`game.lua` is always the entrypoint. Includes resolve relative to the including file, remain inside the source or package root, load at most once, and reject cycles. Small games may define everything in `game.lua`.

### Parser

The current parser normalizes text and dispatches built-in verbs directly; it does not yet expose a general typed `Intent` object. Do not document a typed intent layer as existing.

A typed intent model is optional future work. It becomes worthwhile when disambiguation, structured frontend output, or author hooks need resolved direct and indirect objects. If introduced, it belongs in `mr-core` and must replace rather than duplicate the existing dispatch path.

### Turn and callback pipeline

The implemented contract is:

```text
1. Normalize input and handle non-gameplay meta commands.
2. Classify whether the command can advance a turn.
3. Snapshot all Rust-owned state for rollback and undo.
4. Run before_action with normalized raw input.
5. If not intercepted, parse, resolve, and apply the core action.
6. Run action-specific Lua callbacks and apply queued game API mutations.
7. Run after_action.
8. Advance the turn once and fire due timers.
9. Commit the undo entry and last-command state.
10. Return rendered output and structured events.
```

Required semantics:

- A normal action advances at most one turn.
- An intercepted normal action still consumes the turn and runs `after_action`.
- Meta commands such as `look`, inventory, save/load, `again`, and `undo` follow explicit per-command rules.
- Undo restores every Rust-owned mutation made by core or Lua callbacks.
- Any callback error rolls the command back to its pre-command snapshot.
- Timers observe the fully committed action state and fire after action callbacks.
- `again` repeats the last successfully committed advancing command.

Scheduling during an action counts only subsequent advancing commands, not the action currently in progress. Action and global callbacks see the pre-advance turn; timer callbacks see the newly advanced turn.

### Save contract

Save files contain a versioned envelope, game identity metadata, and serialized `GameState`. They do not contain Lua runtime state or undo history.

Current behavior:

- Save format version is `1`.
- The compatibility id is `game.id`, falling back to the title.
- Saves for another game id are rejected.
- Pretty and compact JSON are supported.
- Legacy raw `GameState` JSON is accepted.
- Game version metadata is recorded but does not currently drive compatibility.

Before declaring save compatibility stable, Moonroom must define:

- Engine save-version migration rules.
- Game-version compatibility: warn, reject, or run an author migration.
- Atomic writes using a temporary file and rename.
- Recovery and diagnostics for truncated or corrupt saves.
- Size/depth limits for untrusted save input.
- Representative compatibility fixtures for every supported old format.

Once published as stable, a save version must not change without a migration or an explicit compatibility break.

### Package and execution trust

The v1 `.moon` format is a JSON envelope containing metadata and a virtual file table. File bytes are hex encoded. It is not a ZIP or other archive format despite presenting an archive-like virtual filesystem.

The three supported distribution forms are:

```text
source directory   editable author format
.moon package      portable single-file virtual project
standalone binary  current Moonroom executable with an embedded .moon payload
```

Package paths are relative and may not escape the virtual root. Source packaging rejects symbolic links. Includes use the same relative-root rules in folders and packages.

Moonroom games are executable content, not passive documents. Until a hardened sandbox and resource limits exist, users must treat Lua projects and `.moon` files like programs and run only content they trust.

Release hardening must define:

- Available Lua standard libraries and host capabilities.
- Instruction and memory limits, including infinite-loop handling.
- Package byte, decoded-file, file-count, and path-length limits.
- Duplicate virtual-path behavior and malformed-package tests.
- Asset inclusion and runtime access semantics.
- Standalone target-platform behavior; standalone builds currently copy the host executable and are not cross-compilation packaging.

A future binary or compressed package format would be a new format version. Minification or Lua bytecode may be convenience features but must never be presented as DRM.

### Frontend boundary

Before `mr-tui`, browser, or JSON clients, define a frontend-neutral session boundary such as:

```rust
EngineSession::command(input) -> TurnResult
```

`TurnResult` should eventually distinguish prose output from structured state and presentation events. The protocol must cover:

- Ordered output blocks.
- Current room and optional status snapshot.
- Inventory changes and other observable state changes.
- Save/load requests without terminal I/O inside the engine.
- Errors and quit state.
- Protocol versioning for JSON consumers.

New frontends must consume this boundary rather than parse CLI text or fork engine behavior.

## Quality Contract

Every feature is complete only when it includes, as applicable:

- Core unit tests for state and action rules.
- Lua integration tests for DSL loading and callbacks.
- Transcript coverage for author-visible behavior.
- Save/load and undo coverage for new Rust-owned state.
- Static validation for invalid author definitions.
- Updates to `README.md`, `docs/lua-dsl.md`, `.luarc.json`, and templates.
- `cargo fmt --all --check`, workspace tests, strict Clippy, and example checks.

Robustness work should add:

- Parser fuzz or property tests.
- Malformed save and package corpora.
- Path traversal, symbolic-link, duplicate-path, and input-limit tests.
- Callback-failure rollback tests.
- Standalone executable smoke tests.
- Cross-platform CI for supported targets.

## Active Release Plan

Moonroom's next target is **0.1: a dependable authoring release**. The release is not a promise to finish every possible parser-fiction feature. It proves one complete workflow:

```text
create a game -> author it -> check it -> test it -> package it -> play it
```

The House Under Glass is the release-driving fixture. Work enters 0.1 only when it improves that workflow, closes a correctness or trust gap, or is needed to ship the showcase. Other ideas belong in `ROADMAP.md`.

### Phase 1: Author feedback (Shipped)

Goal: authors can correct common project mistakes without reading Rust or Lua stack traces.

- Add structured diagnostics with severity, source path, and an actionable message.
- Report DSL source context where Lua loading provides enough location information.
- Extend static checks to callback and event references that can be resolved without executing gameplay.
- Add invalid-project fixtures covering each diagnostic class.

Exit criteria:

- `moonroom check` distinguishes errors from warnings.
- Every supported diagnostic has a stable test and a corrective message.
- The generated starter project passes with no warnings.

Delivered in the 0.1 line: `moonroom check` emits severity-labelled, stable-code diagnostics with corrective guidance; literal scheduled-event references are checked without running gameplay callbacks and report Lua file/line context. Invalid-world and missing-event fixtures cover the diagnostic paths, and the generated starter remains warning-free.

### Phase 2: Parser correctness (Shipped)

Goal: valid commands never resolve an ambiguous object arbitrarily.

- Detect multiple reachable object matches.
- Add a bounded disambiguation interaction or require a more specific command; choose the smaller design that preserves deterministic transcripts.
- Improve failures for inaccessible, hidden, closed-container, and wrong-context objects.
- Add normalization and resolution property tests.
- Document singular pronoun behavior and explicitly defer plural pronouns unless the showcase requires them.

Exit criteria:

- Ambiguous input has deterministic, tested behavior in core, Lua-backed play, and transcripts.
- Parser failures explain the relevant corrective action without revealing hidden objects.

Delivered in the 0.1 line: reachable object matching is deterministic. When a command matches more than one visible, reachable object, Moonroom lists the candidates and requires a more specific name; it does not pick by definition order. Inaccessible objects report closed-container or reachability context without exposing hidden objects. Core, Lua-backed, and transcript tests cover the behavior.

### Phase 3: Save and package safety (Shipped)

Goal: ordinary corruption or oversized input fails safely, and 0.1 defines what it will preserve.

- Write saves atomically through a temporary file and rename.
- Define the game-version compatibility policy and engine save-version migration policy.
- Add fixtures for the legacy raw state and every supported versioned envelope.
- Add save and package byte, file-count, decoded-size, nesting, and path-length limits.
- Test truncated saves, malformed packages, duplicate virtual paths, and traversal attempts.
- Document that Lua content remains trusted executable code; resource limits are not a sandbox.

Exit criteria:

- Failed writes do not destroy the last valid save.
- Unsupported or corrupt inputs produce bounded, actionable errors.
- Compatibility promises are documented before the first tagged release.

Delivered in the 0.1 line: saves write atomically through a synced sibling temporary file and rename. Raw legacy state remains the supported format-0 migration, versioned save envelope 1 is the current compatible format, and compatibility is keyed by game id while game version remains author metadata. Save and package readers reject bounded corrupt input; package limits cover input size, file count, decoded totals, individual file size, path length/depth, duplicate paths, malformed hex, and traversal. Lua remains trusted executable content; these limits are robustness measures, not a sandbox.

### Phase 4: Author documentation (Next)

Goal: a new author can build and test a small game without reconstructing behavior from examples.

- Write a first-game tutorial.
- Add a parser command reference.
- Add a transcript testing guide.
- Add a focused cookbook for locks, containers, hidden objects, NPC topics, timers, scenes, deterministic randomness, and save-compatible puzzle state.
- Add a packaging and save-compatibility guide.

Exit criteria:

- The tutorial produces a project accepted by `check` and `test`.
- Every starter DSL feature links to its authoritative documentation.

### Phase 5: 0.1 release

Goal: ship one supported release path and a polished proof game.

- Finish and test The House Under Glass as the release showcase.
- Define the initial supported host platforms.
- Add CI for formatting, strict Clippy, workspace tests, example checks, package round-trips, and standalone smoke tests on those platforms.
- Define the install workflow and release profile.
- Produce versioned artifacts and checksums.
- Run the complete author workflow from a clean checkout using only published documentation.

Exit criteria:

- The showcase passes static checks and all transcripts from both its source tree and packaged form.
- A fresh user can install Moonroom, create the starter, test it, package it, and run it on each supported platform.
- Release artifacts are reproducible enough to identify their source revision and verify integrity.

## Scope Rules

- Complete phases in order unless a later-phase task is required to unblock an earlier one.
- Prefer correctness, diagnostics, and fixtures over expanding the DSL.
- Add scenery only if it materially improves the showcase before 0.1.
- Defer darkness, light sources, edible/drinkable things, declarative dialogue, and plural pronouns until a shipped story demonstrates the need.
- Do not start hot reload, `game.choice()`, a TUI, browser support, or WASM before the frontend-neutral session contract exists.
- Do not claim untrusted-game sandboxing until Lua capabilities and enforceable runtime limits have been designed and tested as a security boundary.
