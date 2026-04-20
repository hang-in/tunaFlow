//! App startup bootstrap — split from `lib.rs` so that `run()` stays small and
//! each initialization concern (env, db, services, window) owns its failure
//! surface. See `docs/plans/refactorRoadmap_2026-04-20.md` §2.1 Finding 6.

pub mod db;
pub mod env;
pub mod services;
pub mod window;
