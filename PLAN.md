# Moonroom Plan

Moonroom is a Rust engine for parser-based interactive fiction with Lua-authored game worlds.

Rust owns parsing, saveable state, action rules, testing, packaging, and frontends. Lua owns author definitions and scripted callbacks through a controlled `game` API. Lua runtime state is never the save format.

This document is the product roadmap and architecture contract. `README.md` is the current quickstart and `docs/lua-dsl.md` is the author-facing DSL reference.

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

The current implementation has grown across both `Game` and `LuaGame`: the global before hook runs before core parsing, core applies an action and advances time, Lua event callbacks run afterward, and the global after hook runs last. Intercepted before hooks and callback failures do not yet have a fully explicit transactional contract.

The target contract is:

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
- An intercepted normal action follows one documented turn policy; the preferred policy is that it still consumes the turn.
- Meta commands such as `look`, inventory, save/load, `again`, and `undo` follow explicit per-command rules.
- Undo restores every Rust-owned mutation made by core or Lua callbacks.
- Any callback error rolls the command back to its pre-command snapshot.
- Timers observe the fully committed action state and fire after action callbacks.
- `again` repeats the last successfully committed advancing command.

This contract must be implemented before new callback phases or interaction modes are added.

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

## Roadmap

### Milestones 1–6: Foundation — Shipped

Delivered:

- Playable Rust CLI and Lua-authored world loading.
- Rooms, things, movement, inventory, core actions, flags, counters, callbacks, and custom verbs.
- Serializable state, deterministic RNG, JSON save/load, and transcript testing.
- Project templates, examples, documentation, and interactive command history.
- Containers, supporters, locks, wearables, actors, timers, scenes, and chapters.
- Multi-file projects with safe local includes.

Hot reload was not delivered and is not implied by foundation status. It remains a separate design task because reloading definitions while preserving Rust-owned state needs compatibility rules for removed or changed rooms, things, callbacks, and events.

### Milestone 7: Parser quality — In progress

Shipped:

- `again` and bounded `undo`.
- Recent singular-object pronoun resolution.
- Common verbs including open, close, lock, unlock, read, give, and show.
- Article-insensitive and normalized object matching.

Remaining:

- Ambiguity detection and disambiguation when multiple reachable things match.
- A deliberate policy for plural references such as `them`.
- More context-sensitive parser failures.
- Property tests for normalization and object resolution.

Done when ambiguous input never silently selects an arbitrary object and the behavior is covered in core, Lua, and transcript tests.

### Milestone 8: Object state — In progress

Shipped:

- Openable containers, lockable things, keys, guarded exits, hidden/revealed things, containers, supporters, and wearables.

Remaining, in recommended order:

1. Scenery things that remain inspectable but do not clutter room listings.
2. Dark rooms and explicit light sources.
3. Edible and drinkable things, only if a showcase puzzle demonstrates their value.

Done when each property is Rust-owned, serializable, undoable, statically validated, documented, and exercised by transcript fixtures.

### Milestone 9: Dialogue — In progress

Shipped:

- Actors, talk, topic aliases, topic requirements, ask/tell/show/give callbacks, and actor memory.

Remaining:

- Decide whether multi-step conversation needs a declarative dialogue model or is adequately represented by actor memory and scenes.
- Do not add `game.choice()` until the engine can suspend an interaction and request structured input through the frontend-neutral session protocol.

Done when a showcase conversation can support a multi-step puzzle without storing required progress in Lua globals.

### Milestone 10: Scenes and chapters — Shipped

Delivered current scene/chapter state, lifecycle hooks, scene-scoped timers, and transcript assertions. Future work here should be driven by a concrete story requirement.

### Milestone 11: Author tooling — In progress

Shipped:

- `moonroom check`, `inspect`, and transcript recording.
- Validation for world graph errors, object locations, guarded exits, and duplicate vocabulary.
- Lua source paths in load failures.

Remaining:

- Better DSL-specific source context and actionable diagnostics.
- Optional warnings distinct from fatal validation errors.
- Validation of callback/event references where statically possible.
- Stable machine-readable diagnostics if editor integration is pursued.

Done when common author mistakes are reported before play with a source location, severity, and corrective message.

### Milestone 12: Save hardening — In progress

Shipped versioned envelopes, identity checks, pretty/compact JSON, and legacy raw-state loading.

Remaining work is the save contract described above: atomic writes, corruption handling, explicit game-version policy, real migrations, input limits, and compatibility fixtures.

### Milestone 13: Testing — Shipped, ongoing

Delivered transcript assertions, filtering, golden update mode, seed overrides, command-context failures, and regression fixtures. Robustness and cross-platform testing remain continuous quality work rather than a closed feature milestone.

### Milestone 14: Packaging and distribution — In progress

Shipped folder/package loading, pack/unpack, package-aware play/check/test/inspect, and host-platform standalone builds.

Remaining:

- The package trust and resource limits described above.
- Installed-release workflow and supported-platform matrix.
- Release profile and standalone smoke checks.
- Checksums and versioned release artifacts.
- Clear project/package versioning documentation.

Done when a user can install Moonroom and run a packaged game on every supported platform without a source checkout.

### Milestone 15: Frontends — Planned

Order:

1. Define and test the frontend-neutral session and structured output boundary.
2. Add versioned JSON input/output mode for external clients.
3. Build `mr-tui` only if it materially improves play or author testing.
4. Consider a browser frontend after the JSON/session contract is stable.
5. Investigate WASM only after confirming that Lua runtime constraints and package loading are acceptable.

The CLI remains canonical throughout this work.

### Milestone 16: Documentation as product — In progress

Feature documentation is continuous and part of the quality contract. Dedicated remaining deliverables are:

- A first-game tutorial.
- A parser command reference.
- A save compatibility and migration guide.
- A transcript testing guide.
- A focused cookbook covering locks, NPC topics, timers, hidden objects, containers, deterministic randomness, scenes, and save-compatible puzzle state.

The DSL reference may be split by topic when navigation becomes a real problem; file splitting is not itself a product goal.

### Milestone 17: Engine contract hardening — Next

This is the immediate implementation milestone because later callbacks, dialogue choices, and frontends depend on it.

Work:

1. Consolidate turn advancement and undo ownership across `Game` and `LuaGame`.
2. Implement the target callback pipeline and transactional rollback.
3. Add tests for intercepted actions, callback failures, timers, `again`, and undo.
4. Document the final author-visible behavior in the DSL reference.
5. Define and document the current Lua trust boundary before accepting third-party packages as safe content.

Done when one component owns command transactions, every error path has deterministic state behavior, and the implementation matches the documented pipeline.

## Recommended Next Work

Execute in this order:

1. Milestone 17: turn, rollback, undo, and timer semantics.
2. Broader `moonroom check` diagnostics and source context.
3. Scenery things.
4. Darkness and light sources.
5. Parser ambiguity and disambiguation.
6. Save atomicity, compatibility policy, and migration fixtures.
7. Package resource limits and malformed-input hardening.
8. Frontend-neutral session and structured output protocol.

Do not start hot reload, `game.choice()`, TUI, browser, or WASM work until their required state-compatibility or frontend protocol contracts are in place.
