//! `bl2-gui` — read-only Borderlands 2 save viewer built on egui/eframe.
//!
//! One `App` powers both the native window (`src/main.rs`) and the web/WASM build
//! (served by `docker compose up`). It is a thin, read-only view over `bl2-save`.

mod app;
mod io;
mod theme;
pub use app::App;
