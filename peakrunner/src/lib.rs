mod app;
mod audio;
mod flag_hud;
#[cfg(not(target_arch = "wasm32"))]
mod online;
mod mouse;
mod drawlist;
use peakrunner_core::grass;
mod scene;
mod map_scene;
use peakrunner_core::{sim, terrain};

pub use app::PeakRunnerApp;
