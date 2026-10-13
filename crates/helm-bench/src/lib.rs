//! Performance benchmarks for Helm. Not shipped.
//!
//! The benchmarks live in `benches/`. This library holds the logic behind the
//! `bench-compare` binary, which the `bench` CI job uses to check a PR's
//! benchmark times against its base commit and the budgets in `budgets.toml`.

pub mod compare;
