# Parser Command Reference

Moonroom ignores case, collapses whitespace, and ignores one leading `the`, `a`, or `an` when matching things. If several reachable objects match, it lists them and requires a more specific name.

| Intent | Commands |
| --- | --- |
| Look | `look`, `l`, `look at key`, `x key`, `look north`, `look exits` |
| Move | `north`, `n`, `go north`, `enter hall`, `go through door` |
| Objects | `take key`, `take key from box`, `take all`, `drop key`, `drop all` |
| Placement | `put key in box`, `put key on table`, `look in box`, `look on table` |
| State | `open box`, `close box`, `unlock chest with key`, `lock chest` |
| Interaction | `read note`, `use key`, `use key on door`, `touch key` |
| Wearables | `wear coat`, `remove coat` |
| Actors | `talk to caretaker`, `ask caretaker about key`, `tell caretaker about house`, `show key to caretaker`, `give key to caretaker` |
| Session | `again`/`g`, `undo`, `inventory`/`i`, `save [path]`, `load [path]`, `quit` |

`again` repeats the last advancing command. `undo` restores Rust-owned state from before that command. `it` and `them` refer to the most recently referenced visible thing; plural-pronoun resolution is not supported.

Typical corrective output is intentional: a closed container says so, hidden objects are not exposed, and an unreachable object tells the player it cannot be reached from the current context.
