//! Pure computation modules for Strategy Discovery.

pub mod backtest;
pub mod indicators;
pub mod metrics;
pub mod types;

#[cfg(test)]
mod backtest_parity_tests;
#[cfg(test)]
mod indicator_parity_tests;
