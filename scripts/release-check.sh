#!/usr/bin/env bash
set -euo pipefail

workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT

cargo run -q -p mr-cli -- check examples/house
cargo run -q -p mr-cli -- test examples/house
cargo run -q -p mr-cli -- check showcase/house-under-glass
cargo run -q -p mr-cli -- test showcase/house-under-glass

cargo run -q -p mr-cli -- pack showcase/house-under-glass -o "$workdir/house-under-glass.moon"
cargo run -q -p mr-cli -- check "$workdir/house-under-glass.moon"
cargo run -q -p mr-cli -- test "$workdir/house-under-glass.moon"

cargo run -q -p mr-cli -- build showcase/house-under-glass --standalone -o "$workdir/house-under-glass"
printf '%s\n' quit | "$workdir/house-under-glass" >/dev/null

starter="$workdir/starter"
cargo run -q -p mr-cli -- new "$starter"
cargo run -q -p mr-cli -- check "$starter"
cargo run -q -p mr-cli -- test "$starter"
cargo run -q -p mr-cli -- pack "$starter" -o "$workdir/starter.moon"
cargo run -q -p mr-cli -- test "$workdir/starter.moon"
