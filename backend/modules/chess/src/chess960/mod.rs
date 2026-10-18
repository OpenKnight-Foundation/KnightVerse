//! Chess960 starting-position library (BE-132).
//!
//! Moved out of the `knightverse-core` shell crate and into the `chess`
//! workspace crate, which is where the rest of the position/FEN logic lives.
//! The HTTP handlers live in `api::chess960`, which depends on this module.

pub mod generator;
pub mod models;

pub use generator::Chess960Generator;
pub use models::{Chess960Library, Chess960Position, LibraryMetadata};