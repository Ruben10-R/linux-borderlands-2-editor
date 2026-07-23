#!/usr/bin/env bash
# Dev wrapper: build & run inside the official Rust container.
#
# Keeps your host toolchain-free — nothing is installed on the machine.
# Build artifacts + the crate cache live in ./.docker-cache (gitignored),
# owned by your user, so re-runs are fast and nothing is root-owned.
#
# Usage:
#   ./run.sh                              # cargo run (defaults to save0001.sav)
#   ./run.sh run ../samples/save0002.sav  # pass args through to cargo
#   ./run.sh build --release              # any cargo subcommand
#   ./run.sh test
set -euo pipefail
cd "$(dirname "$0")"

IMAGE="rust:1"
CACHE="$PWD/.docker-cache"
mkdir -p "$CACHE/cargo" "$CACHE/target"

# Default to `cargo run` when called with no arguments.
ARGS=("$@")
[ ${#ARGS[@]} -eq 0 ] && ARGS=(run)

# Allocate a TTY only when we actually have one (so CI / non-interactive works).
TTY=()
[ -t 0 ] && TTY=(-it)

exec docker run --rm "${TTY[@]}" \
  --user "$(id -u):$(id -g)" \
  -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e HOME=/tmp \
  -v "$PWD":/work \
  -v "$CACHE/cargo":/cargo \
  -v "$CACHE/target":/target \
  -w /work/poc-roundtrip \
  "$IMAGE" cargo "${ARGS[@]}"
