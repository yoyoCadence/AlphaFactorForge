//! Pure computation modules for Strategy Discovery.

pub mod backtest;
pub mod benchmarks;
pub mod embargo;
pub mod gate;
pub mod indicators;
pub mod metrics;
pub mod prng;
pub mod random_entry;
pub mod score;
pub mod signals;
pub mod split;
pub mod types;

#[cfg(test)]
mod backtest_parity_tests;
#[cfg(test)]
mod benchmark_parity_tests;
#[cfg(test)]
mod gate_score_parity_tests;
#[cfg(test)]
mod indicator_parity_tests;
#[cfg(test)]
mod parity_support;
#[cfg(test)]
mod signals_split_parity_tests;
