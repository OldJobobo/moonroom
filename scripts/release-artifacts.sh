#!/usr/bin/env bash
set -euo pipefail

version=${1:?usage: scripts/release-artifacts.sh VERSION [OUTPUT_DIR]}
output_dir=${2:-"dist/moonroom-$version"}
target=x86_64-unknown-linux-gnu

if [[ $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
  echo "release artifacts are currently supported only on Linux x86_64" >&2
  exit 1
fi

./scripts/release-check.sh
cargo build --release -p mr-cli

mkdir -p "$output_dir"
cp target/release/moonroom "$output_dir/moonroom-$version-$target"
cargo run -q -p mr-cli -- pack showcase/house-under-glass \
  -o "$output_dir/house-under-glass-$version.moon"
"$output_dir/moonroom-$version-$target" build showcase/house-under-glass --standalone \
  -o "$output_dir/house-under-glass-$version-$target"

(cd "$output_dir" && sha256sum * > SHA256SUMS)
printf 'Created release artifacts in %s\n' "$output_dir"
