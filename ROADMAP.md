# Moonroom Roadmap

This document records product directions beyond the active 0.1 release plan in `PLAN.md`. It is intentionally ordered by dependency, not scheduled by date. Items move into `PLAN.md` only when they have a concrete user need, acceptance criteria, and a release target.

## Near-term candidates

These may follow 0.1 when driven by a real game:

- Scenery objects that remain inspectable without cluttering room descriptions.
- Darkness and explicit light sources.
- A decision on whether actor memory and scenes are sufficient for multi-step dialogue.
- Plural object references.
- Edible and drinkable objects.
- More capable save migrations after a second real save format exists.

Each new Rust-owned property must be serializable, undoable, validated, documented, and covered by core, Lua, save/load, and transcript tests.

## Frontend foundation

Before adding another frontend:

1. Define a frontend-neutral engine session API.
2. Return ordered prose, structured events, state changes, errors, and quit state without terminal I/O inside the engine.
3. Define save/load requests and protocol versioning.
4. Add a versioned JSON input/output mode as the first non-CLI consumer.

Only then evaluate a TUI or browser frontend. A new frontend must consume the shared session boundary rather than parse CLI output or duplicate rules.

## Distribution evolution

Possible post-0.1 work includes:

- Broader platform support and automated release publishing.
- Asset inclusion and runtime access semantics.
- A binary or compressed successor to the JSON `.moon` format.
- Cross-compilation-aware standalone builds.
- Package signing or provenance metadata if distribution needs justify it.

Any new package representation requires a new explicit format version. Compression, minification, and Lua bytecode are distribution conveniences, not DRM or sandboxing.

## Trusted-content boundary

Lua games remain executable content. A hardened untrusted-content mode would require a separate security design covering:

- Exposed Lua standard libraries and host capabilities.
- Enforceable instruction, time, memory, and output limits.
- Infinite-loop interruption.
- File and network isolation.
- Adversarial fixtures and a documented threat model.

Input-size limits alone improve robustness but do not create a sandbox.

## Exploratory directions

These should not constrain near-term architecture until their prerequisites and user value are proven:

- Hot reload with state reconciliation for changed definitions.
- Suspended interactions and `game.choice()`.
- Rich terminal UI.
- Browser play.
- WASM builds.
- Editor integration using machine-readable diagnostics.
- A general typed intent model.

Hot reload requires compatibility rules for removed or changed rooms, things, callbacks, and events. Suspended choices require the frontend-neutral session protocol. Browser and WASM work require an acceptable Lua runtime and package-loading strategy.

## Decision rule

A roadmap item becomes active work only when:

1. A game, author workflow, or supported frontend demonstrates the need.
2. Its ownership and save/undo behavior are defined.
3. Validation, tests, and documentation are part of the same deliverable.
4. It has a bounded exit criterion.
