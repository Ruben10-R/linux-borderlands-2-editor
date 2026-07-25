#!/usr/bin/env bash
# Build the NATIVE Linux app (bl2-editor GUI + bl2edit CLI) with Docker.
#
# Keeps your host toolchain-free: the Rust toolchain and GL/X11 build libs live
# in a container image; only the finished binaries land on your disk (./dist/).
#
#   ./build-native.sh
#
# Then run the desktop editor:   ./dist/bl2-editor
#     or the command-line tool:  ./dist/bl2edit --help
#
# To RUN the GUI your desktop needs the usual runtime libs (already present on
# any Linux desktop): libGL, libxkbcommon, libwayland/X11.
set -euo pipefail
cd "$(dirname "$0")"

IMAGE="bl2-native"
CACHE="$PWD/.docker-cache"
mkdir -p "$CACHE/cargo" "$CACHE/target" dist

echo "==> Building the native build image (first time installs GL/X11 libs)…"
docker build -t "$IMAGE" -f docker/native.Dockerfile docker

echo "==> Compiling release binaries (bl2-gui + bl2edit)…"
docker run --rm \
  --user "$(id -u):$(id -g)" \
  -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e HOME=/tmp \
  -v "$PWD":/work -v "$CACHE/cargo":/cargo -v "$CACHE/target":/target \
  -w /work "$IMAGE" \
  cargo build --release -p bl2-gui -p bl2-cli

cp "$CACHE/target/release/bl2-gui" dist/bl2-editor
cp "$CACHE/target/release/bl2edit" dist/bl2edit
chmod +x dist/bl2-editor dist/bl2edit

echo
echo "==> Done. Binaries in ./dist/ :"
ls -la dist/bl2-editor dist/bl2edit
echo
echo "  Desktop editor:  ./dist/bl2-editor     (drag a .sav or profile.bin in, edit, Save)"
echo "  Command line:    ./dist/bl2edit --help"
