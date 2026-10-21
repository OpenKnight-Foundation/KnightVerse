//! Backend integration tests (BE-134).
//!
//! This crate used to be a `[[bin]]` whose `main()` hand-ran a sequence of
//! assertions against `Tournament::new`, `BracketConfig`, a `validation` crate
//! that was deleted as dead code, and private `archiving` helpers. It had not
//! compiled since the bracket/Swiss refactor and was dropped from the
//! `backend` workspace, which left the backend with no end-to-end coverage at
//! all.
//!
//! It is now a proper test crate built on `#[actix_web::test]`, driving the
//! real handlers, the real service layer and a real migrated PostgreSQL
//! instance. Tests skip cleanly when `TEST_DATABASE_URL` is not set.
//!
//! - [`support`] — fixtures: database, migrations, players, tokens.
//! - [`game_lifecycle`] — register → login → create → join → move → complete.

pub mod game_lifecycle;
pub mod support;