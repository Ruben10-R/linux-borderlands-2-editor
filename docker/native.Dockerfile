# Build image for the NATIVE desktop app (bl2-gui) + CLI (bl2edit).
#
# The web/WASM build (docker compose up) needs no system libs, but linking the
# native egui/eframe window needs OpenGL + X11/Wayland dev libraries. We bake
# them into an image once so rebuilds are fast and nothing lands on your host.
FROM rust:1

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libgl1-mesa-dev \
    libxkbcommon-dev libxkbcommon-x11-dev \
    libwayland-dev \
    libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
 && rm -rf /var/lib/apt/lists/*
