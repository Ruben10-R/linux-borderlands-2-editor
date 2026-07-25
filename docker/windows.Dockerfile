# Cross-compile the app to a Windows .exe from Linux, via mingw-w64.
# No Windows machine needed. On Windows the file dialogs use native Win32
# (no GTK) and OpenGL comes from the system, so no extra runtime libs.
FROM rust:1

RUN apt-get update && apt-get install -y --no-install-recommends \
    mingw-w64 \
 && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-pc-windows-gnu
