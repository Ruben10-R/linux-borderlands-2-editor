#!/usr/bin/env bash
# Cross-compile the Windows .exe app + CLI from your Linux machine (via Docker).
# No Windows PC needed; the finished .exe files land in ./dist/.
#
#   ./build-windows.sh
#
# Then copy dist/bl2-editor.exe (and dist/bl2edit.exe) to a Windows PC and
# double-click. The window shows the app icon; file Open/Save use native dialogs.
set -euo pipefail
cd "$(dirname "$0")"

IMAGE="bl2-windows"
CACHE="$PWD/.docker-cache"
TARGET="x86_64-pc-windows-gnu"
mkdir -p "$CACHE/cargo" "$CACHE/target" dist

echo "==> Building the Windows cross-compile image (first time installs mingw)…"
docker build -t "$IMAGE" -f docker/windows.Dockerfile docker

echo "==> Cross-compiling release .exe binaries…"
docker run --rm \
  --user "$(id -u):$(id -g)" \
  -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e HOME=/tmp \
  -e CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
  -v "$PWD":/work -v "$CACHE/cargo":/cargo -v "$CACHE/target":/target \
  -w /work "$IMAGE" \
  cargo build --release --target "$TARGET" -p bl2-gui -p bl2-cli

cp "$CACHE/target/$TARGET/release/bl2-gui.exe" dist/bl2-editor.exe
cp "$CACHE/target/$TARGET/release/bl2edit.exe" dist/bl2edit.exe

echo
echo "==> Done. Windows binaries in ./dist/ :"
ls -la dist/bl2-editor.exe dist/bl2edit.exe
echo
echo "  Copy dist/bl2-editor.exe to a Windows PC and double-click."
