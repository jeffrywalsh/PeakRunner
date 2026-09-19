//! Shared simulation: no graphics, audio, window system, or networking dependencies.
//! These paths keep the existing source and its movement regression tests intact.
#[path = "../../../src/sim.rs"]
pub mod sim;
#[path = "../../../src/terrain.rs"]
pub mod terrain;
#[path = "../../../src/grass.rs"]
pub mod grass;
