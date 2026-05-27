# Moonroom

Moonroom is an interactive fiction engine written in Rust, with game worlds defined in Lua.

Run the example:

```bash
cargo run -q -p mr-cli -- play examples/house
```

Interactive play supports shell-style command history with the up/down arrow keys. Piped input still works for smoke tests and scripts.

Run transcript tests:

```bash
cargo run -q -p mr-cli -- test examples/house
```

Create a new game:

```bash
cargo run -q -p mr-cli -- new my-game
```

Inside the built binary, the user-facing command is `moonroom`.

Lua authoring reference: [docs/lua-dsl.md](docs/lua-dsl.md).

See [PLAN.md](PLAN.md) for the project direction and milestone breakdown.
