//! `bl2-gui` — Borderlands 2 save editor UI, built on egui/eframe.
//!
//! One `App` powers both the native window (`src/main.rs`) and the web/WASM
//! build (served by `docker compose up`), editing through the `bl2-save` core.

mod app;
mod io;
#[cfg(not(target_arch = "wasm32"))]
mod paths;
mod theme;
pub use app::App;
