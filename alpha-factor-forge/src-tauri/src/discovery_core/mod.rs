//! Pure computation modules for Strategy Discovery.

pub mod backtest;
pub mod benchmarks;
pub mod config;
pub mod embargo;
pub mod enumerate;
pub mod gate;
pub mod identity;
pub mod indicators;
pub mod market_data;
pub mod metrics;
pub mod prng;
pub mod random_entry;
pub mod score;
pub mod seed;
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
mod market_data_parity_tests;
#[cfg(test)]
mod parity_support;
#[cfg(test)]
mod runner_config_parity_tests;
#[cfg(test)]
mod signals_split_parity_tests;
