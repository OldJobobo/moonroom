# Packaging and Save Compatibility

During development, run:

```bash
moonroom check .
moonroom test .
moonroom pack . -o dist/game.moon
moonroom check dist/game.moon
moonroom test dist/game.moon
```

`.moon` is a portable JSON package, not a sandbox. Only run games you trust. Moonroom applies file and size limits for robustness, but Lua content remains executable code.

Saves are versioned JSON envelopes and are written atomically. Moonroom accepts legacy raw-state saves as format-0 compatibility input and current envelope version 1. It rejects unknown envelope versions and saves from another compatibility id. Set a stable `game.id`; if a change makes existing state incompatible, publish the changed game with a new id until migration hooks exist. `game.version` is recorded metadata, not a migration mechanism.

Use `save slot.json`, `save --compact slot.json`, and `load slot.json` during play. Keep saves outside a release package; `save.json` is local runtime output.
