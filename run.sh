#!/usr/bin/env bash
# Dev wrapper: build & run inside the official Rust container.
#
# Keeps your host toolchain-free — nothing is installed on the machine.
# Build artifacts + the crate cache live in ./.docker-cache (gitignored),
# owned by your user, so re-runs are fast and nothing is root-owned.
#
# Usage (workspace root is the build context):
#   ./run.sh                                              # bl2edit --help
#   ./run.sh run -p bl2-cli -- info samples/save0001.sav  # run the CLI
#   ./run.sh run -p bl2-cli -- set-money samples/save0001.sav 12345
#   ./run.sh test                                         # test the whole workspace
#   ./run.sh build --release                              # any cargo subcommand
set -euo pipefail
cd "$(dirname "$0")"

IMAGE="rust:1"
CACHE="$PWD/.docker-cache"
mkdir -p "$CACHE/cargo" "$CACHE/target"

# Default to running the CLI's help when called with no arguments.
ARGS=("$@")
[ ${#ARGS[@]} -eq 0 ] && ARGS=(run -p bl2-cli -- --help)

# Allocate a TTY only when we actually have one (so CI / non-interactive works).
TTY=()
[ -t 0 ] && TTY=(-it)

exec docker run --rm "${TTY[@]}" \
  --user "$(id -u):$(id -g)" \
  -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e HOME=/tmp \
  -v "$PWD":/work \
  -v "$CACHE/cargo":/cargo \
  -v "$CACHE/target":/target \
  -w /work \
  "$IMAGE" cargo "${ARGS[@]}"
